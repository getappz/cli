use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Invite a user to the current team.
///
/// # Arguments
/// * `email` - Email address of the user to invite
/// * `role` - Optional role ID for the invitation
#[instrument(skip_all)]
pub async fn invite(session: AppzSession, email: String, role: Option<i64>) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Get current team ID from client
    let team_id = client.get_team_id().await.ok_or_else(|| {
        miette::miette!("No team selected. Please set one with 'teams switch <team>'")
    })?;

    // Create invitation
    let _invitation = client
        .teams()
        .create_invitation(team_id.as_str(), email.clone(), role)
        .await
        .map_err(|e| miette::miette!("Failed to create invitation: {}", e))?;

    // Display success message
    status::success(&format!("Invited {} to team {}", email, team_id))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    if let Some(role_id) = role {
        status::info(&format!("Role ID: {}", role_id))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    Ok(None)
}
