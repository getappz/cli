//! Remember what each target last deployed, so an unchanged project is not
//! installed, built and uploaded again (`appz deploy --force` overrides).
//! State: `<project>/.appz/deploys.json`, keyed `"<slug>:<preview|production>"`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Build output and tool state: not source, and they change on every build.
pub(crate) const SKIP_DIRS: [&str; 22] = [
    ".cache",
    ".turbo",
    ".parcel-cache",
    ".react-router",
    ".idea",
    ".vscode",
    "__pycache__",
    ".venv",
    "venv",
    "target",
    ".git",
    "node_modules",
    ".appz",
    ".vercel",
    ".netlify",
    ".wrangler",
    ".astro",
    ".next",
    ".nuxt",
    ".output",
    ".svelte-kit",
    "dist",
];

#[derive(Serialize, Deserialize, Clone)]
pub struct LastDeploy {
    pub fingerprint: String,
    pub url: String,
    pub at: String,
}

fn state_path(root: &Path) -> PathBuf {
    root.join(".appz").join("deploys.json")
}

fn key(slug: &str, prod: bool) -> String {
    format!("{slug}:{}", if prod { "production" } else { "preview" })
}

/// `.env.local` / `.env*.local` hold per-machine tokens the platform CLIs
/// rewrite (e.g. a fresh Vercel OIDC token), so they are not source.
fn is_volatile(name: &str) -> bool {
    name.starts_with(".env") && name.ends_with(".local") || name.ends_with(".log")
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let kind = entry.file_type()?;
        // Symlinks are not followed: a dangling or directory link would make
        // the read fail, and what they point at is not this project's source.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                collect(&path, root, out)?;
            }
        } else if !is_volatile(&name) {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

/// Every source file (relative, sorted) that the fingerprint covers: build
/// output, tool state and volatile local files are already excluded.
pub(crate) fn source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect(root, root, &mut files).map_err(|e| format!("cannot read project: {e}"))?;
    files.sort();
    Ok(files)
}

/// Content hash of the project's source files (names and bytes), plus
/// anything that changes what gets deployed (`extra`).
pub fn fingerprint(root: &Path, extra: &str) -> Result<String, String> {
    let files = source_files(root)?;
    let mut hasher = Sha256::new();
    hasher.update(extra.as_bytes());
    for rel in files {
        hasher.update(b"\0");
        hasher.update(
            rel.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
                .as_bytes(),
        );
        hasher.update(b"\0");
        let bytes = std::fs::read(root.join(&rel))
            .map_err(|e| format!("cannot read {}: {e}", rel.display()))?;
        hasher.update(&bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn load(root: &Path) -> BTreeMap<String, LastDeploy> {
    std::fs::read_to_string(state_path(root))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The previous deploy of this target when its fingerprint still matches.
pub fn unchanged(root: &Path, slug: &str, prod: bool, fingerprint: &str) -> Option<LastDeploy> {
    load(root)
        .remove(&key(slug, prod))
        .filter(|last| last.fingerprint == fingerprint)
}

pub fn record(root: &Path, slug: &str, prod: bool, fingerprint: &str, url: &str) {
    let mut all = load(root);
    all.insert(
        key(slug, prod),
        LastDeploy {
            fingerprint: fingerprint.to_string(),
            url: url.to_string(),
            at: chrono::Utc::now().to_rfc3339(),
        },
    );
    let path = state_path(root);
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    if root.ancestors().any(|a| a.join(".git").exists()) {
        let _ = appz_core::ensure_gitignored(root);
    }
    // Best effort: failing to remember only costs a redundant deploy later.
    // Write a sibling file, then rename over: a concurrent reader never sees
    // a half-written file (which would parse as empty and drop other targets).
    if let Ok(json) = serde_json::to_string_pretty(&all) {
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_tracks_source_but_not_build_output_or_local_env() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        std::fs::write(r.join("index.html"), "a").unwrap();
        let base = fingerprint(r, "x").unwrap();

        std::fs::create_dir(r.join("dist")).unwrap();
        std::fs::write(r.join("dist/out.js"), "built").unwrap();
        std::fs::write(r.join(".env.local"), "TOKEN=1").unwrap();
        assert_eq!(fingerprint(r, "x").unwrap(), base);

        std::fs::write(r.join("index.html"), "b").unwrap();
        assert_ne!(fingerprint(r, "x").unwrap(), base);
        assert_ne!(fingerprint(r, "y").unwrap(), fingerprint(r, "x").unwrap());
    }

    #[test]
    fn remembers_per_target_and_only_while_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        assert!(unchanged(r, "vercel", false, "f1").is_none());
        record(r, "vercel", false, "f1", "https://a.example");
        assert_eq!(
            unchanged(r, "vercel", false, "f1").unwrap().url,
            "https://a.example"
        );
        assert!(unchanged(r, "vercel", false, "f2").is_none());
        assert!(unchanged(r, "vercel", true, "f1").is_none());
        assert!(unchanged(r, "netlify", false, "f1").is_none());
    }
}
