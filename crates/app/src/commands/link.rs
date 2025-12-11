use crate::project::setup_and_link;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Link the current directory to a project.
///
/// This creates a `.appz/project.json` file that stores the project link
/// and settings, allowing commands like `appz ls` to filter by project.
#[instrument(skip_all)]
pub async fn link(
    session: AppzSession,
    project: Option<String>,
    _team: Option<String>,
) -> AppResult {
    let client = session.get_api_client();
    let cwd = &session.working_dir;

    // Check if already linked
    if crate::project::is_project_linked(cwd) {
        status::warning("This directory is already linked to a project.")
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
        status::info("Run 'appz unlink' to remove the link first.")
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
        return Ok(None);
    }

    // Use setup_and_link for Vercel-style flow
    // Note: project and team parameters are handled within setup_and_link via prompts
    setup_and_link(&client, cwd, false, Some("Link"), project.as_deref())
        .await
        .map_err(|e| miette::miette!("Failed to link project: {}", e))?;

    Ok(None)
}
