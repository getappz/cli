//! Vercel deploy provider.
//!
//! Deploys static sites via the Vercel CLI (`vercel`).
//!
//! ## Detection
//!
//! - `vercel.json` — project configuration
//! - `.vercel/project.json` — linked project state
//!
//! ## Authentication
//!
//! - `VERCEL_TOKEN` environment variable (CI/CD)
//! - `vercel login` interactive flow (local)

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::config::{DeployContext, SetupContext};
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::DeployProvider;
use crate::providers::helpers::{combined_output, extract_url_from_output, get_env_var, has_env_var};

/// Vercel provider configuration stored in `appz.json` deploy targets.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VercelConfig {
    /// Vercel project name.
    pub project_name: Option<String>,
    /// Vercel team slug or ID.
    pub team: Option<String>,
    /// Deployment environment ("production" or "preview").
    pub environment: Option<String>,
    /// Vercel organization ID.
    pub org_id: Option<String>,
    /// Vercel project ID.
    pub project_id: Option<String>,
}

/// Vercel deploy provider.
pub struct VercelProvider;

#[async_trait]
impl DeployProvider for VercelProvider {
    fn name(&self) -> &str {
        "Vercel"
    }

    fn slug(&self) -> &str {
        "vercel"
    }

    fn description(&self) -> &str {
        "Deploy to Vercel's global edge network"
    }

    fn cli_tool(&self) -> &str {
        "vercel"
    }

    fn auth_env_var(&self) -> &str {
        "VERCEL_TOKEN"
    }

    async fn check_prerequisites(
        &self,
        sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("vercel --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("VERCEL_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "vercel".into(),
                install_hint: "npm i -g vercel".into(),
            }),
            (true, false) => {
                // Check if logged in via vercel CLI config
                let result = sandbox.exec("vercel whoami").await;
                if result.map(|o| o.success()).unwrap_or(false) {
                    Ok(PrerequisiteStatus::Ready)
                } else {
                    Ok(PrerequisiteStatus::AuthMissing {
                        env_var: "VERCEL_TOKEN".into(),
                        login_hint: "vercel login".into(),
                    })
                }
            }
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        // Check for .vercel/project.json (linked project)
        let state_path = project_dir.join(".vercel/project.json");
        if state_path.exists() {
            let content = tokio::fs::read_to_string(&state_path).await?;
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(DetectedConfig {
                    config_file: ".vercel/project.json".into(),
                    is_linked: true,
                    project_name: state
                        .get("projectId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    team: state
                        .get("orgId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                }));
            }
        }

        // Check for vercel.json
        let config_path = project_dir.join("vercel.json");
        if config_path.exists() {
            return Ok(Some(DetectedConfig {
                config_file: "vercel.json".into(),
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

        let _ = ui::layout::section_title("Setting up Vercel deployment");

        // Ensure Vercel CLI is installed via sandbox
        let cli_ok = ctx.sandbox.exec("vercel --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(ctx.sandbox.clone()).await?;
        }

        // Run vercel link to connect the project
        let _ = ui::status::info("Linking project to Vercel...");
        let status = ctx.exec_interactive("vercel link").await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: "Failed to link project to Vercel".into(),
            });
        }

        // Read the generated .vercel/project.json via sandbox fs
        let config = if ctx.fs().exists(".vercel/project.json") {
            let content = ctx.fs().read_to_string(".vercel/project.json")?;
            let state: serde_json::Value = serde_json::from_str(&content)?;

            VercelConfig {
                project_id: state.get("projectId").and_then(|v| v.as_str()).map(String::from),
                org_id: state.get("orgId").and_then(|v| v.as_str()).map(String::from),
                ..Default::default()
            }
        } else {
            VercelConfig::default()
        };

        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "vercel".into(),
                url: "https://dry-run.vercel.app".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        // Build command string
        let token_flag = get_env_var("VERCEL_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();
        let cmd = format!("vercel deploy --prod --yes{}", token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://vercel.app".into());

        let duration = start.elapsed().as_millis() as u64;

        Ok(DeployOutput {
            provider: "vercel".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(duration),
        })
    }

    async fn deploy_preview(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "vercel".into(),
                url: "https://dry-run-preview.vercel.app".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: true,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let token_flag = get_env_var("VERCEL_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();
        let cmd = format!("vercel deploy --yes{}", token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://vercel.app".into());

        Ok(DeployOutput {
            provider: "vercel".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: true,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn list_deployments(&self, ctx: DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        // Check if project is linked — vercel ls requires a linked project
        let linked = ctx.project_dir.join(".vercel/project.json").exists();
        if !linked {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: "Project not linked to Vercel. Run `appz deploy --init` or `vercel link` first.".into(),
            });
        }

        let token_flag = get_env_var("VERCEL_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();
        let cmd = format!("vercel ls -F json --yes{}", token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: combined_output(&result),
            });
        }

        // Parse the JSON output from vercel ls
        let deployments: Vec<DeploymentInfo> =
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&result.stdout()) {
                if let Some(arr) = value.as_array() {
                    arr.iter()
                        .filter_map(|item| {
                            Some(DeploymentInfo {
                                id: item.get("uid")?.as_str()?.to_string(),
                                url: item.get("url")?.as_str().map(|u| {
                                    if u.starts_with("https://") {
                                        u.to_string()
                                    } else {
                                        format!("https://{}", u)
                                    }
                                })?,
                                status: DeployStatus::Ready,
                                created_at: None,
                                is_current: false,
                                git_ref: None,
                            })
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

        Ok(deployments)
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
