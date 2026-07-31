//! Default formatter resolution for ecosystems where more than one formatter
//! is in common use (JS/TS: Prettier vs. Biome; Python: Black vs. Ruff).
//!
//! Always prefers whatever the project already uses — its own Prettier/Biome
//! config, or its own Black/Ruff config — and only falls back to appz's
//! default when the project has neither. The fallback is provisioned through
//! mise (appz's own tool manager: added to the project's `[tools]` so
//! `mise install` ensures it, same as every other detected toolchain) rather
//! than a per-ecosystem zero-install runner, and invoked directly once mise
//! has put it on PATH.

use std::path::{Path, PathBuf};

use crate::fs::DetectorFilesystem;

const DEFAULT_BIOME_CONFIG: &str = r#"{
  "formatter": {
    "enabled": true,
    "indentStyle": "space"
  }
}
"#;

/// A resolved `appz format` command, plus the mise tool (name, version) that
/// must be in `[tools]` for it to work — set only when the command relies on
/// appz's fallback rather than something the project already provides.
pub struct FormatResolution {
    pub command: String,
    pub mise_tool: Option<(&'static str, &'static str)>,
}

impl FormatResolution {
    fn project_owned(command: String) -> Self {
        Self {
            command,
            mise_tool: None,
        }
    }

    fn via_mise(command: String, tool: &'static str) -> Self {
        Self {
            command,
            mise_tool: Some((tool, "latest")),
        }
    }
}

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
/// `runner` is that package manager's own exec prefix (e.g. `"npx"`,
/// `"pnpm dlx"`, `"bunx"`, `"yarn dlx"`) — used only when the project already
/// has its own Prettier/Biome config, so the project's own pinned
/// `devDependencies` version is what actually runs. The no-config fallback
/// instead provisions Biome through mise and invokes it directly.
/// `appz_home` is appz's own state dir (pass [`appz_home_dir`]; a parameter
/// rather than read internally so tests don't touch the real `~/.appz`).
pub fn resolve_js_format_command(
    fs: &DetectorFilesystem,
    runner: &str,
    appz_home: &Path,
) -> FormatResolution {
    if has_prettier_config(fs) {
        return FormatResolution::project_owned(format!("{runner} prettier --write ."));
    }
    if has_biome_config(fs) {
        return FormatResolution::project_owned(format!(
            "{runner} @biomejs/biome format --write ."
        ));
    }
    let cmd = match ensure_default_biome_config_at(appz_home) {
        Some(dir) => format!("biome format --config-path {} --write .", dir.display()),
        None => "biome format --write .".to_string(),
    };
    FormatResolution::via_mise(cmd, "biome")
}

fn has_ruff_config(fs: &DetectorFilesystem) -> bool {
    fs.is_file("ruff.toml")
        || fs.is_file(".ruff.toml")
        || fs.check_detector("pyproject.toml", false, Some(r"(?m)^\[tool\.ruff"), None)
}

fn has_black_config(fs: &DetectorFilesystem) -> bool {
    fs.check_detector("pyproject.toml", false, Some(r"(?m)^\[tool\.black\]"), None)
}

/// Resolve the `appz format` command for the Python toolchain: prefer the
/// project's own Ruff or Black config (assumed already installed — same
/// project-owns-its-tool reasoning as JS's own-config case); fall back to
/// Ruff provisioned through mise when neither is configured.
pub fn resolve_python_format_command(fs: &DetectorFilesystem) -> FormatResolution {
    if has_ruff_config(fs) {
        return FormatResolution::project_owned("ruff format .".to_string());
    }
    if has_black_config(fs) {
        return FormatResolution::project_owned("black .".to_string());
    }
    FormatResolution::via_mise("ruff format .".to_string(), "ruff")
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
        let res = resolve_js_format_command(&fs_probe, "npx", &home);
        assert_eq!(res.command, "npx prettier --write .");
        assert!(res.mise_tool.is_none());
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
        let res = resolve_js_format_command(&fs_probe, "pnpm dlx", &home);
        assert_eq!(res.command, "pnpm dlx prettier --write .");
        assert!(res.mise_tool.is_none());
    }

    #[test]
    fn uses_project_biome_config_without_config_path_override() {
        let dir = test_dir("own-biome-project");
        let home = test_dir("own-biome-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        fs::write(dir.join("biome.jsonc"), "{}").unwrap();
        let res = resolve_js_format_command(&fs_probe, "bunx", &home);
        assert_eq!(res.command, "bunx @biomejs/biome format --write .");
        assert!(res.mise_tool.is_none());
    }

    #[test]
    fn falls_back_to_mise_provisioned_biome_with_appz_home_config_path() {
        let dir = test_dir("fallback-project");
        let home = test_dir("fallback-home");
        let fs_probe = DetectorFilesystem::new(dir.clone());
        let res = resolve_js_format_command(&fs_probe, "yarn dlx", &home);
        assert!(res.command.starts_with("biome format --config-path "));
        assert!(res.command.ends_with(" --write ."));
        assert_eq!(res.mise_tool, Some(("biome", "latest")));
        assert!(home.join("biome.jsonc").exists());
    }

    #[test]
    fn prefers_ruff_when_ruff_toml_present() {
        let dir = test_dir("ruff-project");
        fs::write(dir.join("ruff.toml"), "").unwrap();
        let res = resolve_python_format_command(&DetectorFilesystem::new(dir));
        assert_eq!(res.command, "ruff format .");
        assert!(res.mise_tool.is_none());
    }

    #[test]
    fn prefers_ruff_when_tool_ruff_section_in_pyproject() {
        let dir = test_dir("ruff-pyproject");
        fs::write(dir.join("pyproject.toml"), "[tool.ruff]\nline-length = 100\n").unwrap();
        let res = resolve_python_format_command(&DetectorFilesystem::new(dir));
        assert_eq!(res.command, "ruff format .");
        assert!(res.mise_tool.is_none());
    }

    #[test]
    fn prefers_black_when_tool_black_section_in_pyproject() {
        let dir = test_dir("black-pyproject");
        fs::write(dir.join("pyproject.toml"), "[tool.black]\nline-length = 88\n").unwrap();
        let res = resolve_python_format_command(&DetectorFilesystem::new(dir));
        assert_eq!(res.command, "black .");
        assert!(res.mise_tool.is_none());
    }

    #[test]
    fn falls_back_to_mise_provisioned_ruff_with_no_config() {
        let dir = test_dir("python-fallback");
        let res = resolve_python_format_command(&DetectorFilesystem::new(dir));
        assert_eq!(res.command, "ruff format .");
        assert_eq!(res.mise_tool, Some(("ruff", "latest")));
    }
}
