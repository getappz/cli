//! Input root directory for monorepo support
//!
//! Matches Vercel's input-root-directory.ts functionality

use miette::{miette, Result};
use std::path::{Path, PathBuf};
use tracing::instrument;

/// Validate root directory (async version using spawn_blocking)
async fn validate_root_directory(cwd: PathBuf, root_dir: PathBuf) -> Result<bool> {
    let full_path = cwd.join(&root_dir);

    // Use spawn_blocking for file I/O in async context (following workspace rules)
    let full_path_clone = full_path.clone();
    let exists = tokio::task::spawn_blocking(move || full_path_clone.exists())
        .await
        .map_err(|e| miette!("Failed to check file existence: {}", e))?;

    if !exists {
        return Err(miette!("Directory does not exist: {}", full_path.display()));
    }

    let full_path_clone = full_path.clone();
    let is_dir = tokio::task::spawn_blocking(move || full_path_clone.is_dir())
        .await
        .map_err(|e| miette!("Failed to check if path is directory: {}", e))?;

    if !is_dir {
        return Err(miette!("Path is not a directory: {}", full_path.display()));
    }

    // Check that root directory is within cwd (this is a path operation, no I/O needed)
    if !full_path.starts_with(&cwd) {
        return Err(miette!(
            "Root directory must be within the project directory"
        ));
    }

    Ok(true)
}

/// Input root directory for monorepo support
///
/// Prompts user for the directory where code is located.
/// Returns None if root (current directory) or empty.
#[instrument(skip(cwd))]
pub async fn input_root_directory(cwd: PathBuf, auto_confirm: bool) -> Result<Option<String>> {
    if auto_confirm {
        return Ok(None);
    }

    loop {
        // Prompt for root directory
        // Note: The transformer in inquire is display-only, so we'll just prompt normally
        use ui::prompt::prompt;
        let root_directory = prompt("In which directory is your code located?", None)?;

        if root_directory.is_empty() {
            return Ok(None);
        }

        // Normalize path (remove . and .. components)
        let normalized_path = PathBuf::from(&root_directory);
        let normalized = normalized_path.to_string_lossy().to_string();

        if normalized == "." || normalized == "./" || normalized.is_empty() {
            return Ok(None);
        }

        // Validate the directory
        match validate_root_directory(cwd.clone(), PathBuf::from(&root_directory)).await {
            Ok(_) => return Ok(Some(root_directory)),
            Err(e) => {
                tracing::warn!("{} Please choose a different one.", e);
                continue;
            }
        }
    }
}
