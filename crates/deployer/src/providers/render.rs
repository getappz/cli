//! Render deploy provider.
//!
//! Deploys static sites to Render via the `render.yaml` blueprint
//! or the Render dashboard API.
//!
//! ## Detection
//!
//! - `render.yaml` / `render.yml` — Render blueprint
//!
//! ## Authentication
//!
//! - `RENDER_API_KEY` environment variable (CI/CD)
//! - Render dashboard (local)

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::config::{DeployContext, SetupContext};
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::DeployProvider;
use crate::providers::helpers::has_env_var;

/// Render provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderConfig {
    /// Render service ID.
    pub service_id: Option<String>,
    /// Service name.
    pub service_name: Option<String>,
}

/// Render deploy provider.
pub struct RenderProvider;

#[async_trait]
impl DeployProvider for RenderProvider {
    fn name(&self) -> &str {
        "Render"
    }

    fn slug(&self) -> &str {
        "render"
    }

    fn description(&self) -> &str {
        "Deploy to Render (static sites and web services)"
    }

    fn cli_tool(&self) -> &str {
        "render"
    }

    fn auth_env_var(&self) -> &str {
        "RENDER_API_KEY"
    }

    async fn check_prerequisites(
        &self,
        _sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        // Render deploys via git push or dashboard; API key is optional
        if has_env_var("RENDER_API_KEY") {
            Ok(PrerequisiteStatus::Ready)
        } else {
            Ok(PrerequisiteStatus::AuthMissing {
                env_var: "RENDER_API_KEY".into(),
                login_hint: "Get your API key from https://dashboard.render.com/u/settings#api-keys".into(),
            })
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        for name in &["render.yaml", "render.yml"] {
            if project_dir.join(name).exists() {
                return Ok(Some(DetectedConfig {
                    config_file: name.to_string(),
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

        let _ = ui::layout::section_title("Setting up Render deployment");
        let _ = ui::status::info(
            "Render deploys automatically via git push.\n\
             Create a render.yaml blueprint or connect your repo at https://dashboard.render.com"
        );

        let config = RenderConfig::default();
        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "render".into(),
                url: "https://dry-run.onrender.com".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        // Render typically deploys via git push; trigger via sandbox exec
        let result = ctx.exec("git push").await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Render".into(),
                reason: "git push failed. Ensure your repo is connected to Render.".into(),
            });
        }

        Ok(DeployOutput {
            provider: "render".into(),
            url: "https://onrender.com".into(),
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Building,
            created_at: Some(chrono::Utc::now()),
            duration_ms: None,
        })
    }

    async fn deploy_preview(&self, _ctx: DeployContext) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: "Render".into(),
            operation: "preview deployments (use Render's PR preview feature via dashboard)".into(),
        })
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
