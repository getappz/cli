use crate::commands::projects::user_friendly_list_projects_error;
use crate::session::AppzSession;
use crate::ClientExt;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

/// List all projects the user has access to.
///
/// Displays projects in a table format with ID, slug, name, team ID, and creation timestamp.
#[instrument(skip_all)]
pub async fn ls(session: AppzSession) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // List projects
    let projects_response = client
        .projects()
        .list(None, None, None)
        .await
        .map_err(|e| miette::miette!("{}", user_friendly_list_projects_error(&e)))?;

    if projects_response.projects.is_empty() {
        ui::empty::display(
            "No projects found",
            Some("Try creating a project first with 'appz project add <name>'"),
        )?;
        return Ok(None);
    }

    // Prepare table data
    let projects = &projects_response.projects;
    let headers = vec!["ID", "Slug", "Name", "Team ID", "Created"];
    let mut rows = Vec::new();

    for project in projects {
        let project_id = project.id.as_deref().unwrap_or("N/A");
        let slug = project.slug.as_deref().unwrap_or("N/A");
        let name = project.name.as_deref().unwrap_or("N/A");
        let team_id = project.teamId.as_deref().unwrap_or("N/A");

        // Format timestamp (handle both seconds and milliseconds)
        let created = project
            .createdAt
            .map(format::timestamp_auto)
            .unwrap_or_else(|| "N/A".to_string());

        rows.push(vec![
            project_id.to_string(),
            slug.to_string(),
            name.to_string(),
            team_id.to_string(),
            created,
        ]);
    }

    // Display table with professional formatting
    table::display(&headers, &rows, Some("Projects"))?;

    // Display pagination info if available
    if let Some(ref pag) = projects_response.pagination {
        let pagination_info = pagination::PaginationInfo::new(pag.count, pag.next, pag.prev);
        pagination::display(&pagination_info)?;
    }

    Ok(None)
}
