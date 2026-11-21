//! Backup configuration, stable project identifier, and default repository path.

// Consumed by upcoming backup tasks (open_repo, create, list, restore).
#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use super::retention::{Retention, parse_size};

/// Backup configuration parsed from the `"backup"` key in `appz.jsonc`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Whether automatic backups after deploy are enabled (defaults to enabled when omitted).
    pub enabled: Option<bool>,
    /// `"local"` (or omitted) for `~/.appz/backups/<project_id>`, or an explicit path / rustic URI.
    pub repository: Option<String>,
    /// Snapshot retention counts and size limits.
    #[serde(default)]
    pub retention: Retention,
}

impl BackupConfig {
    /// Returns `true` unless `enabled` is explicitly `Some(false)`.
    pub fn is_enabled(&self) -> bool {
        self.enabled != Some(false)
    }
}

/// Read the `"backup"` section from `<root>/appz.jsonc`, falling back to defaults when absent.
pub fn read_config(root: &Path) -> Result<BackupConfig, String> {
    let path = root.join("appz.jsonc");
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BackupConfig::default()),
        Err(e) => return Err(format!("reading {}: {e}", path.display())),
    };

    let stripped = appz_core::strip_jsonc(&source);
    let value: serde_json::Value =
        serde_json::from_str(&stripped).map_err(|e| format!("parsing {}: {e}", path.display()))?;

    let cfg: BackupConfig = match value.get("backup") {
        Some(backup) if !backup.is_null() => serde_json::from_value(backup.clone())
            .map_err(|e| format!("invalid \"backup\" config in {}: {e}", path.display()))?,
        _ => BackupConfig::default(),
    };

    if let Some(size) = &cfg.retention.max_size {
        parse_size(size)?;
    }

    Ok(cfg)
}

/// Stable 16-hex-character identifier derived from the canonical `/`-normalized project root.
pub fn project_id(root: &Path) -> String {
    let canonical = appz_core::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let normalized = canonical.to_string_lossy().replace('\\', "/");
    let digest = Sha256::digest(normalized.as_bytes());
    format!("{digest:x}")[..16].to_string()
}

/// Resolve the backup repository location for `root`: `~/.appz/backups/<project_id>` for
/// `"local"` or omitted `repository`, otherwise the configured path/URI.
pub fn repo_location(cfg: &BackupConfig, root: &Path) -> String {
    match cfg
        .repository
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None | Some("local") => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".appz")
            .join("backups")
            .join(project_id(root))
            .to_string_lossy()
            .into_owned(),
        Some(repo) => repo.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let cfg = BackupConfig::default();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.repository, None);
        assert_eq!(cfg.retention.keep_last, 10);
        assert_eq!(cfg.retention.keep_daily, 14);
        assert_eq!(cfg.retention.keep_weekly, 8);
        assert_eq!(cfg.retention.max_size.as_deref(), Some("2GiB"));
        assert_eq!(cfg.retention.min_keep, 3);
    }

    #[test]
    fn parses_backup_key_from_jsonc_with_comments() {
        let dir = tempfile::tempdir().unwrap();
        let jsonc = r#"{
            // Project deployment config
            "deploy": { "default": "vercel" },
            /* Rustic backup settings */
            "backup": {
                "enabled": false,
                "repository": "rclone:remote:appz-backups",
                "retention": {
                    "keep_last": 5,
                    "max_size": "500MiB"
                }
            }
        }"#;
        std::fs::write(dir.path().join("appz.jsonc"), jsonc).unwrap();

        let cfg = read_config(dir.path()).unwrap();
        assert_eq!(cfg.enabled, Some(false));
        assert!(!cfg.is_enabled());
        assert_eq!(
            cfg.repository.as_deref(),
            Some("rclone:remote:appz-backups")
        );
        assert_eq!(cfg.retention.keep_last, 5);
        assert_eq!(cfg.retention.keep_daily, 14);
        assert_eq!(cfg.retention.keep_weekly, 8);
        assert_eq!(cfg.retention.max_size.as_deref(), Some("500MiB"));
        assert_eq!(cfg.retention.min_keep, 3);
    }

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("2GiB").unwrap(), 2_147_483_648);
        assert_eq!(parse_size("500MiB").unwrap(), 524_288_000);
        assert_eq!(parse_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("4096").unwrap(), 4096);
        assert!(parse_size("12x").is_err());
    }

    #[test]
    fn project_id_is_stable_for_the_same_root() {
        let dir = tempfile::tempdir().unwrap();
        let id1 = project_id(dir.path());
        let id2 = project_id(&dir.path().join("."));
        assert_eq!(id1.len(), 16);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id1, id2);

        let other = tempfile::tempdir().unwrap();
        assert_ne!(id1, project_id(other.path()));
    }

    #[test]
    fn repo_location_defaults_to_appz_home_backups() {
        let dir = tempfile::tempdir().unwrap();
        let id = project_id(dir.path());
        let expected_suffix = std::path::Path::new(".appz").join("backups").join(&id);

        let local = repo_location(&BackupConfig::default(), dir.path());
        assert!(
            std::path::Path::new(&local).ends_with(&expected_suffix),
            "unexpected local repo path: {local}"
        );

        let explicit_local = repo_location(
            &BackupConfig {
                repository: Some("local".to_string()),
                ..Default::default()
            },
            dir.path(),
        );
        assert_eq!(local, explicit_local);

        let remote = repo_location(
            &BackupConfig {
                repository: Some("s3:s3.amazonaws.com/bucket".to_string()),
                ..Default::default()
            },
            dir.path(),
        );
        assert_eq!(remote, "s3:s3.amazonaws.com/bucket");
    }
}
