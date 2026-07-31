//! Default formatter resolution for the JS/TS ecosystem (npm/pnpm/bun/yarn).
//!
//! Prefers whatever formatter the project already uses (Prettier, then an
//! existing Biome config); only falls back to Biome with appz's own shared
//! config when the project has neither, so `appz format` never drops an
//! unrequested config file into the user's repo.

use std::path::{Path, PathBuf};

use crate::fs::DetectorFilesystem;

const DEFAULT_BIOME_CONFIG: &str = r#"{
  "formatter": {
    "enabled": true,
    "indentStyle": "space"
  }
}
"#;

/// `~/.appz` — appz's own state dir, distinct from the per-project `.appz/`
/// written into a repo (see `storage::state_path`).
pub fn appz_home_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".appz")
}

/// Ensure `<dir>/biome.jsonc` exists, writing appz's default config the
/// first time it's needed. Returns `dir` back (Biome's `--config-path` takes
/// a directory, not a file) so callers can pass it straight through.
fn ensure_default_biome_config_at(dir: &Path) -> Option<PathBuf> {
    let config = dir.join("biome.jsonc");
    if !config.exists() {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::write(&config, DEFAULT_BIOME_CONFIG).ok()?;
    }
    Some(dir.to_path_buf())
}

fn has_prettier_config(fs: &DetectorFilesystem) -> bool {
    [
        ".prettierrc",
        ".prettierrc.json",
        ".prettierrc.yaml",
        ".prettierrc.yml",
        ".prettierrc.js",
        ".prettierrc.cjs",
        ".prettierrc.mjs",
        "prettier.config.js",
        "prettier.config.cjs",
        "prettier.config.mjs",
    ]
    .iter()
    .any(|f| fs.is_file(f))
        || fs.check_detector("package.json", false, None, Some("prettier"))
}

fn has_biome_config(fs: &DetectorFilesystem) -> bool {
    fs.is_file("biome.json") || fs.is_file("biome.jsonc")
}

/// Resolve the `appz format` command for a JS/TS package manager toolchain.
/// `runner` is that package manager's zero-install exec prefix, e.g. `"npx"`,
/// `"pnpm dlx"`, `"bunx"`, `"yarn dlx"`. `appz_home` is appz's own state dir
/// (pass [`appz_home_dir`]; a parameter rather than read internally so tests
/// don't touch the real `~/.appz`).
pub fn resolve_js_format_command(
    fs: &DetectorFilesystem,
    runner: &str,
    appz_home: &Path,
) -> Option<String> {
    if has_prettier_config(fs) {
        return Some(format!("{runner} prettier --write ."));
    }
    if has_biome_config(fs) {
        return Some(format!("{runner} @biomejs/biome format --write ."));
    }
    match ensure_default_biome_config_at(appz_home) {
        Some(dir) => Some(format!(
            "{runner} @biomejs/biome format --config-path {} --write .",
            dir.display()
        )),
        None => Some(format!("{runner} @biomejs/biome format --write .")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-core-format-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn prefers_prettier_when_prettierrc_present() {
        let dir = test_dir("prettier-project");
        let home = test_dir("prettier-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        fs::write(dir.join(".prettierrc"), "{}").unwrap();
        assert_eq!(
            resolve_js_format_command(&fs_probe, "npx", &home),
            Some("npx prettier --write .".to_string())
        );
    }

    #[test]
    fn prefers_prettier_when_declared_in_package_json() {
        let dir = test_dir("prettier-pkg-project");
        let home = test_dir("prettier-pkg-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        fs::write(
            dir.join("package.json"),
            r#"{"devDependencies":{"prettier":"^3.0.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            resolve_js_format_command(&fs_probe, "pnpm dlx", &home),
            Some("pnpm dlx prettier --write .".to_string())
        );
    }

    #[test]
    fn uses_project_biome_config_without_config_path_override() {
        let dir = test_dir("own-biome-project");
        let home = test_dir("own-biome-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        fs::write(dir.join("biome.jsonc"), "{}").unwrap();
        assert_eq!(
            resolve_js_format_command(&fs_probe, "bunx", &home),
            Some("bunx @biomejs/biome format --write .".to_string())
        );
    }

    #[test]
    fn falls_back_to_biome_with_appz_home_config_path() {
        let dir = test_dir("fallback-project");
        let home = test_dir("fallback-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        let cmd = resolve_js_format_command(&fs_probe, "yarn dlx", &home).unwrap();
        assert!(cmd.starts_with("yarn dlx @biomejs/biome format --config-path "));
        assert!(cmd.ends_with(" --write ."));
        assert!(home.join("biome.jsonc").exists());
    }
}
