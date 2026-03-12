//! Surge deploy provider.
//!
//! Deploys static sites via the Surge CLI (`surge`).
//!
//! ## Detection
//!
//! - `CNAME` file containing a `.surge.sh` domain
//!
//! ## Authentication
//!
//! - `SURGE_TOKEN` environment variable (CI/CD)
//! - `surge login` interactive flow (local)

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
use crate::providers::helpers::{combined_output, has_env_var};

/// Surge provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SurgeConfig {
    /// Surge domain (e.g. "my-project.surge.sh").
    pub domain: Option<String>,
}

/// Surge deploy provider.
pub struct SurgeProvider;

#[async_trait]
impl DeployProvider for SurgeProvider {
    fn name(&self) -> &str {
        "Surge"
    }

    fn slug(&self) -> &str {
        "surge"
    }

    fn description(&self) -> &str {
        "Deploy to Surge.sh (simple static web publishing)"
    }

    fn cli_tool(&self) -> &str {
        "surge"
    }

    fn auth_env_var(&self) -> &str {
        "SURGE_TOKEN"
    }

    async fn check_prerequisites(
        &self,
        sandbox: &dyn SandboxProvider,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("surge --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("SURGE_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "surge".into(),
                install_hint: "npm i -g surge".into(),
            }),
            (true, false) => Ok(PrerequisiteStatus::AuthMissing {
                env_var: "SURGE_TOKEN".into(),
                login_hint: "surge login".into(),
            }),
        }
    }

    async fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        for dir in &[".", "dist", "build", "public", "_site"] {
            let cname = project_dir.join(dir).join("CNAME");
            if let Ok(content) = tokio::fs::read_to_string(&cname).await {
                if content.trim().ends_with(".surge.sh") {
                    return Ok(Some(DetectedConfig {
                        config_file: format!("{}/CNAME", dir),
                        is_linked: true,
                        project_name: Some(content.trim().to_string()),
                        team: None,
                    }));
                }
            }
        }
        Ok(None)
    }

    async fn setup(&self, ctx: &mut SetupContext) -> DeployResult<serde_json::Value> {
        if ctx.is_ci {
            return Err(DeployError::CiMissingConfig);
        }

        let _ = ui::layout::section_title("Setting up Surge deployment");

        let cli_ok = ctx.sandbox.exec("surge --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(&*ctx.sandbox).await?;
        }

        // Login if needed
        if !has_env_var("SURGE_TOKEN") {
            let _ = ui::status::info("Logging in to Surge...");
            let _ = ctx.exec_interactive("surge login").await?;
        }

        let config = SurgeConfig::default();
        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "surge".into(),
                url: "https://dry-run.surge.sh".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let surge_config = ctx.deploy_config
            .get_target_config::<SurgeConfig>("surge")?
            .unwrap_or_default();

        let output_path = ctx.output_path();
        let output_str = output_path.to_string_lossy();

        let domain_arg = surge_config.domain.as_deref()
            .map(|d| format!(" {}", d))
            .unwrap_or_default();

        let cmd = format!("surge {}{}", output_str, domain_arg);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Surge".into(),
                reason: combined_output(&result),
            });
        }

        let url = surge_config
            .domain
            .map(|d| format!("https://{}", d))
            .unwrap_or_else(|| "https://surge.sh".into());

        Ok(DeployOutput {
            provider: "surge".into(),
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
            provider: "Surge".into(),
            operation: "preview deployments".into(),
        })
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
