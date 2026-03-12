//! Cloudflare Pages deploy provider.
//!
//! Deploys static sites via the Wrangler CLI (`wrangler`).
//!
//! ## Detection
//!
//! - `wrangler.toml` / `wrangler.json` — project configuration
//!
//! ## Authentication
//!
//! - `CLOUDFLARE_API_TOKEN` environment variable (CI/CD)
//! - `wrangler login` interactive flow (local)

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

/// Cloudflare Pages provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloudflarePagesConfig {
    /// Cloudflare account ID.
    pub account_id: Option<String>,
    /// Cloudflare Pages project name.
    pub project_name: Option<String>,
    /// Production branch name.
    pub production_branch: Option<String>,
}

/// Cloudflare Pages deploy provider.
pub struct CloudflarePagesProvider;

#[async_trait]
impl DeployProvider for CloudflarePagesProvider {
    fn name(&self) -> &str {
        "Cloudflare Pages"
    }

    fn slug(&self) -> &str {
        "cloudflare-pages"
    }

    fn description(&self) -> &str {
        "Deploy to Cloudflare's global edge network with Pages"
    }

    fn cli_tool(&self) -> &str {
        "wrangler"
    }

    fn auth_env_var(&self) -> &str {
        "CLOUDFLARE_API_TOKEN"
    }

    async fn check_prerequisites(
        &self,
        sandbox: &dyn SandboxProvider,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("wrangler --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("CLOUDFLARE_API_TOKEN") || has_env_var("CLOUDFLARE_ACCOUNT_ID");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "wrangler".into(),
                install_hint: "npm i -g wrangler".into(),
            }),
            (true, false) => Ok(PrerequisiteStatus::AuthMissing {
                env_var: "CLOUDFLARE_API_TOKEN".into(),
                login_hint: "wrangler login".into(),
            }),
        }
    }

    async fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        for config_name in &["wrangler.toml", "wrangler.json", "wrangler.jsonc"] {
            let config_path = project_dir.join(config_name);
            if config_path.exists() {
                return Ok(Some(DetectedConfig {
                    config_file: config_name.to_string(),
                    is_linked: false,
                    project_name: None,
                    team: None,
                }));
            }
        }
        Ok(None)
    }

    async fn setup(&self, ctx: &mut SetupContext) -> DeployResult<serde_json::Value> {
        if ctx.is_ci {
            return Err(DeployError::CiMissingConfig);
        }

        let _ = ui::layout::section_title("Setting up Cloudflare Pages deployment");

        let cli_ok = ctx.sandbox.exec("wrangler --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(&*ctx.sandbox).await?;
        }

        // Ensure logged in
        let _ = ui::status::info("Authenticating with Cloudflare...");
        let status = ctx.exec_interactive("wrangler login").await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Cloudflare Pages".into(),
                reason: "Failed to authenticate with Cloudflare".into(),
            });
        }

        let config = CloudflarePagesConfig {
            production_branch: Some("main".into()),
            ..Default::default()
        };

        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "cloudflare-pages".into(),
                url: "https://dry-run.pages.dev".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let cf_config = ctx.deploy_config
            .get_target_config::<CloudflarePagesConfig>("cloudflare-pages")?
            .unwrap_or_default();

        let project_flag = cf_config.project_name
            .as_deref()
            .map(|n| format!(" --project-name {}", n))
            .unwrap_or_default();
        let branch_flag = cf_config.production_branch
            .as_deref()
            .map(|b| format!(" --branch {}", b))
            .unwrap_or_default();

        let cmd = format!(
            "wrangler pages deploy {}{}{}",
            ctx.output_dir, project_flag, branch_flag
        );

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Cloudflare Pages".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://pages.dev".into());

        Ok(DeployOutput {
            provider: "cloudflare-pages".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "cloudflare-pages".into(),
                url: "https://dry-run-preview.pages.dev".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: true,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let cf_config = ctx.deploy_config
            .get_target_config::<CloudflarePagesConfig>("cloudflare-pages")?
            .unwrap_or_default();

        let project_flag = cf_config.project_name
            .as_deref()
            .map(|n| format!(" --project-name {}", n))
            .unwrap_or_default();

        let cmd = format!(
            "wrangler pages deploy {} --branch preview{}",
            ctx.output_dir, project_flag
        );

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Cloudflare Pages".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://pages.dev".into());

        Ok(DeployOutput {
            provider: "cloudflare-pages".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: true,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    fn supports_env_vars(&self) -> bool {
        true
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
