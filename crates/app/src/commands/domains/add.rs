//! Add a domain to a project (Vercel parity: `domains add`).
//!
//! Assigns a custom domain to a project's production (or staging/preview) environment.
//! The domain will resolve to the latest deployment for that project+environment.

use crate::session::AppzSession;
use crate::ClientExt;
use crate::commands::projects::resolve_project_id;
use crate::project::read_project_link;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

/// Strip protocol from domain if present.
fn strip_protocol(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with("https://") {
        &s[8..]
    } else if s.starts_with("http://") {
        &s[7..]
    } else {
        s
    }
}

/// Validate hostname: non-empty, no spaces, reasonable length.
fn is_valid_domain(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && !s.chars().all(|c| c == '/' || c == ' ') && s.len() < 256
}

/// Add a domain to a project.
///
/// Vercel parity: `vercel domains add <domain> [project]`.
/// When linked to a project, only domain is required; otherwise domain and project are required.
#[instrument(skip_all)]
pub async fn add(
    session: AppzSession,
    domain: String,
    project: Option<String>,
    environment: String,
    team: Option<String>,
) -> AppResult {
    let client = session.get_api_client();

    let domain_host = strip_protocol(&domain).trim().to_string();
    if !is_valid_domain(&domain_host) {
        return Err(miette::miette!(
            "Invalid domain: \"{}\". Do not include the protocol (https://).",
            domain
        )
        .into());
    }

    // Resolve project: from arg, or from linked project
    let project_id = if let Some(ref proj) = project {
        resolve_project_id(client.clone(), proj.clone()).await?
    } else {
        let link_opt = read_project_link(&session.working_dir).map_err(|e| {
            miette::miette!("No project linked. Run from a linked project or pass project: {}", e)
        })?;
        let link = link_opt.ok_or_else(|| {
            miette::miette!("No project linked. Run from a linked project or pass project name.")
        })?;
        resolve_project_id(client.clone(), link.link.project_id.clone()).await?
    };

    // Resolve team: from -T flag, linked project, or current context
    let team_id = if let Some(ref t) = team {
        Some(t.clone())
    } else if let Ok(Some(link)) = read_project_link(&session.working_dir) {
        Some(link.link.team_id.clone())
    } else {
        client.get_team_id().await
    };

    let team_id = team_id.ok_or_else(|| {
        miette::miette!("teamId is required. Use --team <id> or run from a linked project.")
    })?;

    status::info(&format!(
        "Adding domain {} to project {} ({})",
        domain_host, project_id, environment
    ))
    .map_err(|e| miette::miette!("{}", e))?;

    let env_str = match environment.as_str() {
        "staging" | "preview" => environment.as_str(),
        _ => "production",
    };

    client
        .projects()
        .add_alias(&project_id, &domain_host, Some(team_id), Some(env_str))
        .await
        .map_err(|e| {
            let msg = match &e {
                api::ApiError::ApiError { code, message } => {
                    format!("Failed to add domain: {} (code: {})", message, code)
                }
                api::ApiError::Validation(m) => format!("Validation error: {}", m),
                _ => format!("Failed to add domain: {}", e),
            };
            miette::miette!("{}", msg)
        })?;

    status::success(&format!(
        "Domain {} added to project. It will serve the latest {} deployment.",
        domain_host, env_str
    ))
    .map_err(|e| miette::miette!("{}", e))?;

    Ok(None)
}
