//! Firebase Hosting deploy provider.
//!
//! Deploys static sites via the Firebase CLI (`firebase`).
//!
//! ## Detection
//!
//! - `firebase.json` — project configuration
//! - `.firebaserc` — linked project
//!
//! ## Authentication
//!
//! - `FIREBASE_TOKEN` environment variable (CI/CD)
//! - `firebase login` interactive flow (local)

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
use crate::providers::helpers::{combined_output, extract_url_from_output, get_env_var, has_env_var};

/// Firebase provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirebaseConfig {
    /// Firebase project ID.
    pub project_id: Option<String>,
    /// Hosting site ID (for multi-site projects).
    pub site: Option<String>,
}

/// Firebase Hosting deploy provider.
pub struct FirebaseProvider;

#[async_trait]
impl DeployProvider for FirebaseProvider {
    fn name(&self) -> &str {
        "Firebase Hosting"
    }

    fn slug(&self) -> &str {
        "firebase"
    }

    fn description(&self) -> &str {
        "Deploy to Firebase Hosting with global CDN"
    }

    fn cli_tool(&self) -> &str {
        "firebase"
    }

    fn auth_env_var(&self) -> &str {
        "FIREBASE_TOKEN"
    }

    /// Override: the npm package is `firebase-tools`, not `firebase`.
    async fn ensure_cli(&self, sandbox: &dyn SandboxProvider) -> DeployResult<()> {
        let _ = ui::status::info("Installing Firebase CLI...");
        let output = sandbox.exec("npm install -g firebase-tools").await?;
        if !output.success() {
            return Err(DeployError::CommandFailed {
                command: "npm install -g firebase-tools".into(),
                reason: combined_output(&output),
            });
        }
        Ok(())
    }

    async fn check_prerequisites(
        &self,
        sandbox: &dyn SandboxProvider,
    ) -> DeployResult<PrerequisiteStatus> {
        let cli_ok = sandbox.exec("firebase --version").await.map(|o| o.success()).unwrap_or(false);
        let auth_ok = has_env_var("FIREBASE_TOKEN");

        match (cli_ok, auth_ok) {
            (true, true) => Ok(PrerequisiteStatus::Ready),
            (false, _) => Ok(PrerequisiteStatus::CliMissing {
                tool: "firebase".into(),
                install_hint: "npm i -g firebase-tools".into(),
            }),
            (true, false) => Ok(PrerequisiteStatus::AuthMissing {
                env_var: "FIREBASE_TOKEN".into(),
                login_hint: "firebase login".into(),
            }),
        }
    }

    async fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let firebaserc = project_dir.join(".firebaserc");
        if firebaserc.exists() {
            let content = tokio::fs::read_to_string(&firebaserc).await?;
            let project_id = serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| {
                    v.get("projects")
                        .and_then(|p| p.get("default"))
                        .and_then(|d| d.as_str())
                        .map(String::from)
                });
            return Ok(Some(DetectedConfig {
                config_file: ".firebaserc".into(),
                is_linked: true,
                project_name: project_id,
                team: None,
            }));
        }

        if project_dir.join("firebase.json").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "firebase.json".into(),
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

        let _ = ui::layout::section_title("Setting up Firebase Hosting deployment");

        let cli_ok = ctx.sandbox.exec("firebase --version").await.map(|o| o.success()).unwrap_or(false);
        if !cli_ok {
            self.ensure_cli(&*ctx.sandbox).await?;
        }

        let _ = ui::status::info("Initializing Firebase Hosting...");
        let status = ctx.exec_interactive("firebase init hosting").await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Firebase Hosting".into(),
                reason: "Failed to initialize Firebase Hosting".into(),
            });
        }

        let config = FirebaseConfig::default();
        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput {
                provider: "firebase".into(),
                url: "https://dry-run.web.app".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: false,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let token_flag = get_env_var("FIREBASE_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();
        let cmd = format!("firebase deploy --only hosting{}", token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Firebase Hosting".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://web.app".into());

        Ok(DeployOutput {
            provider: "firebase".into(),
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
                provider: "firebase".into(),
                url: "https://dry-run-preview.web.app".into(),
                additional_urls: vec![],
                deployment_id: None,
                is_preview: true,
                status: DeployStatus::Ready,
                created_at: Some(chrono::Utc::now()),
                duration_ms: Some(0),
            });
        }

        let token_flag = get_env_var("FIREBASE_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();
        let cmd = format!("firebase hosting:channel:deploy preview{}", token_flag);

        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "Firebase Hosting".into(),
                reason: combined_output(&result),
            });
        }

        let url = extract_url_from_output(&result.stdout())
            .or_else(|| extract_url_from_output(&result.stderr()))
            .unwrap_or_else(|| "https://web.app".into());

        Ok(DeployOutput {
            provider: "firebase".into(),
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
        false
    }

    fn supports_custom_domains(&self) -> bool {
        true
    }
}
