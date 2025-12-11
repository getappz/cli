use crate::auth;
use crate::session::AppzSession;
use api::Client;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Log in to your Appz account.
///
/// This command uses OAuth 2.0 Device Authorization Flow (RFC 8628).
///
/// Device Flow:
/// 1. Requests device authorization
/// 2. Displays a user code and opens browser
/// 3. User authorizes in browser
/// 4. CLI polls for and receives authentication token
#[instrument(skip_all)]
pub async fn login(_session: AppzSession) -> AppResult {
    // Create a temporary unauthenticated client for login
    let client =
        Client::new().map_err(|e| miette::miette!("Failed to create API client: {}", e))?;

    // Check if user is already logged in
    if let Ok(auth_config) = auth::load_auth() {
        if auth_config.has_token() && !auth_config.is_token_expired() {
            status::info("You are already logged in. Use `logout` to sign out first.")
                .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
            return Ok(None);
        }
    }

    // Use OAuth Device Flow
    match auth::device_flow_login(&client).await {
        Ok(_token) => {
            println!();
            println!("You are now signed in. You can start using authenticated commands.");
            println!("To deploy something, run `appz ls` to see your deployments.");
            Ok(None)
        }
        Err(e) => Err(miette::miette!("Login failed: {}", e)),
    }
}
