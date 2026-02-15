//! Two-way sync for SSG migrator.
//!
//! - manifest: SyncManifest at .ssg-migrator/sync.json
//! - git: change detection via git2
//! - forward: source → output (one-way)
//! - backward: output → source (copy-only paths)

mod backward;
mod forward;
mod git;

use camino::Utf8PathBuf;
use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

/// Manifest stored at `{output_dir}/.ssg-migrator/sync.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    pub source_dir: String,
    pub output_dir: String,
    pub target: String,
    /// Copy-only path mappings: "source/relative/path" -> "output/relative/path"
    pub file_mappings: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_forward: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_backward: Option<String>,
}

/// Path to the manifest file inside output_dir.
pub fn manifest_path(output_dir: &Utf8PathBuf) -> Utf8PathBuf {
    output_dir.join(".ssg-migrator/sync.json")
}

/// Write the sync manifest after migration.
pub fn write_manifest(
    source_dir: &Utf8PathBuf,
    output_dir: &Utf8PathBuf,
    target: &str,
    mappings: &[(String, String)],
) -> Result<()> {
    let dir = output_dir.join(".ssg-migrator");
    fs::create_dir_all(dir.as_path())
        .map_err(|e| miette!("Failed to create .ssg-migrator dir: {}", e))?;

    let manifest = SyncManifest {
        source_dir: source_dir.to_string(),
        output_dir: output_dir.to_string(),
        target: target.to_string(),
        file_mappings: mappings.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        last_sync_forward: None,
        last_sync_backward: None,
    };

    let path = manifest_path(output_dir);
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| miette!("Failed to serialize manifest: {}", e))?;
    fs::write(path.as_path(), json).map_err(|e| miette!("Failed to write manifest: {}", e))?;
    Ok(())
}

/// Read the sync manifest from output_dir. Returns None if it doesn't exist.
pub fn read_manifest(output_dir: &Utf8PathBuf) -> Result<Option<SyncManifest>> {
    let path = manifest_path(output_dir);
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path.as_path())
        .map_err(|e| miette!("Failed to read manifest: {}", e))?;
    let manifest: SyncManifest =
        serde_json::from_str(&contents).map_err(|e| miette!("Invalid manifest: {}", e))?;
    Ok(Some(manifest))
}

/// Collect copy-only path mappings for a migration.
/// Used by write_manifest. Covers: public/*, src/lib/*, src/assets/*,
/// tailwind.config.*, postcss.config.*, *.css in src/
pub fn collect_copy_only_mappings(
    source_dir: &Utf8PathBuf,
    _output_dir: &Utf8PathBuf,
    target: &str,
) -> Vec<(String, String)> {
    let mut mappings = Vec::new();
    let base = source_dir.as_path();

    // public/* -> public/*
    let public_src = source_dir.join("public");
    if public_src.exists() {
        for entry in WalkDir::new(public_src.as_path())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(base) {
                    if let Some(rel_str) = rel.to_str() {
                        mappings.push((rel_str.to_string(), rel_str.to_string()));
                    }
                }
            }
        }
    }

    // src/lib/* -> src/lib/* (Astro) or src/client/lib/* (Next.js)
    let lib_src = source_dir.join("src/lib");
    if lib_src.exists() {
        let out_prefix = match target {
            "nextjs" => "src/client/lib",
            _ => "src/lib",
        };
        for entry in WalkDir::new(lib_src.as_path())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(base) {
                    if let Some(rel_str) = rel.to_str() {
                        let suffix = rel_str
                            .strip_prefix("src/lib/")
                            .unwrap_or(rel_str);
                        let out_rel = format!("{}/{}", out_prefix, suffix);
                        mappings.push((rel_str.to_string(), out_rel));
                    }
                }
            }
        }
    }

    // src/assets/* -> src/assets/* (Astro) or src/client/assets/* (Next.js)
    let assets_src = source_dir.join("src/assets");
    if assets_src.exists() {
        let out_prefix = match target {
            "nextjs" => "src/client/assets",
            _ => "src/assets",
        };
        for entry in WalkDir::new(assets_src.as_path())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(base) {
                    if let Some(rel_str) = rel.to_str() {
                        let suffix = rel_str
                            .strip_prefix("src/assets/")
                            .unwrap_or(rel_str);
                        let out_rel = format!("{}/{}", out_prefix, suffix);
                        mappings.push((rel_str.to_string(), out_rel));
                    }
                }
            }
        }
    }

    // tailwind.config.*
    for ext in ["ts", "js"] {
        let src = format!("tailwind.config.{}", ext);
        if source_dir.join(&src).exists() {
            mappings.push((src.clone(), src));
        }
    }

    // postcss.config.*
    for ext in ["ts", "js"] {
        let src = format!("postcss.config.{}", ext);
        if source_dir.join(&src).exists() {
            mappings.push((src.clone(), src));
        }
    }

    // *.css in src/
    let src_src = source_dir.join("src");
    if src_src.exists() {
        for entry in WalkDir::new(src_src.as_path())
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "css") {
                if let Ok(rel) = path.strip_prefix(base) {
                    if let Some(rel_str) = rel.to_str() {
                        let out_rel = match target {
                            "nextjs" => format!(
                                "src/client/{}",
                                rel_str.strip_prefix("src/").unwrap_or(rel_str)
                            ),
                            _ => rel_str.to_string(),
                        };
                        mappings.push((rel_str.to_string(), out_rel));
                    }
                }
            }
        }
    }

    mappings
}

pub use backward::{sync_backward, SyncResult};
pub use forward::sync_forward;
pub use git::{changed_files, is_git_repo, staged_files};
