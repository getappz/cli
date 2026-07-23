use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::detect::DetectedToolchain;
use crate::generator;

/// FNV-1a hasher that accumulates bytes and produces a hex string.
struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
    fn finish_hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// appz's own bookkeeping for a project: `<root>/.appz/state.jsonl`.
pub fn state_path(root: &Path) -> PathBuf {
    root.join(".appz").join("state.jsonl")
}

/// Path to `mise.toml` in the project root (no longer a shadow dir).
pub fn mise_config_path(root: &Path) -> PathBuf {
    root.join("mise.toml")
}

/// Ensure `<root>/.gitignore` contains a `.appz/` entry. Idempotent.
fn ensure_gitignored(root: &Path) {
    let gitignore = root.join(".gitignore");
    let existing = fs::read_to_string(&gitignore).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == ".appz/") {
        return;
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("# appz local state (not shared)\n.appz/\n");
    fs::write(&gitignore, content).unwrap_or_else(|e| {
        eprintln!("error: cannot write '.gitignore' at '{}': {e}", gitignore.display());
        std::process::exit(1);
    });
}

/// Files whose content changes should trigger re-detection + re-install.
const INPUT_FILES: &[&str] = &[
    "package.json", "package-lock.json",
    "pnpm-lock.yaml", "yarn.lock", "bun.lockb", "bun.lock",
    "Cargo.toml", "Cargo.lock",
    "go.mod", "go.sum",
    ".nvmrc", ".node-version", ".python-version", ".ruby-version", ".go-version",
    "rust-toolchain.toml", "rust-toolchain", ".terraform-version",
    "requirements.txt", "pyproject.toml", "Pipfile", "Gemfile", "Gemfile.lock", "setup.py",
    "turbo.json", "rush.json", "nx.json",
    "Dockerfile", "Dockerfile.vercel", "mcp.json", "mix.exs",
];

/// Hash of all input files in the project that affect detection/mise config.
pub fn compute_input_hash(root: &Path) -> String {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = FnvHasher::new();
    for name in INPUT_FILES {
        let path = canonical.join(name);
        if let Ok(content) = fs::read(&path) {
            hasher.update(name.as_bytes());
            hasher.update(&content);
        }
    }
    hasher.finish_hex()
}

// ── Snapshot types ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub ts: String,
    pub input_hash: String,
    pub toolchains: Vec<StoredToolchain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToolchain {
    pub name: String,
    pub slug: String,
    pub mise_plugin: String,
    pub version: String,
    pub frameworks: Vec<String>,
    pub build_command: Option<String>,
    pub install_command: Option<String>,
    pub dev_command: Option<String>,
    pub test_command: Option<String>,
    pub lint_command: Option<String>,
    pub format_command: Option<String>,
    pub output_directory: Option<String>,
    pub env_prefix: Option<String>,
}

impl From<&DetectedToolchain> for StoredToolchain {
    fn from(tc: &DetectedToolchain) -> Self {
        Self {
            name: tc.name.to_string(),
            slug: tc.slug.to_string(),
            mise_plugin: tc.mise_plugin.to_string(),
            version: tc.version.clone(),
            frameworks: tc.frameworks.iter().map(|f| f.name.to_string()).collect(),
            build_command: tc.build_command.clone(),
            install_command: tc.install_command.clone(),
            dev_command: tc.dev_command.clone(),
            test_command: tc.test_command.clone(),
            lint_command: tc.lint_command.clone(),
            format_command: tc.format_command.clone(),
            output_directory: tc.output_directory.map(|s| s.to_string()),
            env_prefix: tc.env_prefix.map(|s| s.to_string()),
        }
    }
}

// ── Write / Read ─────────────────────────────────────────────────

