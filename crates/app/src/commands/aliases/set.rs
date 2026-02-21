//! Assign a custom domain to a deployment (Vercel parity: `alias set`).
//!
//! Usage: `appz alias set [deployment-url] [custom-domain]`
//! Do not include the HTTP protocol (e.g. `https://`) for the custom-domain.

use crate::session::AppzSession;
use crate::ClientExt;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Strip protocol from deployment URL or custom domain if present.
fn strip_protocol(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with("https://") {
        &s[8..]
    } else if s.starts_with("http://") {
        &s[7..]
    } else {
        s
    }
}

/// Validate hostname: non-empty, no spaces, not just slashes.
fn is_valid_hostname(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && !s.chars().all(|c| c == '/' || c == ' ') && s.len() < 256
}

/// Assign a custom domain to a deployment.
///
/// Vercel parity: `vercel alias set [deployment-url] [custom-domain]`.
#[instrument(skip_all)]
pub async fn set(
    session: AppzSession,
    deployment_url: String,
    custom_domain: String,
) -> AppResult {
    let client = session.get_api_client();

    let deployment_ref = strip_protocol(&deployment_url).to_string();
    let alias_hostname = strip_protocol(&custom_domain).to_string();

    if !is_valid_hostname(&deployment_ref) {
        return Err(miette::miette!(
            "Invalid deployment: \"{}\". Provide a deployment URL or ID.",
            deployment_url
        )
        .into());
    }

    if !is_valid_hostname(&alias_hostname) {
        return Err(miette::miette!(
            "Invalid domain: \"{}\". Do not include the protocol (https://).",
            custom_domain
        )
        .into());
    }

    // Resolve deployment (by URL or ID)
    let deployment = client
        .deployments()
        .get(&deployment_ref)
        .await
        .map_err(|e| {
            let msg = match &e {
                api::ApiError::ApiError { code, message } => {
                    format!("Deployment not found: {}", message)
                }
                api::ApiError::Validation(m) => format!("Validation error: {}", m),
                _ => format!("Failed to get deployment: {}", e),
            };
            miette::miette!("{}", msg)
        })?;

    status::info(&format!(
        "Assigning alias {} to deployment {}",
        alias_hostname,
        deployment.url.as_deref().unwrap_or(&deployment.id)
    ))
    .map_err(|e| miette::miette!("{}", e))?;

    let result = client
        .aliases()
        .create(&deployment.id, &alias_hostname)
        .await
        .map_err(|e| {
            let msg = match &e {
                api::ApiError::ApiError { code, message } => {
                    format!("Failed to create alias: {}", message)
                }
                api::ApiError::Validation(m) => format!("Validation error: {}", m),
                _ => format!("Failed to create alias: {}", e),
            };
            miette::miette!("{}", msg)
        })?;

    status::success(&format!(
        "{} now points to {}",
        result.alias,
        deployment.url.as_deref().unwrap_or(&deployment.id)
    ))
    .map_err(|e| miette::miette!("{}", e))?;

    Ok(None)
}
