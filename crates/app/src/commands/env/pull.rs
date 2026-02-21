//! Pull environment variables and write to .env.local.

use crate::project::{read_project_link, ProjectLinkAndSettings};
use crate::ClientExt;
use miette::{miette, Result};
use starbase::AppResult;
use std::path::Path;
use std::{fs, io};
use tracing::instrument;
use ui::status;

const CONTENTS_PREFIX: &str = "# Created by Appz CLI\n";

/// Default output filename for env pull by target (Vercel parity).
/// - development → .env.local
/// - preview → .env.preview.local
/// - production → .env.production.local
pub fn default_env_filename(target: &str) -> String {
    match target {
        "development" => ".env.local".to_string(),
        "preview" => ".env.preview.local".to_string(),
        "production" => ".env.production.local".to_string(),
        _ => format!(".env.{}.local", target),
    }
}

fn escape_value(v: &str) -> String {
    v.replace('\n', "\\n").replace('\r', "\\r")
}

#[instrument(skip_all)]
pub async fn pull_env(
    session: crate::session::AppzSession,
    filename: String,
    target: String,
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

    let pull = client
        .projects()
        .pull_env(&link.link.project_id, &target)
        .await
        .map_err(|e| miette!("Failed to pull env vars: {}", e))?;

    client.set_team_id(None).await;

    let full_path = session.working_dir.join(&filename);
    let exists = full_path.exists();

    if exists && !yes {
        let mut buffer = String::with_capacity(CONTENTS_PREFIX.len());
        if let Ok(mut f) = fs::File::open(&full_path) {
            let _ = io::Read::read_to_string(&mut f, &mut buffer);
        }
        if !buffer.starts_with(CONTENTS_PREFIX) {
            let confirmed = inquire::Confirm::new(&format!(
                "Overwrite existing {}?",
                filename
            ))
            .with_default(false)
            .prompt()
            .map_err(|e| miette!("Prompt failed: {}", e))?;
            if !confirmed {
                return Ok(None);
            }
        }
    }

    let mut keys: Vec<_> = pull.env.keys().collect();
    keys.sort();
    let contents = CONTENTS_PREFIX.to_string()
        + &keys
            .iter()
            .map(|k| {
                let v = pull.env.get(*k).map(|s| escape_value(s)).unwrap_or_default();
                format!("{}=\"{}\"", k, v)
            })
            .collect::<Vec<_>>()
            .join("\n")
        + "\n";

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).map_err(|e| miette!("Failed to create directory: {}", e))?;
    }
    fs::write(&full_path, contents).map_err(|e| miette!("Failed to write {}: {}", filename, e))?;

    let _ = status::success(&format!(
        "{} {} ({})",
        if exists { "Updated" } else { "Created" },
        filename,
        target
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

#[cfg(test)]
mod tests {
    use super::default_env_filename;

    #[test]
    fn test_default_env_filename_development() {
        assert_eq!(default_env_filename("development"), ".env.local");
    }

    #[test]
    fn test_default_env_filename_preview() {
        assert_eq!(default_env_filename("preview"), ".env.preview.local");
    }

    #[test]
    fn test_default_env_filename_production() {
        assert_eq!(default_env_filename("production"), ".env.production.local");
    }

    #[test]
    fn test_default_env_filename_custom() {
        assert_eq!(default_env_filename("staging"), ".env.staging.local");
    }
}
