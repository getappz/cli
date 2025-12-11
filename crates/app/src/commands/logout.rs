use crate::auth;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Log out and clear the authentication token.
///
/// This command removes the stored authentication token from `~/.appz/auth.json`
/// and clears it from the current session. You will need to log in again to
/// use authenticated commands.
#[instrument(skip_all)]
pub async fn logout(session: AppzSession) -> AppResult {
    // Clear token from auth.json config file
    auth::clear_token()
        .map_err(|e| miette::miette!("Failed to clear authentication token: {}", e))?;

    // Clear token from current API client session
    let client = session.get_api_client();
    client.clear_token().await;

    // Display success message
    status::success("Logged out successfully")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
