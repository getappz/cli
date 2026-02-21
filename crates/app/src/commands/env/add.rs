//! Add an environment variable to a linked project.

use crate::project::{read_project_link, ProjectLinkAndSettings};
use api::models::AddEnvRequest;
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use tracing::instrument;
use ui::status;

#[instrument(skip_all)]
pub async fn add(
    session: crate::session::AppzSession,
    key: String,
    value: Option<String>,
    target: String,
    force: bool,
) -> AppResult {
    let link = require_linked_project(&session.working_dir)?;
    let client = session.get_api_client();

    if client.get_token().await.is_none() {
        return Err(miette!("Not logged in. Run 'appz login' or set APPZ_TOKEN.").into());
    }

    let value = value.unwrap_or_else(|| {
        inquire::Password::new("Value: ")
            .prompt()
            .unwrap_or_default()
    });
    if value.is_empty() {
        return Err(miette!("Value cannot be empty.").into());
    }

    if !link.link.team_id.is_empty() {
        client.set_team_id(Some(link.link.team_id.clone())).await;
    }

    let body = AddEnvRequest {
        key: key.clone(),
        value,
        r#type: Some("plain".to_string()),
        target: vec![target.clone()],
        gitBranch: None,
    };

    client
        .projects()
        .add_env(&link.link.project_id, &body, force)
        .await
        .map_err(|e| miette!("Failed to add env var: {}", e))?;

    client.set_team_id(None).await;

    let _ = status::success(&format!(
        "Added {} to {} ({})",
        key, link.link.project_name.as_deref().unwrap_or("project"), target
    ));

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
