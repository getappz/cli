use std::path::Path;

use crate::detect::{CommandOverrides, DetectedToolchain};
use crate::frameworks;
use crate::fs::DetectorFilesystem;

/// Toolchain slug for root-level static HTML (no package manager manifest).
pub const STATIC_HTML_SLUG: &str = "static-html";

/// Dev command marker — `appz` runs the embedded static server instead of a shell command.
pub const STATIC_SERVE_DEV_CMD: &str = "__appz_static_serve__";

/// Default listen port for the embedded static server (`appz dev` / `appz preview`).
pub const STATIC_DEV_PORT: u16 = 4321;

/// Whether `dir` contains a servable `index.html` at its root.
pub fn has_index_html(dir: &Path) -> bool {
    dir.is_dir() && dir.join("index.html").is_file()
}

const MANIFEST_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "composer.json",
    "Gemfile",
    "pyproject.toml",
    "requirements.txt",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
];

/// True when the tree is a plain static site: root `index.html` and no known stack manifest.
pub fn looks_like_static_html(fs: &DetectorFilesystem) -> bool {
    if !fs.check_detector("index.html", false, None, None) {
        return false;
    }
    !MANIFEST_MARKERS.iter().any(|p| fs.is_file(p))
}

/// Deploy output directory from detected toolchains (matches `appz deploy` resolution).
pub fn primary_output_directory(toolchains: &[DetectedToolchain]) -> &'static str {
    toolchains
        .iter()
        .find_map(|tc| tc.output_directory)
        .unwrap_or("dist")
}

/// Plain static HTML needs no `mise run build` — output is already on disk.
///
/// `Ok(Some(()))` when this project is static HTML and `index.html` exists at the output root.
/// `Ok(None)` when another toolchain owns the build. `Err` when static but not deployable.
pub fn static_prebuild_ready(
    project: &Path,
    toolchains: &[DetectedToolchain],
) -> Result<Option<()>, String> {
    if !toolchains.iter().any(|tc| tc.slug == STATIC_HTML_SLUG) {
        return Ok(None);
    }
    let out = project.join(primary_output_directory(toolchains));
    if has_index_html(&out) {
        return Ok(Some(()));
    }
    Err(format!(
        "static HTML site has no index.html in '{}' — nothing to deploy",
        primary_output_directory(toolchains)
    ))
}

pub fn detected_static_html(overrides: &CommandOverrides) -> DetectedToolchain {
    let dev_cmd = overrides
        .dev_command
        .clone()
        .unwrap_or_else(|| STATIC_SERVE_DEV_CMD.to_string());

    DetectedToolchain {
        name: "Static HTML",
        slug: STATIC_HTML_SLUG,
        mise_plugin: STATIC_HTML_SLUG,
        version: String::new(),
        version_files: &[],
        frameworks: vec![frameworks::DetectedFramework {
            name: "Static HTML",
            ecosystem: "static",
        }],
        build_command: overrides.build_command.clone(),
        install_command: overrides.install_command.clone(),
        dev_command: Some(dev_cmd),
        test_command: None,
        lint_command: None,
        format_command: None,
        format_tool: None,
        output_directory: Some("."),
        env_prefix: None,
    }
}

/// Root directory to serve for `appz preview` (build output or static tree).
pub fn resolve_preview_root(
    project: &Path,
    toolchains: &[DetectedToolchain],
    override_dir: Option<&Path>,
) -> Result<std::path::PathBuf, String> {
    if let Some(dir) = override_dir {
        let dir = dir.to_path_buf();
        if !dir.is_dir() {
            return Err(format!("not a directory: {}", dir.display()));
        }
        if !has_index_html(&dir) {
            return Err(format!(
                "no index.html in {} — pass a directory with a built site",
                dir.display()
            ));
        }
        return Ok(dir);
    }

    let mut candidates: Vec<String> = Vec::new();
    let mut push = |rel: &str| {
        if !candidates.iter().any(|c| c == rel) {
            candidates.push(rel.to_string());
        }
    };
    for tc in toolchains {
        if let Some(rel) = tc.output_directory {
            push(rel);
        }
    }
    for rel in ["dist", "out", "public", "build"] {
        push(rel);
    }
    // Plain static HTML has output `.`; preview still allows it, but only after build dirs.
    push(".");

    candidates.sort_by(|a, b| {
        use std::cmp::Ordering;
        match (a.as_str() == ".", b.as_str() == ".") {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => Ordering::Equal,
        }
    });

    for rel in candidates {
        let candidate = project.join(&rel);
        if has_index_html(&candidate) {
            return Ok(candidate);
        }
    }

    Err("no serveable output found — run `appz build` or pass `--serve-dir <path>`".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::detect_toolchains;
    use std::fs;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join("appz-core-test").join(name);
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn detects_static_html_without_manifest() {
        let dir = test_dir("static-html");
        fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        let toolchains = detect_toolchains(&dir).unwrap();
        assert_eq!(toolchains.len(), 1);
        assert_eq!(toolchains[0].slug, STATIC_HTML_SLUG);
        assert_eq!(
            toolchains[0].dev_command.as_deref(),
            Some(STATIC_SERVE_DEV_CMD)
        );
    }

    #[test]
    fn package_json_blocks_static_html_detection() {
        let dir = test_dir("static-with-pkg");
        fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        fs::write(dir.join("package.json"), r#"{"name":"x"}"#).unwrap();
        let toolchains = detect_toolchains(&dir).unwrap();
        assert!(!toolchains.iter().any(|t| t.slug == STATIC_HTML_SLUG));
    }

    #[test]
    fn static_prebuild_skips_mise_build() {
        let dir = test_dir("static-prebuild");
        fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
        let toolchains = detect_toolchains(&dir).unwrap();
        assert_eq!(primary_output_directory(&toolchains), ".");
        assert!(static_prebuild_ready(&dir, &toolchains).unwrap().is_some());
    }
    #[test]
    fn resolve_preview_root_prefers_dist_over_project_root() {
        let dir = test_dir("preview-dist");
        fs::write(dir.join("index.html"), "source").unwrap();
        fs::create_dir_all(dir.join("dist")).unwrap();
        fs::write(dir.join("dist/index.html"), "built").unwrap();
        let toolchains = detect_toolchains(&dir).unwrap();
        let root = resolve_preview_root(&dir, &toolchains, None).unwrap();
        assert_eq!(root, dir.join("dist"));
    }

}