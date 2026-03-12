//! Create a project transfer request (Vercel-aligned).

use crate::commands::projects::resolve_project_id;
use crate::commands::teams::resolve_team_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Create a project transfer request.
/// When `to_team` is provided, performs direct transfer (create + accept).
#[instrument(skip_all)]
pub async fn create(
    session: AppzSession,
    project: &str,
    to_team: Option<&str>,
) -> AppResult {
    let client = session.get_api_client();

    let project_id = resolve_project_id(&client, project).await?;

    let resp = client
        .projects()
        .create_transfer_request(&project_id, None)
        .await
        .map_err(|e| miette::miette!("Failed to create transfer request: {}", e))?;

    if let Some(team_ref) = to_team {
        let target_team_id = resolve_team_id(&client, team_ref).await?;
        let prev_team = client.get_team_id().await;
        client.set_team_id(Some(target_team_id.clone())).await;
        let transferred = client
            .projects()
            .accept_transfer_request(&resp.code)
            .await;
        client.set_team_id(prev_team).await;

        if let Ok(proj) = transferred {
            let slug = proj.slug.as_deref().unwrap_or("unknown");
            status::success(&format!(
                "Project '{}' transferred to team {}",
                slug, target_team_id
            ))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
            return Ok(None);
        }

        return Err(miette::miette!(
            "Created transfer code but failed to accept. Use: appz transfer accept {}",
            resp.code
        ));
    }

    status::success("Transfer request created. Share this code with the receiving team.")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    status::info(&format!("Code: {}", resp.code))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    status::info(&format!(
        "Code expires in 24 hours. Use 'appz transfer accept {}' to accept.",
        resp.code
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
