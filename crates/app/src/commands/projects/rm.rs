use crate::commands::projects::resolve_project_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Delete a project.
///
/// # Arguments
/// * `project` - Project ID or slug to delete
/// * `yes` - Skip confirmation prompt if true
#[instrument(skip_all)]
pub async fn rm(session: AppzSession, project: String, yes: bool) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Resolve project identifier to project ID
    let project_id = resolve_project_id(&client, &project).await?;

    // Get project details for display before deletion
    let project_details = client
        .projects()
        .get(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to get project details: {}", e))?;

    // Show confirmation prompt unless --yes flag is set
    if !yes {
        let project_slug = project_details.slug.as_deref().unwrap_or("unknown");
        let confirm_msg = format!(
            "Are you sure you want to delete project '{}' (ID: {})? This action cannot be undone.",
            project_slug, project_id
        );

        if !confirm(&confirm_msg, false)? {
            println!("Canceled");
            return Ok(None);
        }
    }

    // Delete project
    client
        .projects()
        .delete(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to delete project: {}", e))?;

    // Display success message
    let project_slug = project_details.slug.as_deref().unwrap_or("unknown");
    status::success(&format!(
        "Deleted project '{}' (ID: {})",
        project_slug, project_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
