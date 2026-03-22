//! Netlify deploy provider.
//!
//! Deploys static sites via the Netlify CLI (`netlify`).
//!
//! ## Detection
//!
//! - `netlify.toml` — project configuration
//! - `.netlify/state.json` — linked project state
//!
//! ## Authentication
//!
//! - `NETLIFY_AUTH_TOKEN` environment variable (CI/CD)
//! - `netlify login` interactive flow (local)

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
use crate::providers::helpers::{combined_output, extract_url_from_output, has_env_var};

/// Netlify provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetlifyConfig {
    /// Netlify site ID.
    pub site_id: Option<String>,
    /// Build output directory (overrides top-level outputDirectory).
    pub publish_dir: Option<String>,
}

/// Netlify deploy provider.
pub struct NetlifyProvider;

impl NetlifyProvider {
    async fn deploy_internal(&self, ctx: DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput::dry_run("netlify", is_preview));
        }

        let site_flag = ctx.deploy_config
            .get_target_config::<NetlifyConfig>("netlify")?
            .and_then(|c| c.site_id)
            .map(|id| format!(" --site {}", id))
            .unwrap_or_default();

        let prod_flag = if is_preview { "" } else { " --prod" };
        let cmd = format!("netlify deploy{} --dir {}{}", prod_flag, ctx.output_dir, site_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Netlify".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://netlify.app".into());

        Ok(DeployOutput {
            provider: "netlify".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }
}

#[async_trait]
impl DeployProvider for NetlifyProvider {
    fn name(&self) -> &str {
        "Netlify"
    }

    fn slug(&self) -> &str {
        "netlify"
    }

    fn description(&self) -> &str {
        "Deploy to Netlify's composable web platform"
    }

    fn cli_tool(&self) -> &str {
        "netlify"
    }

    fn auth_env_var(&self) -> &str {
        "NETLIFY_AUTH_TOKEN"
    }

    /// Override default ensure_cli since the npm package is `netlify-cli`, not `netlify`.
    async fn ensure_cli(&self, sandbox: Arc<dyn SandboxProvider>) -> DeployResult<()> {
        let _ = ui::status::info("Installing Netlify CLI...");
        let output = sandbox.exec("npm install -g netlify-cli").await?;
        if !output.success() {
            return Err(DeployError::CommandFailed {
                command: "npm install -g netlify-cli".into(),
                reason: combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check_prerequisites(
        &self,
        sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("netlify --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("NETLIFY_AUTH_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "netlify".into(),
                install_hint: "npm i -g netlify-cli".into(),
            }),
            (true, false) => {
                let result = sandbox.exec("netlify status").await;
                if result.map(|o| o.success()).unwrap_or(false) {
                    Ok(PrerequisiteStatus::Ready)
                } else {
                    Ok(PrerequisiteStatus::AuthMissing {
                        env_var: "NETLIFY_AUTH_TOKEN".into(),
                        login_hint: "netlify login".into(),
                    })
                }
            }
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        let state_path = project_dir.join(".netlify/state.json");
        if state_path.exists() {
            let content = tokio::fs::read_to_string(&state_path).await?;
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(DetectedConfig {
                    config_file: ".netlify/state.json".into(),
                    is_linked: true,
                    project_name: state
                        .get("siteId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    team: None,
                }));
            }
        }

        if project_dir.join("netlify.toml").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "netlify.toml".into(),
                is_linked: false,
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

        let _ = ui::layout::section_title("Setting up Netlify deployment");

        let cli_ok = ctx.sandbox.exec("netlify --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(ctx.sandbox.clone()).await?;
        }

        let _ = ui::status::info("Linking project to Netlify...");
        let status = ctx.exec_interactive("netlify init").await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Netlify".into(),
                reason: "Failed to link project to Netlify".into(),
            });
        }

        // Read the generated .netlify/state.json via sandbox fs
        let config = if ctx.fs().exists(".netlify/state.json") {
            let content = ctx.fs().read_to_string(".netlify/state.json")?;
            let state: serde_json::Value = serde_json::from_str(&content)?;
            NetlifyConfig {
                site_id: state.get("siteId").and_then(|v| v.as_str()).map(String::from),
                publish_dir: ctx.output_dir.clone(),
            }
        } else {
            NetlifyConfig {
                publish_dir: ctx.output_dir.clone(),
                ..Default::default()
            }
        };

        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false).await
    }

    async fn deploy_preview(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true).await
    }

    fn supports_env_vars(&self) -> bool {
        true
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }

    fn supports_rollback(&self) -> bool {
        true
    }
}
