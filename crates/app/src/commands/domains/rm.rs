use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Delete a domain.
///
/// # Arguments
/// * `domain` - Domain name to delete
/// * `yes` - Skip confirmation prompt if true
#[instrument(skip_all)]
pub async fn rm(session: AppzSession, domain: String, yes: bool) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Get current team_id for domain deletion
    let team_id = client.get_team_id().await;

    // Show confirmation prompt unless --yes flag is set
    if !yes {
        let confirm_msg = format!(
            "Are you sure you want to delete domain '{}'? This action cannot be undone.",
            domain
        );

        if !confirm(&confirm_msg, false)? {
            println!("Canceled");
            return Ok(None);
        }
    }

    // Delete domain
    client
        .domains()
        .delete(&domain, team_id)
        .await
        .map_err(|e| miette::miette!("Failed to delete domain: {}", e))?;

    // Display success message
    status::success(&format!("Deleted domain '{}'", domain))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
