//! List environment variables for a linked project.

use crate::project::{read_project_link, ProjectLinkAndSettings};
use crate::ClientExt;
use api::models::ProjectEnvVariable;
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use tracing::instrument;
use ui::{layout, status};

fn format_target(t: &serde_json::Value) -> String {
    match t {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::String(s) => s.clone(),
        _ => "-".to_string(),
    }
}

#[instrument(skip_all)]
pub async fn ls(session: crate::session::AppzSession, target: Option<String>) -> AppResult {
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
        .list_env(&link.link.project_id, target.as_deref(), false)
        .await
        .map_err(|e| miette!("Failed to list env vars: {}", e))?;

    client.set_team_id(None).await;

    if list.envs.is_empty() {
        let _ = status::info("No environment variables found.");
        return Ok(None);
    }

    let _ = layout::blank_line();
    let _ = layout::section_title("Environment Variables");

    let table = build_table(&list.envs);
    println!("{}", table);

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

fn build_table(envs: &[ProjectEnvVariable]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "{:<24} {:<12} {:<20}", "NAME", "TARGET", "VALUE");
    let _ = writeln!(out, "{:-<24} {:-<12} {:-<20}", "", "", "");
    for ev in envs {
        let value = ev
            .value
            .as_ref()
            .map(|v| {
                if v.len() > 18 {
                    format!("{}...", &v[..15])
                } else {
                    v.clone()
                }
            })
            .unwrap_or_else(|| "***".to_string());
        let target = ev
            .target
            .as_ref()
            .map(format_target)
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(out, "{:<24} {:<12} {:<20}", ev.key, target, value);
    }
    out
}
