//! Rollback status polling logic

use crate::commands::deployment_utils;
use crate::project::resolve_project_context;
use crate::session::AppzSession;
use api::models::{Deployment, Project};
use miette::Result;
use starbase::AppResult;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::instrument;
use ui::progress;
use ui::status as ui_status;

/// Check rollback status
#[instrument(skip_all)]
pub async fn rollback_status(session: AppzSession, timeout: Duration) -> AppResult {
    let client = session.get_api_client();
    let cwd = session.working_dir.clone();

    // Resolve project context
    let project_context = resolve_project_context(&client, &cwd)
        .await?
        .ok_or_else(|| miette::miette!("Project not linked. Run 'appz link' first."))?;

    let project_id = project_context.link.project_id.clone();

    // Get project
    let project = client
        .projects()
        .get(&project_id)
        .await
        .map_err(|e| miette::miette!("Failed to get project: {}", e))?;

    poll_rollback_status(&client, &project_id, None, &project, timeout, false).await?;

    Ok(None)
}

/// Poll for rollback status until completion or timeout
#[tracing::instrument(skip(client, deployment, project))]
pub async fn poll_rollback_status(
    client: &api::Client,
    project_id: &str,
    deployment: Option<&Deployment>,
    project: &Project,
    timeout: Duration,
    performing_rollback: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let rollback_timeout = start_time + timeout;
    let mut counter = 0u32;

    let project_name = project
        .name
        .as_deref()
        .or(project.slug.as_deref())
        .unwrap_or("project");

    let mut spinner_message = if deployment.is_some() {
        "Rollback in progress".to_string()
    } else {
        format!("Checking rollback status of {}", project_name)
    };

    let mut spinner: Option<progress::SpinnerHandle> = None;

    loop {
        // Get updated project to check lastAliasRequest
        let project_check = client
            .projects()
            .get(project_id)
            .await
            .map_err(|e| miette::miette!("Failed to get project: {}", e))?;

        let last_alias_request = project_check.lastAliasRequest.as_ref();

        let job_status = last_alias_request.and_then(|lar| lar.jobStatus.as_deref());
        let requested_at = last_alias_request.and_then(|lar| lar.requestedAt);
        let to_deployment_id = last_alias_request.and_then(|lar| lar.toDeploymentId.as_deref());
        let request_type = last_alias_request.and_then(|lar| lar.type_.as_deref());

        // Show spinner when polling; one-time info when not active on first iteration
        if matches!(job_status, Some("pending") | Some("in-progress")) {
            if spinner.is_none() {
                spinner = Some(progress::spinner(&spinner_message));
            } else {
                spinner.as_ref().unwrap().set_message(&spinner_message);
            }
        } else if counter == 0 {
            let _ = ui_status::info(&format!("{}...", spinner_message));
        }

        // Check if no active rollback
        if job_status.is_none()
            || requested_at.is_none()
            || to_deployment_id.is_none()
            || request_type != Some("rollback")
        {
            let requested_at_ts = requested_at.unwrap_or(0);
            let now_ts = chrono::Utc::now().timestamp_millis();
            // Check if requested_at is older than the timeout period
            let timeout_ms = timeout.as_millis() as i64;
            if requested_at_ts == 0 || (now_ts - requested_at_ts) > timeout_ms {
                spinner = None;
                let _ = ui_status::info("No deployment rollback in progress");
                return Ok(());
            }
        }

        // Check if skipped
        if job_status == Some("skipped") && request_type == Some("rollback") {
            spinner = None;
            let _ = ui_status::info("Rollback was skipped");
            return Ok(());
        }

        // Check if succeeded
        if job_status == Some("succeeded") {
            spinner = None;
            return render_job_succeeded(
                client,
                project,
                requested_at.unwrap_or(0),
                to_deployment_id.unwrap_or(""),
                performing_rollback,
            )
            .await;
        }

        // Check if failed
        if job_status == Some("failed") {
            spinner = None;
            return render_job_failed(
                client,
                project_id,
                deployment,
                to_deployment_id.unwrap_or(""),
            )
            .await;
        }

        // Check if unknown status
        if !matches!(job_status, Some("pending") | Some("in-progress")) {
            spinner = None;
            let _ = ui_status::error(&format!(
                "Unknown rollback status \"{}\"",
                job_status.unwrap_or("unknown")
            ));
            return Err(miette::miette!("Unknown rollback status"));
        }

        // Check timeout
        let requested_at_ts = requested_at.unwrap_or(0);
        let now_ts = chrono::Utc::now().timestamp_millis();
        if requested_at_ts > 0 && (now_ts - requested_at_ts) > timeout.as_millis() as i64
            || Instant::now() >= rollback_timeout
        {
            spinner = None;
            let _ = ui_status::error(&format!(
                "The rollback exceeded its deadline - rerun 'appz rollback {}' to try again",
                to_deployment_id.unwrap_or("")
            ));
            return Err(miette::miette!("Rollback timeout"));
        }

        // Update spinner message on first poll
        if counter == 0 && deployment.is_none() {
            if let Some(ts) = requested_at {
                spinner_message = format!("{} requested at {}", spinner_message, ts);
            }
        }

        // Sleep before next poll
        sleep(Duration::from_millis(250)).await;
        counter += 1;
    }
}

async fn render_job_succeeded(
    client: &api::Client,
    project: &Project,
    requested_at: i64,
    to_deployment_id: &str,
    performing_rollback: bool,
) -> Result<()> {
    let project_name = project
        .name
        .as_deref()
        .or(project.slug.as_deref())
        .unwrap_or("project");

    // Try to get deployment info
    let deployment_info = match client.deployments().get(to_deployment_id).await {
        Ok(deployment) => {
            let url = deployment.url.as_deref().unwrap_or(to_deployment_id);
            format!("{} ({})", url, to_deployment_id)
        }
        Err(_) => to_deployment_id.to_string(),
    };

    let duration = if performing_rollback && requested_at > 0 {
        let now_ts = chrono::Utc::now().timestamp_millis();
        let elapsed_ms = now_ts - requested_at;
        let elapsed = Duration::from_millis(elapsed_ms as u64);
        format!(" {}", deployment_utils::format_elapsed_time(elapsed))
    } else {
        String::new()
    };

    let _ = ui_status::success(&format!(
        "Success! {} was rolled back to {}{}",
        project_name, deployment_info, duration
    ));

    Ok(())
}

async fn render_job_failed(
    client: &api::Client,
    _project_id: &str,
    deployment: Option<&Deployment>,
    to_deployment_id: &str,
) -> Result<()> {
    let deployment_name = if let Some(deployment) = deployment {
        deployment
            .url
            .as_deref()
            .unwrap_or(&deployment.id)
            .to_string()
    } else {
        match client.deployments().get(to_deployment_id).await {
            Ok(deployment) => deployment
                .url
                .as_deref()
                .unwrap_or(to_deployment_id)
                .to_string(),
            Err(_) => to_deployment_id.to_string(),
        }
    };

    let _ = ui_status::error(&format!(
        "Failed to remap all aliases to the requested deployment {} ({})",
        deployment_name, to_deployment_id
    ));

    Err(miette::miette!("Rollback failed"))
}
