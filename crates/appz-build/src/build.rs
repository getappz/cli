//! Build execution via sandbox.
//!
//! Runs install and build commands, validates output directory.
//! Ports `validateDistDir` logic from Vercel static-build.

// Rust guideline compliant 2026-02-18

use crate::detect::DetectedFramework;
use miette::{miette, Result};
use sandbox::SandboxProvider;
use std::path::Path;

/// Run install command in the sandbox.
///
/// # Errors
///
/// Command execution failures.
pub async fn run_install(
    sandbox: &dyn SandboxProvider,
    framework: &DetectedFramework,
) -> Result<()> {
    sandbox
        .exec_interactive(&framework.install_command)
        .await
        .map_err(|e| miette!("Install failed: {}", e))?;
    Ok(())
}

/// Run build command in the sandbox.
///
/// # Errors
///
/// Command execution failures.
pub async fn run_build(sandbox: &dyn SandboxProvider, framework: &DetectedFramework) -> Result<()> {
    sandbox
        .exec_interactive(&framework.build_command)
        .await
        .map_err(|e| miette!("Build failed: {}", e))?;
    Ok(())
}

/// Validate that the output directory exists and is non-empty.
///
/// Ports `validateDistDir` from Vercel static-build. Ensures the build
/// produced usable output before proceeding to deployment.
///
/// # Errors
///
/// - Directory does not exist
/// - Path is not a directory
/// - Directory is empty
pub fn validate_output_dir(output_path: &Path) -> Result<()> {
    if !output_path.exists() {
        return Err(miette!(
            "No output directory found at \"{}\" after the build completed. \
             Configure outputDirectory in appz.json or ensure your build command produces output there.",
            output_path.display()
        ));
    }

    if !output_path.is_dir() {
        return Err(miette!(
            "The path \"{}\" is not a directory.",
            output_path.display()
        ));
    }

    let is_empty = std::fs::read_dir(output_path)
        .map_err(|e| miette!("Failed to read output directory: {}", e))?
        .next()
        .is_none();

    if is_empty {
        return Err(miette!(
            "Output directory \"{}\" is empty after the build.",
            output_path.display()
        ));
    }

    Ok(())
}
