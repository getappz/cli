//! Show deployment logs (Vercel-aligned).

use crate::project::read_project_link;
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use tracing::instrument;
use ui::status;

#[instrument(skip_all)]
pub async fn logs(
    session: crate::session::AppzSession,
    deployment: Option<String>,
) -> AppResult {
    let client = session.get_api_client();

    if client.get_token().await.is_none() {
        return Err(miette!("Not logged in. Run 'appz login' or set APPZ_TOKEN.").into());
    }

    let deployment_id_or_url = match deployment {
        Some(d) => d,
        None => {
            let link = require_linked_project(&session.working_dir)?;
            if !link.link.team_id.is_empty() {
                client.set_team_id(Some(link.link.team_id.clone())).await;
            }
            let list = client
                .deployments()
                .list(Some(link.link.project_id.clone()), Some(1), None, None, None)
                .await
                .map_err(|e| miette!("Failed to list deployments: {}", e))?;
            client.set_team_id(None).await;

            let dep = list
                .deployments
                .first()
                .ok_or_else(|| miette!("No deployments found. Deploy first or specify a deployment."))?;
            dep.id.clone()
        }
    };

    let logs_resp = client
        .deployments()
        .logs(&deployment_id_or_url)
        .await
        .map_err(|e| miette!("Failed to fetch logs: {}", e))?;

    if logs_resp.logs.is_empty() {
        let _ = status::info("No logs found for this deployment.");
        return Ok(None);
    }

    for entry in &logs_resp.logs {
        let ts = entry
            .timestamp
            .map(|t| format_timestamp(t))
            .unwrap_or_else(|| "".to_string());
        let level = entry.level.as_deref().unwrap_or("");
        let msg = entry.message.as_deref().unwrap_or("");
        println!("{} {} {}", ts, level, msg);
    }

    Ok(None)
}

fn require_linked_project(cwd: &Path) -> Result<crate::project::ProjectLinkAndSettings> {
    let link = read_project_link(cwd).map_err(|e| miette!("{}", e))?;
    link.ok_or_else(|| {
        miette!(
            "Project not linked and no deployment specified. Run 'appz link' or pass a deployment: appz logs <url-or-id>"
        )
    })
}

fn format_timestamp(ms: i64) -> String {
    ui::format::timestamp(ms / 1000)
}
