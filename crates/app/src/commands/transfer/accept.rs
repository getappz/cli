//! Accept a project transfer request into the current team (Vercel-aligned).

use crate::session::AppzSession;
use crate::ClientExt;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Accept a project transfer request into the current team.
#[instrument(skip_all)]
pub async fn accept(session: AppzSession, code: String) -> AppResult {
    let client = session.get_api_client();

    let team_id = client.get_team_id().await.ok_or_else(|| {
        miette::miette!(
            "Team scope required. Set team with --scope, APPZ_TEAM_ID, or 'appz teams switch'."
        )
    })?;

    let project = client
        .projects()
        .accept_transfer_request(&code)
        .await
        .map_err(|e| miette::miette!("Failed to accept transfer: {}", e))?;

    let project_slug = project.slug.as_deref().unwrap_or("unknown");
    status::success(&format!(
        "Project '{}' transferred to team {}",
        project_slug, team_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
