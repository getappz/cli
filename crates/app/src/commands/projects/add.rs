use crate::commands::teams::resolve_team_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Create a new project.
///
/// # Arguments
/// * `slug` - Unique project identifier (slug)
/// * `name` - Optional display name for the project
/// * `team` - Optional team ID or slug to associate the project with
#[instrument(skip_all)]
pub async fn add(
    session: AppzSession,
    slug: String,
    name: Option<String>,
    team: Option<String>,
) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Resolve team identifier to team ID if provided
    let team_id = if let Some(ref team_identifier) = team {
        Some(resolve_team_id(&client, team_identifier).await?)
    } else {
        None
    };

    // Create project
    let project = client
        .projects()
        .create(slug.clone(), name.clone(), team_id.clone())
        .await
        .map_err(|e| miette::miette!("Failed to create project: {}", e))?;

    // Display success message
    let project_slug = project.slug.as_deref().unwrap_or("unknown");
    let project_id = project.id.as_deref().unwrap_or("unknown");
    status::success(&format!(
        "Created project '{}' (ID: {})",
        project_slug, project_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    if let Some(ref project_name) = project.name {
        status::info(&format!("Name: {}", project_name))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    if let Some(ref team_id) = project.teamId {
        status::info(&format!("Team ID: {}", team_id))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    Ok(None)
}
