//! Pull project config and env from Appz API (Vercel-aligned).
//!
//! Fetches project settings and env vars, writes .appz/project.json and .env.local.

use crate::commands::env::pull_env;
use crate::project::{read_project_link, write_project_link, ProjectLinkAndSettings};
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use tracing::instrument;
use ui::status;

#[instrument(skip_all)]
pub async fn pull(session: crate::session::AppzSession) -> AppResult {
    let link = require_linked_project(&session.working_dir)?;
    let client = session.get_api_client();

    if client.get_token().await.is_none() {
        return Err(miette!("Not logged in. Run 'appz login' or set APPZ_TOKEN.").into());
    }

    if !link.link.team_id.is_empty() {
        client.set_team_id(Some(link.link.team_id.clone())).await;
    }

    let project = client
        .projects()
        .get(&link.link.project_id)
        .await
        .map_err(|e| miette!("Failed to fetch project: {}", e))?;

    client.set_team_id(None).await;

    let mut link_and_settings = link.clone();
    link_and_settings.link.project_name = project.name.clone();
    if let Some(ref slug) = project.slug {
        if link_and_settings.link.project_name.is_none() {
            link_and_settings.link.project_name = Some(slug.clone());
        }
    }
    if let Some(ref id) = project.id {
        link_and_settings.link.project_id = id.clone();
    }
    if let Some(ref tid) = project.teamId {
        link_and_settings.link.team_id = tid.clone();
    }

    write_project_link(&session.working_dir, &link_and_settings)
        .map_err(|e| miette!("Failed to write project config: {}", e))?;

    let _ = status::success("Downloaded project settings to .appz/project.json");

    pull_env(session, ".env.local".to_string(), "development".to_string(), true).await?;

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
