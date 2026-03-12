//! Standardized build output (.appz/output).
//!
//! Produces Build Output v3–style layout for deployment. Ports from
//! Vercel's build-output-v3 and static-build.

// Rust guideline compliant 2026-02-18

use miette::{miette, Result};
use std::fs;
use std::path::Path;

/// Default output directory name for Appz (Build Output v3 style).
pub const APPZ_OUTPUT_DIR: &str = ".appz/output";

/// Resolve output directory: appz.json outputDirectory override, else framework default.
pub fn resolve_build_output_dir(
    project_root: &Path,
    framework_default: &str,
) -> std::path::PathBuf {
    if let Ok(root) =
        starbase_utils::json::read_file::<serde_json::Value>(project_root.join("appz.json"))
    {
        if let Some(dir) = root.get("outputDirectory").and_then(|v| v.as_str()) {
            return project_root.join(dir);
        }
    }
    project_root.join(framework_default)
}

/// Produce standardized output at `.appz/output/` from build artifacts.
///
/// Copies `src_dir` into `.appz/output/static/` and writes `config.json`
/// with version and routes. Call after a successful build.
///
/// # Errors
///
/// I/O and JSON serialization failures.
pub fn produce_standardized_output(
    project_root: &Path,
    src_dir: &Path,
) -> Result<std::path::PathBuf> {
    let output_root = project_root.join(APPZ_OUTPUT_DIR);
    let static_dir = output_root.join("static");

    fs::create_dir_all(&static_dir)
        .map_err(|e| miette!("Failed to create {}: {}", static_dir.display(), e))?;

    copy_dir_all(src_dir, &static_dir)
        .map_err(|e| miette!("Failed to copy build output: {}", e))?;

    let config = serde_json::json!({
        "version": 3,
        "routes": [
            { "handle": "error" },
            { "status": 404, "src": r"^(?!/api).*$", "dest": "/404.html" }
        ],
        "crons": []
    });

    let config_path = output_root.join("config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| miette!("Failed to serialize config.json: {}", e))?;
    fs::write(&config_path, json)
        .map_err(|e| miette!("Failed to write config.json: {}", e))?;

    Ok(output_root)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
