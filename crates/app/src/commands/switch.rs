use crate::auth;
use crate::commands::teams::resolve_team_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Switch the active team context.
///
/// This command sets the active team for subsequent API requests.
/// The team context is persisted to `~/.appz/auth.json` and will be
/// automatically loaded on future CLI runs.
///
/// # Arguments
/// * `team` - Team ID or slug to switch to
#[instrument(skip_all)]
pub async fn switch(session: AppzSession, team: String) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Resolve team identifier to team ID
    let team_id = resolve_team_id(&client, &team).await?;

    // Get team details for display
    let team_details = client
        .teams()
        .get(&team_id)
        .await
        .map_err(|e| miette::miette!("Failed to get team details: {}", e))?;

    // Save team_id to auth.json config file
    auth::save_team_id(team_id.clone())
        .map_err(|e| miette::miette!("Failed to save team context: {}", e))?;

    // Set team_id on current API client session
    client.set_team_id(Some(team_id.clone())).await;

    // Display success message
    status::success(&format!(
        "Switched to team '{}' (ID: {})",
        team_details.slug, team_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    if let Some(ref team_name) = team_details.name {
        status::info(&format!("Name: {}", team_name))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    Ok(None)
}
