use crate::project::remove_project_link;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Unlink the current directory from a project.
///
/// This removes the `.appz/project.json` file.
#[instrument(skip_all)]
pub async fn unlink(session: AppzSession) -> AppResult {
    let cwd = &session.working_dir;

    if !crate::project::is_project_linked(cwd) {
        status::warning("This directory is not linked to any project.")
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
        return Ok(None);
    }

    remove_project_link(cwd)
        .map_err(|e| miette::miette!("Failed to remove project link: {}", e))?;

    status::success("Unlinked project")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    status::info("Removed .appz/project.json")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
