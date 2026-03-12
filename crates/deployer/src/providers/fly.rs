//! Fly.io deploy provider.
//!
//! Deploys static sites to Fly.io via the `flyctl` / `fly` CLI.
//!
//! ## Detection
//!
//! - `fly.toml` — Fly app configuration
//!
//! ## Authentication
//!
//! - `FLY_API_TOKEN` environment variable (CI/CD)
//! - `fly auth login` interactive flow (local)

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::config::{DeployContext, SetupContext};
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::DeployProvider;
use crate::providers::helpers::{combined_output, extract_url_from_output, has_env_var};

/// Fly.io provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlyConfig {
    /// Fly app name.
    pub app_name: Option<String>,
    /// Fly organization.
    pub org: Option<String>,
    /// Fly region.
    pub region: Option<String>,
}

/// Fly.io deploy provider.
pub struct FlyProvider;

/// Resolve the fly CLI command name via the sandbox.
///
/// Checks `flyctl` first (common install name), falls back to `fly`.
async fn resolve_fly_cmd(sandbox: &dyn SandboxProvider) -> &'static str {
    if sandbox.exec("flyctl version").await.map(|o| o.success()).unwrap_or(false) {
        "flyctl"
    } else {
        "fly"
    }
}

#[async_trait]
impl DeployProvider for FlyProvider {
    fn name(&self) -> &str {
        "Fly.io"
    }

    fn slug(&self) -> &str {
        "fly"
    }

    fn description(&self) -> &str {
        "Deploy to Fly.io's global application platform"
    }

    fn cli_tool(&self) -> &str {
        "fly"
    }

    fn auth_env_var(&self) -> &str {
        "FLY_API_TOKEN"
    }

    async fn check_prerequisites(
        &self,
        sandbox: &dyn SandboxProvider,
    ) -> DeployResult<PrerequisiteStatus> {
        let has_fly = sandbox.exec("fly version").await.map(|o| o.success()).unwrap_or(false);
        let has_flyctl = sandbox.exec("flyctl version").await.map(|o| o.success()).unwrap_or(false);
        let cli_ok = has_fly || has_flyctl;
        let auth_ok = has_env_var("FLY_API_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "fly".into(),
                install_hint: "curl -L https://fly.io/install.sh | sh".into(),
            }),
            (true, false) => {
                let cmd = resolve_fly_cmd(sandbox).await;
                let result = sandbox.exec(&format!("{} auth whoami", cmd)).await;
                if result.map(|o| o.success()).unwrap_or(false) {
                    Ok(PrerequisiteStatus::Ready)
                } else {
                    Ok(PrerequisiteStatus::AuthMissing {
                        env_var: "FLY_API_TOKEN".into(),
                        login_hint: "fly auth login".into(),
                    })
                }
            }
        }
    }

    async fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        if project_dir.join("fly.toml").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "fly.toml".into(),
                is_linked: true,
                project_name: None,
                team: None,
            }));
        }
        Ok(None)
    }

    async fn setup(&self, ctx: &mut SetupContext) -> DeployResult<serde_json::Value> {
        if ctx.is_ci {
            return Err(DeployError::CiMissingConfig);
        }

        let _ = ui::layout::section_title("Setting up Fly.io deployment");

        let cmd = resolve_fly_cmd(&*ctx.sandbox).await;

        // Verify CLI is available
        let cli_ok = ctx.sandbox.exec(&format!("{} version", cmd)).await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            return Err(DeployError::CliNotFound {
                tool: "fly".into(),
                provider: "Fly.io".into(),
            });
        }

        let _ = ui::status::info("Launching Fly.io app...");
        let status = ctx.exec_interactive(&format!("{} launch", cmd)).await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Fly.io".into(),
                reason: "Failed to launch Fly.io app".into(),
            });
        }

        let config = FlyConfig::default();
        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "fly".into(),
                url: "https://dry-run.fly.dev".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let cmd_name = resolve_fly_cmd(&*ctx.sandbox).await;
        let cmd = format!("{} deploy", cmd_name);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Fly.io".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://fly.dev".into());

        Ok(DeployOutput {
            provider: "fly".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn deploy_preview(&self, _ctx: &DeployContext) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: "Fly.io".into(),
            operation: "preview deployments".into(),
        })
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