/// Write current state + input hash as one JSONL line under `.appz/`, and
/// write (or merge into) `mise.toml` at the project root.
/// Returns the path to `mise.toml`.
pub fn write_state(root: &Path, toolchains: &[DetectedToolchain]) -> PathBuf {
    let appz_dir = root.join(".appz");
    fs::create_dir_all(&appz_dir).unwrap_or_else(|e| {
        eprintln!("error: cannot create '.appz' dir '{}': {e}", appz_dir.display());
        std::process::exit(1);
    });
    ensure_gitignored(root);

    let sp = state_path(root);
    let mp = mise_config_path(root);

    let snapshot = StateSnapshot {
        ts: Utc::now().to_rfc3339(),
        input_hash: compute_input_hash(root),
        toolchains: toolchains.iter().map(StoredToolchain::from).collect(),
    };

    // Append snapshot as JSONL line
    let line = serde_json::to_string(&snapshot).expect("serialize state");
    let mut content = fs::read_to_string(&sp).unwrap_or_default();
    content.push_str(&line);
    content.push('\n');
    fs::write(&sp, &content).unwrap_or_else(|e| {
        eprintln!("error: cannot write state '{}': {e}", sp.display());
        std::process::exit(1);
    });

    // Write or merge mise.toml
    let existing = fs::read_to_string(&mp).ok();
    let toml = generator::generate_merged(existing.as_deref(), toolchains).unwrap_or_else(|e| {
        eprintln!("error: cannot parse existing '{}': {e}", mp.display());
        std::process::exit(1);
    });
    fs::write(&mp, &toml).unwrap_or_else(|e| {
        eprintln!("error: cannot write mise config '{}': {e}", mp.display());
        std::process::exit(1);
    });

    mp
}

/// Read the latest full snapshot (toolchains + hash) from JSONL.
pub fn read_latest_snapshot(root: &Path) -> Option<StateSnapshot> {
    let sp = state_path(root);
    let content = fs::read_to_string(sp).ok()?;
    let last_line = content.lines().last()?;
    serde_json::from_str(last_line).ok()
}

/// Read toolchains from the latest snapshot (backward compat).
pub fn read_latest_state(root: &Path) -> Option<Vec<StoredToolchain>> {
    read_latest_snapshot(root).map(|s| s.toolchains)
}

/// Check if the stored input hash matches current files.
pub fn inputs_unchanged(root: &Path) -> bool {
    match read_latest_snapshot(root) {
        Some(snap) => snap.input_hash == compute_input_hash(root),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::detect::DetectedToolchain;

    fn sample_toolchain() -> DetectedToolchain {
        DetectedToolchain {
            name: "Node.js",
            slug: "node",
            mise_plugin: "node",
            version: "20".to_string(),
            version_files: &[],
            frameworks: Vec::new(),
            build_command: None,
            install_command: None,
            dev_command: None,
            test_command: None,
            lint_command: None,
            format_command: None,
            output_directory: None,
            env_prefix: None,
        }
    }

    #[test]
    fn test_input_hash_deterministic() {
        let dir = std::env::temp_dir().join("appz-hash-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{"name":"x"}"#).unwrap();

        let h1 = compute_input_hash(&dir);
        let h2 = compute_input_hash(&dir);
        assert_eq!(h1, h2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_input_hash_changes_on_file_change() {
        let dir = std::env::temp_dir().join("appz-hash-change");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{"name":"x"}"#).unwrap();

        let h1 = compute_input_hash(&dir);
        fs::write(dir.join("package.json"), r#"{"name":"y","dependencies":{"next":"^14"}}"#).unwrap();
        let h2 = compute_input_hash(&dir);
        assert_ne!(h1, h2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_state_creates_mise_toml_and_state_at_root() {
        let dir = std::env::temp_dir().join("appz-write-state-fresh");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        write_state(&dir, &[sample_toolchain()]);

        assert!(dir.join("mise.toml").exists(), "mise.toml written to project root");
        assert!(dir.join(".appz").join("state.jsonl").exists(), "state cache written under .appz/");
        let toml = fs::read_to_string(dir.join("mise.toml")).unwrap();
        assert!(toml.contains("node = \"20\""));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_state_merges_into_existing_mise_toml() {
        let dir = std::env::temp_dir().join("appz-write-state-merge");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mise.toml"), "[tasks.build]\nrun = \"echo hi\"\n# keep me\n").unwrap();

        write_state(&dir, &[sample_toolchain()]);

        let toml = fs::read_to_string(dir.join("mise.toml")).unwrap();
        assert!(toml.contains("[tasks.build]"), "existing table preserved:\n{toml}");
        assert!(toml.contains("# keep me"), "existing comment preserved:\n{toml}");
        assert!(toml.contains("node = \"20\""), "tools table upserted:\n{toml}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_state_gitignores_appz_dir_idempotently() {
        let dir = std::env::temp_dir().join("appz-write-state-gitignore");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        write_state(&dir, &[sample_toolchain()]);
        write_state(&dir, &[sample_toolchain()]);

        let gitignore = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(gitignore.matches(".appz/").count(), 1, "entry appended exactly once:\n{gitignore}");

        let _ = fs::remove_dir_all(&dir);
    }
}
