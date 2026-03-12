//! Remove an environment variable from a linked project.

use crate::project::{read_project_link, ProjectLinkAndSettings};
use crate::ClientExt;
use api::models::ProjectEnvVariable;
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use tracing::instrument;
use ui::status;

fn env_matches_target(ev: &ProjectEnvVariable, target: &str) -> bool {
    match &ev.target {
        Some(serde_json::Value::String(s)) => s == target,
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(target)),
        _ => true,
    }
}

#[instrument(skip_all)]
pub async fn rm(
    session: crate::session::AppzSession,
    key: String,
    target: Option<String>,
    yes: bool,
) -> AppResult {
    let link = require_linked_project(&session.working_dir)?;
    let client = session.get_api_client();

    if client.get_token().await.is_none() {
        return Err(miette!("Not logged in. Run 'appz login' or set APPZ_TOKEN.").into());
    }

    if !link.link.team_id.is_empty() {
        client.set_team_id(Some(link.link.team_id.clone())).await;
    }

    let list = client
        .projects()
        .list_env(&link.link.project_id, None, false)
        .await
        .map_err(|e| miette!("Failed to list env vars: {}", e))?;

    let matches: Vec<&ProjectEnvVariable> = list
        .envs
        .iter()
        .filter(|ev| {
            ev.key == key
                && target
                    .as_ref()
                    .map(|t| env_matches_target(ev, t))
                    .unwrap_or(true)
        })
        .collect();

    client.set_team_id(None).await;

    if matches.is_empty() {
        return Err(miette!(
            "Environment variable '{}' not found{}.",
            key,
            target.as_ref().map(|t| format!(" for target '{}'", t)).unwrap_or_default()
        ).into());
    }

    if matches.len() > 1 && target.is_none() {
        return Err(miette!(
            "Multiple matching variables. Specify --target to remove one."
        ).into());
    }

    if !yes && matches.len() == 1 {
        let ev = matches[0];
        let target_str = ev
            .target
            .as_ref()
            .and_then(|t| t.as_str())
            .unwrap_or("all");
        let confirmed = inquire::Confirm::new(&format!(
            "Remove {} from {} ({})?",
            key,
            link.link.project_name.as_deref().unwrap_or("project"),
            target_str
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| miette!("Prompt failed: {}", e))?;
        if !confirmed {
            return Ok(None);
        }
    }

    if !link.link.team_id.is_empty() {
        client.set_team_id(Some(link.link.team_id.clone())).await;
    }

    for ev in &matches {
        client
            .projects()
            .remove_env(&link.link.project_id, &ev.id)
            .await
            .map_err(|e| miette!("Failed to remove env var: {}", e))?;
    }

    client.set_team_id(None).await;

    let _ = status::success("Removed environment variable");

    Ok(None)
}

fn require_linked_project(cwd: &Path) -> Result<ProjectLinkAndSettings> {
    let link = read_project_link(cwd).map_err(|e| miette!("{}", e))?;
    link.ok_or_else(|| {
        miette!(
            "Project not linked. Run 'appz link' to link this directory to a project."
        )
    })
}
