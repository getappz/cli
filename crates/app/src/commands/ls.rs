use crate::args::LsArgs;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;

/// Parse policy args like ["errored=6m", "preview=12m"] into vec of (k,v).
fn parse_policy(policy: &[String]) -> Option<Vec<(String, String)>> {
    if policy.is_empty() {
        return None;
    }
    let parsed: Vec<(String, String)> = policy
        .iter()
        .filter_map(|s| {
            let eq = s.find('=')?;
            let (k, v) = s.split_at(eq);
            Some((k.trim().to_string(), v[1..].trim().to_string()))
        })
        .collect();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// List deployments — from Appz cloud (if linked) or a hosting provider.
#[instrument(skip_all)]
pub async fn ls(session: AppzSession, args: LsArgs) -> AppResult {
    // If an explicit provider is given, always use the provider path
    if let Some(ref provider_slug) = args.provider {
        return ls_from_provider(&session, provider_slug).await;
    }

    // Try Appz cloud first (if linked)
    #[cfg(feature = "appz-cloud")]
    {
        if session.get_project_context().is_some() {
            let project_context = session.get_project_context().unwrap();
            return ls_from_cloud(&session, &args, project_context).await;
        }
    }

    // Fall back to deploy provider (if configured)
    #[cfg(feature = "deploy")]
    {
        let project_dir = session.working_dir.clone();
        if let Ok(Some(config)) = deployer::read_deploy_config(&project_dir) {
            if let Some(ref default) = config.default {
                return ls_from_provider(&session, default).await;
            }
        }
    }

    Err(miette::miette!(
        "No deployment source found.\n\n\
         Options:\n  \
         - Link to Appz cloud: appz link\n  \
         - Specify a provider: appz ls <provider> (e.g. vercel, netlify)\n  \
         - Set up deployment: appz deploy --init"
    ))
}

// ---------------------------------------------------------------------------
// Appz cloud path
// ---------------------------------------------------------------------------

#[cfg(feature = "appz-cloud")]
async fn ls_from_cloud(
    session: &AppzSession,
    args: &LsArgs,
    project_context: &crate::project::ProjectContext,
) -> AppResult {
    use crate::ClientExt;
    use api::error::ApiError as ApiErrorType;

    let client = session.get_api_client();
    let project_id = project_context.link.project_id.clone();

    if client.get_team_id().await.is_none() {
        client
            .set_team_id(Some(project_context.link.team_id.clone()))
            .await;
    }

    let policy_params = parse_policy(&args.policy);
    let show_policy = policy_params.is_some();
    let deployments_response = match client
        .deployments()
        .list(
            Some(project_id),
            None,
            None,
            None,
            None,
            policy_params.clone(),
        )
        .await
    {
        Ok(r) => r,
        Err(ApiErrorType::Forbidden(_)) => {
            return Err(miette::miette!(
                "Access denied: this project belongs to a different team. \
                 Switch to the project's team or use --scope to list deployments."
            ));
        }
        Err(e) => return Err(miette::miette!("Failed to list deployments: {}", e)),
    };

    if deployments_response.deployments.is_empty() {
        ui::empty::display(
            "No deployments found",
            Some("Try creating a deployment first"),
        )?;
        return Ok(None);
    }

    let mut headers = vec!["Age", "Deployment", "Status", "Environment"];
    if !show_policy {
        headers.push("Duration");
        headers.push("Username");
    } else {
        headers.push("Proposed Expiration");
    }
    let mut rows = Vec::new();

    for deployment in &deployments_response.deployments {
        let status = deployment.status.as_deref().unwrap_or("unknown");
        let status_display = format!("● {}", ui::format::status_badge(status));
        let url = deployment
            .url
            .as_deref()
            .unwrap_or("–")
            .to_string();
        let env = deployment
            .env_type
            .as_deref()
            .map(|t| {
                if t.eq_ignore_ascii_case("production") {
                    "Production"
                } else {
                    "Preview"
                }
            })
            .unwrap_or("Preview");
        let age = ui::format::timestamp_age_short(deployment.createdAt);
        let duration = if status.eq_ignore_ascii_case("ready")
            || status.eq_ignore_ascii_case("completed")
        {
            let dur_secs = (deployment.updatedAt - deployment.createdAt) / 1000;
            if dur_secs >= 0 {
                ui::format::duration(dur_secs as u64)
            } else {
                "–".to_string()
            }
        } else {
            "–".to_string()
        };
        let username = deployment
            .createdBy
            .as_deref()
            .unwrap_or("–")
            .to_string();

        let proposed_exp = deployment
            .proposedExpiration
            .map(|ts| ui::format::timestamp_auto(ts))
            .unwrap_or_else(|| "No expiration".to_string());

        if show_policy {
            rows.push(vec![age, url, status_display, env.to_string(), proposed_exp]);
        } else {
            rows.push(vec![age, url, status_display, env.to_string(), duration, username]);
        }
    }

    ui::table::display(&headers, &rows, Some("Deployments"))?;

    let pagination_info = ui::pagination::PaginationInfo::new(
        deployments_response.pagination.count,
        deployments_response.pagination.next,
        deployments_response.pagination.prev,
    );
    ui::pagination::display(&pagination_info)?;

    Ok(None)
}

// ---------------------------------------------------------------------------
// Deploy provider path
// ---------------------------------------------------------------------------

#[cfg(feature = "deploy")]
async fn ls_from_provider(session: &AppzSession, provider_slug: &str) -> AppResult {
    let project_dir = session.working_dir.clone();
    let deploy_config = deployer::read_deploy_config(&project_dir)
        .map_err(|e| miette::miette!("{}", e))?
        .unwrap_or_default();

    let output_dir = crate::commands::deploy::resolve_output_dir(&project_dir);
    let provider = deployer::get_provider(provider_slug).map_err(|e| miette::miette!("{}", e))?;
    let sandbox = crate::commands::deploy::create_deploy_sandbox(project_dir).await?;

    let ctx = deployer::DeployContext::new(sandbox, output_dir)
        .with_config(deploy_config);

    let deployments = provider
        .list_deployments(ctx)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    if deployments.is_empty() {
        let _ = ui::status::info("No deployments found.");
        return Ok(None);
    }

    let headers = vec!["Age", "Deployment", "Status", ""];
    let mut rows = Vec::new();

    for dep in &deployments {
        let age = dep
            .created_at
            .map(|dt| {
                let secs = (chrono::Utc::now() - dt).num_seconds().max(0) as u64;
                ui::format::duration(secs)
            })
            .unwrap_or_else(|| "–".to_string());
        let status_str = format!("{}", dep.status);
        let current = if dep.is_current { "(current)" } else { "" };
        rows.push(vec![age, dep.url.clone(), status_str, current.to_string()]);
    }

    ui::table::display(&headers, &rows, Some(&format!("{} Deployments", provider.name())))?;

    Ok(None)
}

/// Fallback when deploy feature is not enabled.
#[cfg(not(feature = "deploy"))]
async fn ls_from_provider(_session: &AppzSession, provider_slug: &str) -> AppResult {
    Err(miette::miette!(
        "Provider '{}' requires the deploy feature. Rebuild with --features deploy.",
        provider_slug
    ))
}
