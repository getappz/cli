//! Framework detection for build pipeline.
//!
//! Wraps [`detectors::detect_framework_record`] and returns a
//! [`DetectedFramework`] with build/install/output configuration.

// Rust guideline compliant 2026-02-18

use detectors::{
    detect_framework_record, DetectFrameworkRecordOptions, DetectorFilesystem, StdFilesystem,
};
use frameworks::frameworks;
use miette::{miette, Result};
use std::path::Path;
use std::sync::Arc;

/// Detected framework with build configuration.
///
/// Extracted from detectors + frameworks for use by the build pipeline.
#[derive(Debug, Clone)]
pub struct DetectedFramework {
    /// Framework display name (e.g. "Next.js")
    pub name: String,
    /// Framework slug (e.g. "nextjs")
    pub slug: Option<String>,
    /// Install command (user script, framework default, or package manager default)
    pub install_command: String,
    /// Build command (user script or framework default)
    pub build_command: String,
    /// Output directory relative to project root (e.g. "dist", "build", "out")
    pub output_directory: String,
    /// Package manager info (for sandbox tool selection)
    pub package_manager: Option<detectors::PackageManagerInfo>,
}

/// Detect framework in project root.
///
/// Uses fs-detectors–style logic via [`detect_framework_record`].
/// Returns build config or `None` when no framework is detected.
///
/// # Errors
///
/// I/O or detection errors.
pub async fn detect_framework(project_root: &Path) -> Result<Option<DetectedFramework>> {
    let fs: Arc<dyn DetectorFilesystem> =
        Arc::new(StdFilesystem::new(Some(project_root.to_path_buf())));

    let framework_list: Vec<_> = frameworks().to_vec();
    let options = DetectFrameworkRecordOptions {
        fs,
        framework_list,
    };

    let (framework, _version, package_manager) = match detect_framework_record(options).await {
        Ok(Some(t)) => t,
        Ok(None) => return Ok(None),
        Err(e) => return Err(miette!("Framework detection failed: {}", e)),
    };

    // Resolve install command
    let user_install = package_manager
        .as_ref()
        .and_then(|pm| pm.install_script.clone());
    let framework_install = framework
        .settings
        .as_ref()
        .and_then(|s| s.install_command.as_ref())
        .and_then(|c| c.value)
        .map(|s| s.to_string());
    let install_command = user_install
        .or(framework_install)
        .unwrap_or_else(|| default_install_command(&package_manager));

    // Resolve build command
    let user_build = package_manager
        .as_ref()
        .and_then(|pm| pm.build_script.clone());
    let framework_build = framework
        .settings
        .as_ref()
        .and_then(|s| s.build_command.as_ref())
        .and_then(|c| c.value)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            miette!(
                "No build command configured for framework: {}",
                framework.name
            )
        })?;
    let build_command = user_build.unwrap_or(framework_build);

    // Resolve output directory (framework default, then common fallbacks)
    let output_directory = framework
        .settings
        .as_ref()
        .and_then(|s| s.output_directory.as_ref())
        .and_then(|c| c.value)
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_output_directory(framework.slug));

    Ok(Some(DetectedFramework {
        name: framework.name.to_string(),
        slug: framework.slug.map(|s| s.to_string()),
        install_command,
        build_command,
        output_directory,
        package_manager: package_manager.clone(),
    }))
}

fn default_install_command(pm: &Option<detectors::PackageManagerInfo>) -> String {
    match pm.as_ref().map(|p| p.manager.as_str()) {
        Some("yarn") => "yarn install".to_string(),
        Some("pnpm") => "pnpm install".to_string(),
        Some("bun") => "bun install".to_string(),
        _ => "npm install".to_string(),
    }
}

/// Common output directories per framework slug (Vercel-style).
fn default_output_directory(slug: Option<&str>) -> String {
    match slug {
        Some("nextjs") => "dist".to_string(),
        Some("gatsby") => "public".to_string(),
        Some("hugo") | Some("jekyll") => "public".to_string(),
        Some("astro") | Some("sveltekit") => "dist".to_string(),
        Some("nuxt") | Some("vue") => "dist".to_string(),
        Some("docusaurus") => "build".to_string(),
        _ => "dist".to_string(),
    }
}
