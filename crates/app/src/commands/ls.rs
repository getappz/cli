use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

#[instrument(skip_all)]
pub async fn ls(session: AppzSession) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Get project context (should already be loaded in analyze phase)
    let project_context = session
        .get_project_context()
        .ok_or_else(|| miette::miette!("Project context not available - this should not happen"))?;

    let project_id = project_context.link.project_id.clone();

    // List deployments for this project
    let deployments_response = client
        .deployments()
        .list(Some(project_id), None, None, None, None)
        .await
        .map_err(|e| miette::miette!("Failed to list deployments: {}", e))?;

    if deployments_response.deployments.is_empty() {
        ui::empty::display(
            "No deployments found",
            Some("Try creating a deployment first"),
        )?;
        return Ok(None);
    }

    // Prepare table data
    let headers = vec!["ID", "Status", "Project ID", "Created"];
    let mut rows = Vec::new();

    for deployment in &deployments_response.deployments {
        let status = deployment.status.as_deref().unwrap_or("unknown");
        let status_badge = format::status_badge(status);
        let project_id = deployment.projectId.as_deref().unwrap_or("N/A");

        // Format timestamp
        let created = format::timestamp(deployment.createdAt);

        rows.push(vec![
            deployment.id.clone(),
            status_badge,
            project_id.to_string(),
            created,
        ]);
    }

    // Display table with professional formatting
    table::display(&headers, &rows, Some("Deployments"))?;

    // Display pagination info if available
    let pagination_info = pagination::PaginationInfo::new(
        deployments_response.pagination.count,
        deployments_response.pagination.next,
        deployments_response.pagination.prev,
    );
    pagination::display(&pagination_info)?;

    Ok(None)
}
