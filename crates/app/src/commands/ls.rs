use api::error::ApiError as ApiErrorType;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

#[instrument(skip_all)]
pub async fn ls(session: AppzSession) -> AppResult {
    let client = session.get_api_client();

    let project_context = session
        .get_project_context()
        .ok_or_else(|| miette::miette!("Project context not available - this should not happen"))?;

    let project_id = project_context.link.project_id.clone();

    // Match Vercel: set team scope before fetch (use --scope if set, else project's team)
    if client.get_team_id().await.is_none() {
        client
            .set_team_id(Some(project_context.link.team_id.clone()))
            .await;
    }

    let deployments_response = match client
        .deployments()
        .list(Some(project_id), None, None, None, None)
        .await
    {
        Ok(r) => r,
        Err(ApiErrorType::Forbidden(_)) => {
            return Err(miette::miette!(
                "Access denied: this project belongs to a different team. \
                 Switch to the project's team or use --scope to list deployments."
            ));
        }
        Err(e) => return Err(miette::miette!("Failed to list deployments: {}", e)),
    };

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
