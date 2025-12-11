//! Connect git repository to project
//!
//! Matches Vercel's connect-git functionality (simplified)

use miette::{miette, Result};
use starbase_utils::fs;
use std::path::Path;
use tracing::instrument;
use ui::prompt::confirm;

/// Connect git repository to project
///
/// Detects .git/config and prompts to connect if found.
/// This is a simplified version - actual git connection would require API support.
#[instrument(skip(project_path))]
pub async fn connect_git_repository(project_path: &Path, auto_confirm: bool) -> Result<()> {
    let git_config_path = project_path.join(".git").join("config");

    // Use spawn_blocking for file I/O in async context (following workspace rules)
    let git_config_path_clone = git_config_path.clone();
    let exists = tokio::task::spawn_blocking(move || git_config_path_clone.exists())
        .await
        .map_err(|e| miette!("Failed to check file existence: {}", e))?;

    if !exists {
        // No git repository found
        return Ok(());
    }

    // Check if there are any remotes (simplified check)
    let git_config_path_clone = git_config_path.clone();
    let git_config_content = tokio::task::spawn_blocking(move || {
        fs::read_file(&git_config_path_clone)
            .map_err(|e| miette!("Failed to read .git/config: {}", e))
    })
    .await
    .map_err(|e| miette!("Failed to read file: {}", e))??;

    // Simple check for remote URLs
    let has_remotes = git_config_content.contains("[remote");

    if !has_remotes {
        return Ok(());
    }

    // Prompt to connect
    if !auto_confirm {
        let should_connect = confirm("Detected a repository. Connect it to this project?", true)?;
        if !should_connect {
            return Ok(());
        }
    }

    // TODO: Actually connect the repository via API
    // For now, just acknowledge
    tracing::info!("Repository connection would be performed here (API integration needed)");

    Ok(())
}
