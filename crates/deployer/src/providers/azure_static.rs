//! Azure Static Web Apps deploy provider.
//!
//! Deploys static sites via the SWA CLI (`swa`).
//!
//! ## Detection
//!
//! - `staticwebapp.config.json` — SWA configuration
//! - `swa-cli.config.json` — SWA CLI configuration
//!
//! ## Authentication
//!
//! - `SWA_CLI_DEPLOYMENT_TOKEN` environment variable (CI/CD)
//! - `swa login` interactive flow (local)

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
use crate::providers::helpers::{combined_output, extract_url_from_output, get_env_var, has_env_var};

/// Azure Static Web Apps provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AzureStaticConfig {
    /// Azure subscription ID.
    pub subscription_id: Option<String>,
    /// Resource group name.
    pub resource_group: Option<String>,
    /// Static web app name.
    pub app_name: Option<String>,
}

/// Azure Static Web Apps deploy provider.
pub struct AzureStaticProvider;

#[async_trait]
impl DeployProvider for AzureStaticProvider {
    fn name(&self) -> &str {
        "Azure Static Web Apps"
    }

    fn slug(&self) -> &str {
        "azure-static"
    }

    fn description(&self) -> &str {
        "Deploy to Azure Static Web Apps"
    }

    fn cli_tool(&self) -> &str {
        "swa"
    }

    fn auth_env_var(&self) -> &str {
        "SWA_CLI_DEPLOYMENT_TOKEN"
    }

    /// Override: the npm package is `@azure/static-web-apps-cli`, not `swa`.
    async fn ensure_cli(&self, sandbox: Arc<dyn SandboxProvider>) -> DeployResult<()> {
        let _ = ui::status::info("Installing SWA CLI...");
        let output = sandbox.exec("npm install -g @azure/static-web-apps-cli").await?;
        if !output.success() {
            return Err(DeployError::CommandFailed {
                command: "npm install -g @azure/static-web-apps-cli".into(),
                reason: combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check_prerequisites(
        &self,
        sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("swa --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("SWA_CLI_DEPLOYMENT_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "swa".into(),
                install_hint: "npm i -g @azure/static-web-apps-cli".into(),
            }),
            (true, false) => Ok(PrerequisiteStatus::AuthMissing {
                env_var: "SWA_CLI_DEPLOYMENT_TOKEN".into(),
                login_hint: "swa login".into(),
            }),
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        for name in &["staticwebapp.config.json", "swa-cli.config.json"] {
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

        let _ = ui::layout::section_title("Setting up Azure Static Web Apps deployment");

        let cli_ok = ctx.sandbox.exec("swa --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(ctx.sandbox.clone()).await?;
        }

        let _ = ui::status::info("Initializing SWA CLI...");
        let status = ctx.exec_interactive("swa init").await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Azure Static Web Apps".into(),
                reason: "Failed to initialize SWA CLI".into(),
            });
        }

        let config = AzureStaticConfig::default();
        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "azure-static".into(),
                url: "https://dry-run.azurestaticapps.net".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let token_flag = get_env_var("SWA_CLI_DEPLOYMENT_TOKEN")
            .map(|t| format!(" --deployment-token {}", t))
            .unwrap_or_default();

        let cmd = format!("swa deploy --app-location {}{}", ctx.output_dir, token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Azure Static Web Apps".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://azurestaticapps.net".into());

        Ok(DeployOutput {
            provider: "azure-static".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn deploy_preview(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "azure-static".into(),
                url: "https://dry-run-preview.azurestaticapps.net".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: true,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let token_flag = get_env_var("SWA_CLI_DEPLOYMENT_TOKEN")
            .map(|t| format!(" --deployment-token {}", t))
            .unwrap_or_default();

        let cmd = format!("swa deploy --app-location {} --env preview{}", ctx.output_dir, token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Azure Static Web Apps".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://azurestaticapps.net".into());

        Ok(DeployOutput {
            provider: "azure-static".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: true,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
