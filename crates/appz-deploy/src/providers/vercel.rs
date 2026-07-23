//! Vercel deploy provider.
//!
//! ## Detection
//! - `.vercel/project.json` — linked project state (projectId/orgId)
//! - `vercel.json` — project configuration
//!
//! ## Authentication
//! - `VERCEL_TOKEN` environment variable (CI/CD) or `vercel login` (local)
//!
//! ## Build model
//! Vercel builds through its OWN pipeline, not appz's `mise run build`: this
//! provider runs the documented prebuilt-deploy recipe (`vercel build` then
//! `vercel deploy --prebuilt`) rather than handing Vercel an arbitrary static
//! folder, which it only accepts in the Build Output API format `vercel
//! build` itself produces. "Drive the platform's own CLI" means Vercel keeps
//! doing what it already does well.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct VercelProvider;

impl VercelProvider {
    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let mut build_args = vec!["build"];
        if !is_preview {
            build_args.push("--prod");
        }
        let build = ctx
            .exec("vercel", &build_args, DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !build.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("vercel build failed:\n{}\n{}", build.stdout, build.stderr),
            });
        }

        let mut deploy_args = vec!["deploy", "--prebuilt", "--yes"];
        if !is_preview {
            deploy_args.push("--prod");
        }
        let deploy = ctx
            .exec("vercel", &deploy_args, DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !deploy.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "vercel deploy failed:\n{}\n{}",
                    deploy.stdout, deploy.stderr
                ),
            });
        }

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: extract_deploy_url(&deploy.stdout),
            is_preview,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }
}

impl DeployProvider for VercelProvider {
    fn name(&self) -> &'static str {
        "Vercel"
    }
    fn slug(&self) -> &'static str {
        "vercel"
    }
    fn cli_tool(&self) -> &'static str {
        "vercel"
    }
    fn install_package(&self) -> &'static str {
        "vercel"
    }
    fn auth_env_var(&self) -> &'static str {
        "VERCEL_TOKEN"
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("vercel").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "vercel".into(),
                install_hint: "npm i -g vercel".into(),
            });
        }
        if std::env::var("VERCEL_TOKEN").is_err() {
            return Ok(PrerequisiteStatus::AuthMissing {
                env_var: "VERCEL_TOKEN".into(),
                login_hint: "vercel login".into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let state_path = project_dir.join(".vercel/project.json");
        if state_path.exists() {
            let content = std::fs::read_to_string(&state_path)?;
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(DetectedConfig {
                    config_file: ".vercel/project.json".into(),
                    is_linked: true,
                    project_name: state
                        .get("projectId")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    team: state
                        .get("orgId")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                }));
            }
        }
        if project_dir.join("vercel.json").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "vercel.json".into(),
                is_linked: false,
                project_name: None,
                team: None,
            }));
        }
        Ok(None)
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        if !ctx.project_dir.join(".vercel/project.json").exists() {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: "project not linked to Vercel — run `vercel link` first".into(),
            });
        }
        let out = ctx
            .exec("vercel", &["ls", "--yes"], QUERY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !out.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("vercel ls failed:\n{}\n{}", out.stdout, out.stderr),
            });
        }
        // Best-effort text scan, not a schema we assert with false confidence
        // (`vercel ls`'s plain-text table format isn't guaranteed stable
        // across CLI versions; verify against a live run before relying on
        // this for anything beyond a human-readable listing).
        let deployments = out
            .stdout
            .lines()
            .filter_map(|line| {
                let url = line
                    .split_whitespace()
                    .find(|tok| tok.starts_with("https://"))?;
                Some(DeploymentInfo {
                    id: url.to_string(),
                    url: Some(url.to_string()),
                    status: DeployStatus::Unknown,
                    created_at: None,
                    is_current: false,
                })
            })
            .collect();
        Ok(deployments)
    }
}

/// Extract the real deployment URL from `vercel deploy`'s stdout. Vercel's
/// CLI contract is designed for scripting (`URL=$(vercel deploy ...)`) — its
/// stdout is the deployment URL. Scans for the LAST bare `https://` line
/// rather than the first (the upstream bug this fixes: some flows print an
/// inspect/status URL before the deploy URL) and returns `None` — never a
/// guessed URL like `https://{dirname}.vercel.app` — if nothing matches.
fn extract_deploy_url(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_last_url_not_the_first() {
        // Regression test for the upstream bug this fixes: some flows print
        // an inspect/status URL before the real deploy URL.
        let stdout = "https://vercel.com/acme/app/inspect/abc123\nhttps://app-abc123.vercel.app\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://app-abc123.vercel.app".to_string())
        );
    }

    #[test]
    fn returns_none_instead_of_fabricating_a_url() {
        let stdout = "Deploying...\nDone.\n";
        assert_eq!(extract_deploy_url(stdout), None);
    }
}
