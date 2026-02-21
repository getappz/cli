//! Open the linked project in the Appz Dashboard.
//!
//! Vercel-aligned: `vercel open` opens the project dashboard in the default browser.
//! Requires the current directory to be linked to a project (`.appz/project.json` or env vars).

use crate::project::resolve_project_context;
use crate::session::AppzSession;
use crate::ClientExt;
use miette::miette;
use starbase::AppResult;
use tracing::instrument;
use ui::status;

const DEFAULT_DASHBOARD_BASE: &str = "https://appz.dev";

/// Construct the dashboard URL for a project.
///
/// Uses `{base}/{team-slug}/{project-slug}` when team is available,
/// or `{base}/project/{project-slug}` for personal scope.
fn build_dashboard_url(
    base: &str,
    team_slug: Option<&str>,
    project_slug: &str,
) -> String {
    let base = base.trim_end_matches('/');
    match team_slug {
        Some(slug) if !slug.is_empty() => format!("{}/{}/{}", base, slug, project_slug),
        _ => format!("{}/project/{}", base, project_slug),
    }
}

/// Open the current linked project in the Appz Dashboard.
///
/// Requires a linked project (run `appz link` first if not linked).
#[instrument(skip_all)]
pub async fn open(session: AppzSession) -> AppResult {
    let client = session.get_api_client();
    let cwd = session.working_dir.clone();

    let ctx = resolve_project_context(client.clone(), cwd)
        .await
        .map_err(|e| miette!("Failed to resolve project: {}", e))?
        .ok_or_else(|| {
            miette!(
                "No linked project found. Run 'appz link' first to link this directory to a project."
            )
        })?;

    let link = &ctx.link;
    let project_id = link.project_id.as_str();

    // Set team context for API fetches (restore after)
    let previous_team_id = client.get_team_id().await;
    let had_team = !link.team_id.is_empty();
    if had_team {
        client.set_team_id(Some(link.team_id.clone())).await;
    }

    let project = client
        .projects()
        .get(project_id)
        .await
        .map_err(|e| miette!("Failed to fetch project: {}", e))?;

    let project_slug = project
        .slug
        .as_deref()
        .or(project.name.as_deref())
        .unwrap_or(project_id);

    let team_slug = if had_team {
        client
            .teams()
            .get(&link.team_id)
            .await
            .ok()
            .and_then(|t| Some(t.slug).filter(|s| !s.is_empty()))
    } else {
        None
    };

    // Restore previous team scope
    client.set_team_id(previous_team_id).await;

    let dashboard_base =
        std::env::var("APPZ_DASHBOARD_URL").unwrap_or_else(|_| DEFAULT_DASHBOARD_BASE.to_string());
    let url = build_dashboard_url(
        &dashboard_base,
        team_slug.as_deref(),
        project_slug,
    );

    status::info("Opening project in browser...")
        .map_err(|e| miette!("Failed to display message: {}", e))?;

    if let Err(e) = webbrowser::open(&url) {
        tracing::debug!("Failed to open browser: {}", e);
        status::info(&format!("Open manually: {}", url))
            .map_err(|e| miette!("Failed to display message: {}", e))?;
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dashboard_url_with_team() {
        assert_eq!(
            build_dashboard_url("https://appz.dev", Some("my-team"), "my-project"),
            "https://appz.dev/my-team/my-project"
        );
    }

    #[test]
    fn test_build_dashboard_url_personal() {
        assert_eq!(
            build_dashboard_url("https://appz.dev", None, "my-project"),
            "https://appz.dev/project/my-project"
        );
    }

    #[test]
    fn test_build_dashboard_url_empty_team_slug() {
        assert_eq!(
            build_dashboard_url("https://appz.dev", Some(""), "my-project"),
            "https://appz.dev/project/my-project"
        );
    }

    #[test]
    fn test_build_dashboard_url_trailing_slash_base() {
        assert_eq!(
            build_dashboard_url("https://appz.dev/", Some("team"), "proj"),
            "https://appz.dev/team/proj"
        );
    }
}
