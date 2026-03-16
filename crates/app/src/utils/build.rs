//! Build output directory detection utilities

use detectors::{detect_framework_record, DetectFrameworkRecordOptions, StdFilesystem};
use frameworks::frameworks;
use miette::{miette, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Detect the build output directory for a project.
///
/// This function attempts to detect the build output directory by:
/// 1. Using an explicit directory if provided
/// 2. Checking framework settings for output_directory
/// 3. Falling back to common output directories (dist, build, .output/public, out, .next)
///
/// # Arguments
/// * `project_path` - The project root directory
/// * `explicit_dir` - Optional explicit directory override
///
/// # Returns
/// The detected build output directory path
///
/// # Errors
/// Returns an error if:
/// - Framework detection fails
/// - The output directory doesn't exist
/// - The output path is not a directory
pub async fn detect_build_output_dir(
    project_path: &Path,
    explicit_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    // Check .appz/output/static first (WordPress static export, custom builds)
    let appz_static = project_path.join(".appz/output/static");
    if explicit_dir.is_none() && appz_static.is_dir() {
        return Ok(appz_static);
    }

    // If explicit directory provided, use it
    if let Some(ref d) = explicit_dir {
        let output_dir = if d.is_absolute() {
            d.clone()
        } else {
            project_path.join(d)
        };

        if !output_dir.exists() {
            return Err(miette!(
                "Build output directory not found: {}\n\nPlease run 'appz build' first to build your project.",
                output_dir.display()
            ));
        }

        if !output_dir.is_dir() {
            return Err(miette!(
                "Output path is not a directory: {}",
                output_dir.display()
            ));
        }

        return Ok(output_dir);
    }

    // Create filesystem detector
    let fs = Arc::new(StdFilesystem::new(Some(project_path.to_path_buf())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    let output_dir = match detect_framework_record(options).await {
        Ok(Some((fw, _version, _package_manager))) => {
            // Try to get from framework settings
            let mut found = None;
            if let Some(settings) = &fw.settings {
                if let Some(output_dir) = &settings.output_directory {
                    if let Some(value) = output_dir.value {
                        let dir = project_path.join(value);
                        if dir.exists() {
                            found = Some(dir);
                        }
                    }
                }
            }

            // Fallback to common output directories if not found in settings
            found.unwrap_or_else(|| {
                let common_dirs = ["dist", "build", ".output/public", "out", ".next"];
                for dir_name in &common_dirs {
                    let dir = project_path.join(dir_name);
                    if dir.exists() {
                        return dir;
                    }
                }
                // Default to dist if nothing found (will error later if doesn't exist)
                project_path.join("dist")
            })
        }
        Ok(None) => {
            // No framework detected, try common directories
            let common_dirs = ["dist", "build", ".output/public", "out"];
            let mut found = None;
            for dir_name in &common_dirs {
                let dir_path = project_path.join(dir_name);
                if dir_path.exists() {
                    found = Some(dir_path);
                    break;
                }
            }
            found.unwrap_or_else(|| project_path.join("dist"))
        }
        Err(e) => {
            return Err(miette!("Error detecting framework: {}", e));
        }
    };

    // Check if output directory exists
    if !output_dir.exists() {
        return Err(miette!(
            "Build output directory not found: {}\n\nPlease run 'appz build' first to build your project.",
            output_dir.display()
        ));
    }

    if !output_dir.is_dir() {
        return Err(miette!(
            "Output path is not a directory: {}",
            output_dir.display()
        ));
    }

    Ok(output_dir)
}

