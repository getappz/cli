use crate::commands::aliases::resolve_alias_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Delete an alias.
///
/// # Arguments
/// * `alias` - Alias ID or alias string to delete
/// * `yes` - Skip confirmation prompt if true
#[instrument(skip_all)]
pub async fn rm(session: AppzSession, alias: String, yes: bool) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Resolve alias identifier to alias ID
    let alias_id = resolve_alias_id(&client, &alias).await?;

    // Get alias details for display before deletion
    let alias_details = client
        .aliases()
        .get(&alias_id.to_string())
        .await
        .map_err(|e| miette::miette!("Failed to get alias details: {}", e))?;

    // Show confirmation prompt unless --yes flag is set
    if !yes {
        let confirm_msg = format!(
            "Are you sure you want to delete alias '{}' (ID: {})? This action cannot be undone.",
            alias_details.alias, alias_id
        );

        if !confirm(&confirm_msg, false)? {
            println!("Canceled");
            return Ok(None);
        }
    }

    // Delete alias
    client
        .aliases()
        .delete(&alias_id.to_string())
        .await
        .map_err(|e| miette::miette!("Failed to delete alias: {}", e))?;

    // Display success message
    status::success(&format!(
        "Deleted alias '{}' (ID: {})",
        alias_details.alias, alias_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
