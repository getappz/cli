use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

/// List all domains the user has access to.
///
/// Displays domains in a table format with ID, name, team ID, and creation timestamp.
#[instrument(skip_all)]
pub async fn ls(session: AppzSession) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // List domains
    let domains_response = client
        .domains()
        .list(None, None, None, None)
        .await
        .map_err(|e| {
            let error_msg = match &e {
                api::ApiError::Validation(msg) => format!("Validation error: {}", msg),
                api::ApiError::ApiError { code, message } => {
                    format!("API error {}: {}", code, message)
                }
                _ => format!("Failed to list domains: {}", e),
            };
            miette::miette!("{}", error_msg)
        })?;

    if domains_response.domains.is_empty() {
        ui::empty::display("No domains found", Some("Try adding a domain first"))?;
        return Ok(None);
    }

    // Prepare table data
    let domains = &domains_response.domains;
    let headers = vec!["ID", "Name", "Team ID", "Created"];
    let mut rows = Vec::new();

    for domain in domains {
        let domain_id = domain.id.clone();
        let name = domain.name.clone();
        let team_id = domain.teamId.clone();

        // Format timestamp (handle both seconds and milliseconds)
        let created = format::timestamp_auto(domain.createdAt);

        rows.push(vec![domain_id, name, team_id, created]);
    }

    // Display table with professional formatting
    table::display(&headers, &rows, Some("Domains"))?;

    // Display pagination info if available
    let pagination_info = pagination::PaginationInfo::new(
        domains_response.pagination.count,
        domains_response.pagination.next,
        domains_response.pagination.prev,
    );
    pagination::display(&pagination_info)?;

    Ok(None)
}
