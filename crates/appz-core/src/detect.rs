use std::path::Path;

use serde::Serialize;

use crate::frameworks;
use crate::fs::DetectorFilesystem;
use crate::toolchain_registry::get_frameworks;
use crate::toolchains::{DetectionConfidence, DetectionCriteria, Framework};

/// Overrides read from project config file (vercel.json / appz.json).
#[derive(Debug, Default, Clone)]
pub struct CommandOverrides {
    pub build_command: Option<String>,
    pub install_command: Option<String>,
    pub dev_command: Option<String>,
}

impl CommandOverrides {
    fn set_field(&mut self, key: &str, value: &str) {
        match key {
            "buildCommand" | "build_command" => self.build_command = Some(value.to_string()),
            "installCommand" | "install_command" => self.install_command = Some(value.to_string()),
            "devCommand" | "dev_command" => self.dev_command = Some(value.to_string()),
            _ => {}
        }
    }
}

/// Workspace / monorepo manager detection result.
#[derive(Debug, Default, Clone, Serialize)]
pub struct MonorepoConfig {
    pub manager: Option<String>,
    pub package_paths: Vec<String>,
}

/// Detect monorepo workspaces from package.json or workspace config files.
pub fn detect_monorepo(root: &Path) -> MonorepoConfig {
    let pkg_json = root.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg_json) {
        // Check for pnpm workspace in separate file first
        if root.join("pnpm-workspace.yaml").exists() || root.join("pnpm-workspace.yml").exists() {
            return MonorepoConfig {
                manager: Some("pnpm".to_string()),
                package_paths: vec!["packages/*".to_string()],
            };
        }

        // Check for npm/yarn workspaces in package.json
        if let Ok(val) = content.parse::<serde_json::Value>()
            && let Some(ws) = val.get("workspaces")
        {
            // pnpm / yarn
            if let Some(arr) = ws.as_array() {
                return MonorepoConfig {
                    manager: if root.join("pnpm-lock.yaml").exists() {
                        Some("pnpm".to_string())
                    } else if root.join("yarn.lock").exists() {
                        Some("yarn".to_string())
                    } else {
                        Some("npm".to_string())
                    },
                    package_paths: arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                };
            }
            // npm workspaces
            if let Some(arr) = val.pointer("/workspaces/packages")
                && let Some(arr) = arr.as_array()
            {
                return MonorepoConfig {
                    manager: Some("npm".to_string()),
                    package_paths: arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                };
            }
        }
    }

    // lerna.json
    if root.join("lerna.json").exists() {
        return MonorepoConfig {
            manager: Some("lerna".to_string()),
            package_paths: vec!["packages/*".to_string()],
        };
    }

    // pnpm-workspace.yaml
    let pnpm_yaml = root.join("pnpm-workspace.yaml");
    if pnpm_yaml.exists()
        && let Ok(content) = std::fs::read_to_string(&pnpm_yaml)
        && let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content)
        && let Some(pkgs) = val.get("packages").and_then(|v| v.as_sequence())
    {
        return MonorepoConfig {
            manager: Some("pnpm".to_string()),
            package_paths: pkgs
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        };
    }

    MonorepoConfig::default()
}

/// Strip JSONC comments (// and /* */) before JSON parsing.
pub fn strip_jsonc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Read command overrides from project config file (Vercel-style).
/// Tries: appz.jsonc -> appz.toml -> appz.yaml -> defaults.
fn read_overrides(root: &Path) -> CommandOverrides {
    let jsonc_path = root.join("appz.jsonc");
    let toml_path = root.join("appz.toml");
    let yaml_path = root.join("appz.yaml");

    // JSONC
    if jsonc_path.exists()
        && let Ok(content) = std::fs::read_to_string(&jsonc_path)
    {
        let cleaned = strip_jsonc(&content);
        if let Ok(val) = cleaned.parse::<serde_json::Value>()
            && let Some(obj) = val.as_object()
        {
            let mut ov = CommandOverrides::default();
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    ov.set_field(k, s);
                }
            }
            return ov;
        }
    }

    // TOML
    if toml_path.exists()
        && let Ok(content) = std::fs::read_to_string(&toml_path)
        && let Ok(val) = content.parse::<toml::Value>()
    {
        let mut ov = CommandOverrides::default();
        if let Some(table) = val.as_table() {
            for (k, v) in table {
                if let Some(s) = v.as_str() {
                    ov.set_field(k, s);
                }
            }
        }
        return ov;
    }

    // YAML
    if yaml_path.exists()
        && let Ok(content) = std::fs::read_to_string(&yaml_path)
        && let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content)
        && let Some(mapping) = val.as_mapping()
    {
        let mut ov = CommandOverrides::default();
        for (k, v) in mapping {
            if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                ov.set_field(key, val);
            }
        }
        return ov;
    }

    CommandOverrides::default()
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectedToolchain {
    pub name: &'static str,
    pub slug: &'static str,
    pub mise_plugin: &'static str,
    pub version: String,
    pub version_files: &'static [&'static str],
    pub frameworks: Vec<frameworks::DetectedFramework>,
    pub build_command: Option<String>,
    pub install_command: Option<String>,
    pub dev_command: Option<String>,
    pub test_command: Option<String>,
    pub lint_command: Option<String>,
    pub format_command: Option<String>,
    /// A mise tool (name, version) needed for `format_command` to work,
    /// beyond what this toolchain's own `mise_plugin` already provides — set
    /// only when `format_command` falls back to an appz default rather than
    /// something the project already provides.
    pub format_tool: Option<(&'static str, &'static str)>,
    pub output_directory: Option<&'static str>,
    pub env_prefix: Option<&'static str>,
}

