//! Rollback command - rollback to a previous deployment

mod status;

use crate::commands::deployment_utils;
use crate::project::resolve_project_context;
use crate::session::AppzSession;
use api::models::Deployment;
use miette::Result;
use starbase::AppResult;
use std::time::Duration;
use tracing::instrument;
use ui::status as ui_status;

/// Rollback to a previous deployment
#[instrument(skip_all)]
pub async fn rollback(
    session: AppzSession,
    deployment_id_or_url: Option<String>,
    timeout: Option<String>,
    yes: bool,
) -> AppResult {
    let client = session.get_api_client();
    let cwd = session.working_dir.clone();

    // Resolve project context
    let project_context = resolve_project_context(&client, &cwd)
        .await?
        .ok_or_else(|| miette::miette!("Project not linked. Run 'appz link' first."))?;

    let project_id = project_context.link.project_id.clone();

    let team_id = project_context.link.team_id.clone();

    // Parse timeout
    let timeout_duration = timeout
        .as_ref()
        .and_then(|t| deployment_utils::parse_timeout(t))
        .unwrap_or_else(|| Duration::from_secs(180)); // Default 3 minutes

    // If no deployment specified, check status
    let deployment_id_or_url = match deployment_id_or_url {
        Some(id) => id,
        None => {
            return status::rollback_status(session, timeout_duration).await;
        }
    };

    // Resolve deployment
    let deployment =
        deployment_utils::resolve_deployment_by_id_or_url(&client, &deployment_id_or_url).await?;

    // Request rollback
    request_rollback(&client, &project_id, &deployment, team_id, timeout_duration).await?;

    Ok(None)
}

/// Request a rollback and poll for completion
#[tracing::instrument(skip(client, deployment, team_id))]
async fn request_rollback(
    client: &api::Client,
    project_id: &str,
    deployment: &Deployment,
    team_id: String,
    timeout: Duration,
) -> Result<()> {
    let deployment_id = deployment.id.clone();

    // Request rollback
    let _ = ui_status::info("Requesting rollback...");

    let rollback_result = client
        .deployments()
        .rollback(project_id, &deployment_id, Some(team_id))
        .await;

    match rollback_result {
        Ok(_) => {
            // Rollback requested, now poll for status
            let _ = ui_status::info("Rollback in progress...");

            // Get project to check status
            let project = client
                .projects()
                .get(project_id)
                .await
                .map_err(|e| miette::miette!("Failed to get project: {}", e))?;

            status::poll_rollback_status(
                client,
                project_id,
                Some(deployment),
                &project,
                timeout,
                true, // performing_rollback
            )
            .await?;
        }
        Err(e) => {
            return Err(miette::miette!("Failed to rollback deployment: {}", e));
        }
    }

    Ok(())
}

/// Check rollback status (subcommand)
#[instrument(skip_all)]
pub async fn status(session: AppzSession, timeout: Option<String>) -> AppResult {
    let timeout_duration = timeout
        .as_ref()
        .and_then(|t| deployment_utils::parse_timeout(t))
        .unwrap_or_else(|| Duration::from_secs(180)); // Default 3 minutes

    status::rollback_status(session, timeout_duration).await
}
