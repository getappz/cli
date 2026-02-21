//! Whoami command – show the username of the currently logged-in user (Vercel-aligned).

use crate::session::AppzSession;
use crate::ClientExt;
use starbase::AppResult;
use tracing::instrument;

/// Show the username of the currently logged-in user.
///
/// Outputs username by default. Use `--format json` for full user object
/// (username, email, name) – suitable for piping or scripting.
#[instrument(skip_all)]
pub async fn whoami(session: AppzSession, json_format: bool) -> AppResult {
    let client = session.get_api_client();

    let user = client
        .users()
        .get_current()
        .await
        .map_err(|e| miette::miette!("Failed to get current user: {}", e))?;

    if json_format {
        let output = serde_json::json!({
            "username": user.username,
            "email": user.email,
            "name": user.name
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    } else {
        // Show username (Vercel: contextName – username or email)
        let display = user.username.trim();
        if display.is_empty() {
            println!("{}", user.email.as_deref().unwrap_or("unknown"));
        } else {
            println!("{display}");
        }
    }

    Ok(None)
}