/// Check detection criteria against the filesystem.
fn check_criteria(fs: &DetectorFilesystem, criteria: &DetectionCriteria) -> bool {
    // Both empty → no criteria to match (Vercel's behavior: treat as non-match)
    if criteria.every.is_empty() && criteria.some.is_empty() {
        return false;
    }
    for detector in criteria.every {
        if !fs.check_detector(
            detector.path,
            detector.is_glob,
            detector.match_content,
            detector.match_package,
        ) {
            return false;
        }
    }
    if !criteria.some.is_empty() {
        let mut some_match = false;
        for detector in criteria.some {
            if fs.check_detector(
                detector.path,
                detector.is_glob,
                detector.match_content,
                detector.match_package,
            ) {
                some_match = true;
                break;
            }
        }
        if !some_match {
            return false;
        }
    }
    true
}

fn detect_version(fs: &DetectorFilesystem, version_files: &[&str]) -> Option<String> {
    for vf in version_files {
        if let Some(content) = fs.read_file(vf) {
            let version = content.trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    None
}

fn scan_frameworks(fs: &DetectorFilesystem, slug: &str) -> Vec<frameworks::DetectedFramework> {
    match slug {
        "node" => fs
            .read_file("package.json")
            .map(|c| frameworks::scan_npm(&c))
            .unwrap_or_default(),
        "rust" => fs
            .read_file("Cargo.toml")
            .map(|c| frameworks::scan_cargo(&c))
            .unwrap_or_default(),
        "python" => {
            let mut f = Vec::new();
            if let Some(c) = fs.read_file("pyproject.toml") {
                f.extend(frameworks::scan_python(&c));
            }
            if let Some(c) = fs.read_file("requirements.txt") {
                f.extend(frameworks::scan_python(&c));
            }
            f
        }
        "go" => fs
            .read_file("go.mod")
            .map(|c| frameworks::scan_go(&c))
            .unwrap_or_default(),
        "ruby" => {
            let mut f = Vec::new();
            if let Some(c) = fs.read_file("Gemfile") {
                f.extend(frameworks::scan_ruby(&c));
            }
            if let Some(c) = fs.read_file("Gemfile.lock") {
                f.extend(frameworks::scan_ruby(&c));
            }
            f
        }
        "java" => {
            let mut f = Vec::new();
            if let Some(c) = fs.read_file("pom.xml") {
                f.extend(frameworks::scan_java(&c));
            }
            if let Some(c) = fs.read_file("build.gradle") {
                f.extend(frameworks::scan_java(&c));
            }
            f
        }
        "elixir" => fs
            .read_file("mix.exs")
            .map(|c| frameworks::scan_elixir(&c))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Extract version from package.json for a Framework's matchPackage detector.
fn detect_package_version(fs: &DetectorFilesystem, tc: &Framework) -> Option<String> {
    let detectors = tc.detectors.every.iter().chain(tc.detectors.some.iter());
    for d in detectors {
        if let Some(pkg) = d.match_package {
            let content = fs.read_file("package.json")?;
            if let Some(v) = crate::pkg::parse_deps(&content).get(pkg)
                && !v.is_empty()
            {
                return Some(v.clone());
            }
        }
    }
    None
}

/// Detect all applicable toolchains in the given root directory.
pub fn detect_toolchains(root: &Path) -> Result<Vec<DetectedToolchain>, String> {
    let root = crate::paths::canonicalize(root).map_err(|e| e.to_string())?;
    let fs = DetectorFilesystem::new(root.clone());
    let overrides = read_overrides(&root);
    let mut detected: Vec<DetectedToolchain> = Vec::new();

    // First pass: collect all matches
    for tc in get_frameworks() {
        if check_criteria(&fs, &tc.detectors) {
            let version = detect_version(&fs, tc.version_files)
                .or_else(|| detect_package_version(&fs, tc))
                .unwrap_or_else(|| tc.default_version.to_string());
            let frameworks = scan_frameworks(&fs, tc.slug);

            let over = &overrides;
            let build_cmd = over
                .build_command
                .clone()
                .or_else(|| tc.commands.build.map(String::from));
            let install_cmd = over
                .install_command
                .clone()
                .or_else(|| tc.commands.install.map(String::from));
            let dev_cmd = over
                .dev_command
                .clone()
                .or_else(|| tc.commands.dev.map(String::from));
            let test_cmd = tc.commands.test.map(String::from);
            let lint_cmd = tc.commands.lint.map(String::from);
            // JS/TS package managers and Python get a computed default
            // (prefer the project's own formatter config; fall back to a
            // mise-provisioned tool) instead of a static table entry — see
            // `format_defaults`.
            let appz_home = crate::format_defaults::appz_home_dir();
            // `slug` alone can't distinguish BUN_PM from BUN_RUNTIME — both are
            // "bun" (see toolchain_registry's duplicate-slug dedup test) and a
            // bun project detects both, so match `name` too or the command
            // would run twice.
            let (format_cmd, format_tool) = match (tc.slug, tc.name) {
                ("npm", _) => {
                    let r =
                        crate::format_defaults::resolve_js_format_command(&fs, "npx", &appz_home);
                    (Some(r.command), r.mise_tool)
                }
                ("pnpm", _) => {
                    let r = crate::format_defaults::resolve_js_format_command(
                        &fs, "pnpm dlx", &appz_home,
                    );
                    (Some(r.command), r.mise_tool)
                }
                ("bun", "bun") => {
                    let r =
                        crate::format_defaults::resolve_js_format_command(&fs, "bunx", &appz_home);
                    (Some(r.command), r.mise_tool)
                }
                ("yarn", _) => {
                    let r = crate::format_defaults::resolve_js_format_command(
                        &fs, "yarn dlx", &appz_home,
                    );
                    (Some(r.command), r.mise_tool)
                }
                ("python", _) => {
                    let r = crate::format_defaults::resolve_python_format_command(&fs);
                    (Some(r.command), r.mise_tool)
                }
                _ => (tc.commands.format.map(String::from), None),
            };

            detected.push(DetectedToolchain {
                name: tc.name,
                slug: tc.slug,
                mise_plugin: tc.mise_plugin,
                version,
                version_files: tc.version_files,
                frameworks,
                build_command: build_cmd,
                install_command: install_cmd,
                dev_command: dev_cmd,
                test_command: test_cmd,
                lint_command: lint_cmd,
                format_command: format_cmd,
                format_tool,
                output_directory: tc.output_directory,
                env_prefix: tc.env_prefix,
            });
        }
    }

    // Second pass: apply supersedes (remove frameworks superseded by others)
    let mut superseded: Vec<&str> = Vec::new();
    for tc in &detected {
        // Find the Framework entry to check supersedes
        if let Some(fw) = get_frameworks().iter().find(|f| f.slug == tc.slug) {
            superseded.extend(fw.supersedes.iter().copied());
        }
    }
    detected.retain(|tc| !superseded.contains(&tc.slug));

    // Third pass: if any strong match exists, drop weak-only matches
    let has_strong = detected.iter().any(|tc| {
        get_frameworks().iter().any(|f| {
            f.slug == tc.slug && matches!(f.detection_confidence, DetectionConfidence::Strong)
        })
    });
    if has_strong {
        detected.retain(|tc| {
            !get_frameworks().iter().any(|f| {
                f.slug == tc.slug && matches!(f.detection_confidence, DetectionConfidence::Weak)
            })
        });
    }

    Ok(detected)
}

/// Map a package manager slug to its install command prefix.
pub fn pm_install_cmd(slug: &str) -> &'static str {
    match slug {
        "pnpm" => "pnpm install",
        "yarn" => "yarn install",
        "bun" => "bun install",
        _ => "npm install",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-core-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_detect_node_from_package_json() {
        let dir = test_dir("node");
        fs::write(dir.join("package.json"), r#"{"name": "test"}"#).unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let node = result.iter().find(|t| t.slug == "node");
        assert!(node.is_some(), "node should be detected");
        assert_eq!(node.unwrap().version, "lts");
    }

    #[test]
    fn test_detect_rust_from_cargo_toml() {
        let dir = test_dir("rust");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\n[dependencies]\naxum = \"0.7\"",
        )
        .unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let rust = result.iter().find(|t| t.slug == "rust");
        assert!(rust.is_some(), "rust should be detected");
        assert!(
            rust.unwrap().frameworks.iter().any(|f| f.name == "Axum"),
            "axum framework should be detected"
        );
    }

    #[test]
    fn test_detect_rust_gets_cargo_build_command() {
        let dir = test_dir("rust-build-command");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let rust = result.iter().find(|t| t.slug == "rust").unwrap();
        assert_eq!(
            rust.build_command.as_deref(),
            Some("cargo build"),
            "a bare Rust project (no web framework) must still get a build command"
        );
    }

    #[test]
    fn test_detect_go_from_go_mod() {
        let dir = test_dir("go");
        fs::write(dir.join("go.mod"), "module test\ngo 1.21").unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let go = result.iter().find(|t| t.slug == "go");
        assert!(go.is_some(), "go should be detected");
    }

    #[test]
    fn test_detect_version_from_file() {
        let dir = test_dir("node-version");
        fs::write(dir.join("package.json"), "{}").unwrap();
        fs::write(dir.join(".nvmrc"), "20.11.0").unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let node = result.iter().find(|t| t.slug == "node").unwrap();
        assert_eq!(node.version, "20.11.0");
    }

    #[test]
    fn test_detect_nothing_in_empty_dir() {
        let dir = test_dir("empty");

        let result = detect_toolchains(&dir).unwrap();
        assert!(result.is_empty(), "empty dir should detect nothing");
    }

    #[test]
    fn test_detect_python_from_requirements() {
        let dir = test_dir("python");
        fs::write(dir.join("requirements.txt"), "flask\n").unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let py = result.iter().find(|t| t.slug == "python");
        assert!(py.is_some(), "python should be detected");
    }

    #[test]
    fn test_detector_filesystem_caching() {
        let dir = test_dir("fs-cache");
        fs::write(dir.join("test.txt"), "hello").unwrap();

        let root = dir.canonicalize().unwrap();
        let fs = DetectorFilesystem::new(root);
        assert!(fs.is_file("test.txt"));
        assert!(fs.has_path("test.txt"));
        assert_eq!(fs.read_file("test.txt"), Some("hello".to_string()));

        let entries = fs.readdir(".");
        assert!(entries.iter().any(|e| e.name == "test.txt"));
    }

    #[test]
    fn test_detect_npm_framework_react() {
        let dir = test_dir("react");
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies": {"react": "^18.0.0", "react-dom": "^18.0.0"}}"#,
        )
        .unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let node = result.iter().find(|t| t.slug == "node").unwrap();
        assert!(node.frameworks.iter().any(|f| f.name == "React"));
    }

    #[test]
    fn test_detect_cargo_framework_axum() {
        let dir = test_dir("cargo-axum");
        fs::write(
            dir.join("Cargo.toml"),
            "[dependencies]\naxum = \"0.7\"\ntokio = \"1\"",
        )
        .unwrap();

        let result = detect_toolchains(&dir).unwrap();
        let rust = result.iter().find(|t| t.slug == "rust").unwrap();
        assert!(rust.frameworks.iter().any(|f| f.name == "Axum"));
    }

    #[test]
    fn test_framework_scan_npm() {
        let content = r#"{"dependencies": {"next": "^14.0.0", "react": "^18.0.0"}}"#;
        let frames = crate::frameworks::scan_npm(content);
        assert!(frames.iter().any(|f| f.name == "Next.js"));
        assert!(frames.iter().any(|f| f.name == "React"));
    }

    #[test]
    fn test_framework_scan_cargo() {
        let content = "[dependencies]\nleptos = \"0.6\"\nsqlx = { version = \"0.7\", features = [\"runtime-tokio\"] }";
        let frames = crate::frameworks::scan_cargo(content);
        assert!(frames.iter().any(|f| f.name == "Leptos"));
        assert!(frames.iter().any(|f| f.name == "SQLx"));
    }
}
