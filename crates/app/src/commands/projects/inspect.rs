//! Inspect a project's details (Vercel-aligned).
//!
//! With name: fetch project by ID or slug.
//! Without name: use linked project from current directory (requires `.appz/project.json`).

use crate::commands::projects::resolve_project_id;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Inspect project details.
///
/// # Arguments
/// * `name` - Project ID or slug (optional – uses linked project if omitted)
/// * `yes` - Skip confirmation when linking (used when no name and not yet linked)
#[instrument(skip_all)]
pub async fn inspect(
    session: AppzSession,
    name: Option<String>,
    _yes: bool,
) -> AppResult {
    let client = session.get_api_client();

    let project_id = if let Some(ref n) = name {
        resolve_project_id(&client, n).await?
    } else {
        // Use linked project from CWD
        let project_context = session.get_project_context().ok_or_else(|| {
            miette::miette!(
                "No project specified and current directory is not linked.\n\
                 Run `appz project inspect <name>` or `appz link` to link this directory first."
            )
        })?;
        project_context.link.project_id.clone()
    };

    let project = client
        .projects()
        .get(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to get project: {}", e))?;

    let id = project.id.as_deref().unwrap_or("N/A");
    let slug = project.slug.as_deref().unwrap_or("N/A");
    let name_display = project.name.as_deref().unwrap_or(slug);
    let team_id = project.teamId.as_deref().unwrap_or("N/A");

    status::success(&format!("Project: {}", name_display))
        .map_err(|e| miette::miette!("{}", e))?;

    status::info(&format!("  ID:      {}", id)).map_err(|e| miette::miette!("{}", e))?;
    status::info(&format!("  Slug:    {}", slug)).map_err(|e| miette::miette!("{}", e))?;
    status::info(&format!("  Team:    {}", team_id)).map_err(|e| miette::miette!("{}", e))?;

    if let Some(created_at) = project.createdAt {
        let created_str = ui::format::timestamp_auto(created_at);
        status::info(&format!("  Created: {}", created_str)).map_err(|e| miette::miette!("{}", e))?;
    }

    Ok(None)
}
