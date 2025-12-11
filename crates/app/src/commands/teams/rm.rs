use crate::commands::teams::resolve_team_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Delete a team.
///
/// # Arguments
/// * `team` - Team ID or slug to delete
/// * `yes` - Skip confirmation prompt if true
#[instrument(skip_all)]
pub async fn rm(session: AppzSession, team: String, yes: bool) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Resolve team identifier to team ID
    let team_id = resolve_team_id(&client, &team).await?;

    // Get team details for display before deletion
    let team_details = client
        .teams()
        .get(&team_id)
        .await
        .map_err(|e| miette::miette!("Failed to get team details: {}", e))?;

    // Show confirmation prompt unless --yes flag is set
    if !yes {
        let team_name = team_details.name.as_deref().unwrap_or(&team_details.slug);
        let confirm_msg = format!(
            "Are you sure you want to delete team '{}' (ID: {})? This action cannot be undone.",
            team_name, team_id
        );

        if !confirm(&confirm_msg, false)? {
            println!("Canceled");
            return Ok(None);
        }
    }

    // Delete team
    client
        .teams()
        .delete(&team_id)
        .await
        .map_err(|e| miette::miette!("Failed to delete team: {}", e))?;

    // Display success message
    let team_name = team_details.name.as_deref().unwrap_or(&team_details.slug);
    status::success(&format!("Deleted team '{}' (ID: {})", team_name, team_id))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
