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
/// * `safe` - Skip removal if project has deployments with active preview/production URL
#[instrument(skip_all)]
pub async fn rm(
    session: AppzSession,
    project: String,
    yes: bool,
    safe: bool,
) -> AppResult {
    let client = session.get_api_client();

    let project_id = resolve_project_id(&client, &project).await?;

    let project_details = client
        .projects()
        .get(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to get project details: {}", e))?;

    // --safe: skip if project has deployments with active URL
    if safe {
        let team_id = client.get_team_id().await;
        if let Ok(resp) = client
            .deployments()
            .list(
                Some(project_id.clone()),
                Some(50),
                None,
                None,
                team_id,
            )
            .await
        {
            let has_active = resp.deployments.iter().any(|d| {
                d.url.as_ref()
                    .is_some_and(|u| !u.is_empty())
            });
            if has_active {
                let slug = project_details.slug.as_deref().unwrap_or("unknown");
                status::success(&format!(
                    "Skipped project '{}' (has deployments with active preview/production URL)",
                    slug
                ))
                .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
                return Ok(None);
            }
        }
    }

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

    client
        .projects()
        .delete(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to delete project: {}", e))?;

    let project_slug = project_details.slug.as_deref().unwrap_or("unknown");
    status::success(&format!(
        "Deleted project '{}' (ID: {})",
        project_slug, project_id
    ))
    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    Ok(None)
}
