use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

/// List all aliases the user has access to.
///
/// Displays aliases in a table format with ID, alias, target, domain, and creation timestamp.
#[instrument(skip_all)]
pub async fn ls(session: AppzSession) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // List aliases
    let aliases_response = client
        .aliases()
        .list(None, None, None, None, None)
        .await
        .map_err(|e| {
            let error_msg = match &e {
                api::ApiError::Validation(msg) => format!("Validation error: {}", msg),
                api::ApiError::ApiError { code, message } => {
                    format!("API error {}: {}", code, message)
                }
                _ => format!("Failed to list aliases: {}", e),
            };
            miette::miette!("{}", error_msg)
        })?;

    if aliases_response.aliases.is_empty() {
        ui::empty::display(
            "No aliases found",
            Some("Aliases are created automatically when you deploy projects"),
        )?;
        return Ok(None);
    }

    // Prepare table data
    let aliases = &aliases_response.aliases;
    let headers = vec!["ID", "Alias", "Target", "Domain", "Created"];
    let mut rows = Vec::new();

    for alias in aliases {
        let alias_id = alias.id.to_string();
        let alias_str = alias.alias.clone();
        let target = alias.target.clone();
        let domain = alias.domain.as_deref().unwrap_or("N/A");

        // Format timestamp (handle both seconds and milliseconds)
        let created = format::timestamp_auto(alias.createdAt);

        rows.push(vec![
            alias_id,
            alias_str,
            target,
            domain.to_string(),
            created,
        ]);
    }

    // Display table with professional formatting
    table::display(&headers, &rows, Some("Aliases"))?;

    // Display pagination info if available
    let pagination_info = pagination::PaginationInfo::new(
        aliases_response.pagination.count,
        aliases_response.pagination.next,
        aliases_response.pagination.prev,
    );
    pagination::display(&pagination_info)?;

    Ok(None)
}
