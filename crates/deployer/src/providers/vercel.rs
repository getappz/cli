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
use crate::providers::helpers::{combined_output, get_env_var, has_env_var};

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

impl VercelProvider {
    async fn deploy_internal(&self, ctx: DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput::dry_run("vercel", is_preview));
        }

        // Ensure vercel.json has outputDirectory (and skip build if output already exists)
        let skip_build = has_prebuilt_output(&ctx.project_dir, &ctx.output_dir);
        ensure_vercel_json(&ctx.project_dir, &ctx.output_dir, skip_build)?;
        if skip_build {
            ensure_vercelignore(&ctx.project_dir, &ctx.output_dir);
        }

        // Handle --prebuilt target mismatch before deploying
        if ctx.prebuilt {
            let requested_target = if is_preview { "preview" } else { "production" };
            resolve_prebuilt_target_mismatch(&ctx, requested_target).await?;
        }

        let token_flag = get_env_var("VERCEL_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();

        let prod_flag = if is_preview { "" } else { " --prod" };
        let prebuilt_flag = if ctx.prebuilt { " --prebuilt" } else { "" };
        let cmd = format!("vercel deploy{}{} --yes{}", prebuilt_flag, prod_flag, token_flag);

        let status = ctx.exec_interactive(&cmd).await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: format!("vercel deploy exited with code {}", status.code().unwrap_or(-1)),
            });
        }

        let url = read_vercel_project_url(&ctx.project_dir)
            .unwrap_or_else(|| "https://vercel.app".into());

        Ok(DeployOutput {
            provider: "vercel".into(),
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
        self.deploy_internal(ctx, false).await
    }

    async fn deploy_preview(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true).await
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

/// Read the project URL from .vercel/project.json after a deployment.
/// Ensure .vercelignore exists to exclude source files when deploying
/// pre-built static output (e.g. WordPress projects).
fn ensure_vercelignore(project_dir: &std::path::Path, output_dir: &str) {
    let ignore_path = project_dir.join(".vercelignore");
    // Only create if it doesn't exist — don't overwrite user customizations
    if ignore_path.exists() {
        return;
    }

    // Ignore everything except the output directory
    let content = format!(
        "# Auto-generated by appz deploy — only upload the build output\n\
         *\n\
         !{}\n\
         !{}/**\n\
         !vercel.json\n",
        output_dir, output_dir
    );

    let _ = std::fs::write(&ignore_path, content);
}

/// Check if the output directory already contains built files.
fn has_prebuilt_output(project_dir: &std::path::Path, output_dir: &str) -> bool {
    let output_path = project_dir.join(output_dir);
    output_path.is_dir()
        && std::fs::read_dir(&output_path)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
}

/// Ensure vercel.json exists with the correct outputDirectory.
///
/// If vercel.json doesn't exist, creates it. If it exists, updates
/// outputDirectory without overwriting other settings. When `skip_build`
/// is true, also sets buildCommand to empty so Vercel doesn't try to
/// build (the output is pre-built, e.g. from appz wp-export).
fn ensure_vercel_json(
    project_dir: &std::path::Path,
    output_dir: &str,
    skip_build: bool,
) -> DeployResult<()> {
    let vercel_json_path = project_dir.join("vercel.json");

    let mut config: serde_json::Value = if vercel_json_path.exists() {
        let content = std::fs::read_to_string(&vercel_json_path).map_err(|e| {
            DeployError::CommandFailed {
                command: "read vercel.json".into(),
                reason: e.to_string(),
            }
        })?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "outputDirectory".to_string(),
            serde_json::Value::String(output_dir.to_string()),
        );
        if skip_build {
            // Empty buildCommand tells Vercel to skip building
            obj.insert(
                "buildCommand".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
    }

    let json_str = serde_json::to_string_pretty(&config).map_err(|e| {
        DeployError::CommandFailed {
            command: "serialize vercel.json".into(),
            reason: e.to_string(),
        }
    })?;

    std::fs::write(&vercel_json_path, json_str).map_err(|e| {
        DeployError::CommandFailed {
            command: "write vercel.json".into(),
            reason: e.to_string(),
        }
    })?;

    Ok(())
}

/// Read the target from `.vercel/output/builds.json`.
///
/// Returns the `target` field (e.g. "preview" or "production"), or `None` if
/// the file doesn't exist or the field is missing.
fn read_prebuilt_target(project_dir: &std::path::Path) -> Option<String> {
    let builds_path = project_dir.join(".vercel/output/builds.json");
    let content = std::fs::read_to_string(&builds_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value.get("target")?.as_str().map(String::from)
}

/// Update the target in `.vercel/output/builds.json`.
fn update_prebuilt_target(project_dir: &std::path::Path, target: &str) -> DeployResult<()> {
    let builds_path = project_dir.join(".vercel/output/builds.json");
    let content = std::fs::read_to_string(&builds_path).map_err(|e| DeployError::CommandFailed {
        command: "read .vercel/output/builds.json".into(),
        reason: e.to_string(),
    })?;
    let mut value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "target".to_string(),
            serde_json::Value::String(target.to_string()),
        );
    }
    let json_str =
        serde_json::to_string_pretty(&value).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })?;
    std::fs::write(&builds_path, json_str).map_err(|e| DeployError::CommandFailed {
        command: "write .vercel/output/builds.json".into(),
        reason: e.to_string(),
    })?;
    Ok(())
}

/// Detect and resolve a prebuilt target mismatch.
///
/// When `--prebuilt` is used, the `.vercel/output/builds.json` contains the
/// target environment the output was built for. If it doesn't match the
/// requested target:
///
/// - **WordPress projects**: the prebuilt output is framework-agnostic static
///   HTML, so we can safely update the target in `builds.json` directly.
/// - **Other projects**: prompt the user and rebuild with `vercel build` using
///   the correct target.
async fn resolve_prebuilt_target_mismatch(
    ctx: &crate::config::DeployContext,
    requested_target: &str,
) -> DeployResult<()> {
    let current_target = match read_prebuilt_target(&ctx.project_dir) {
        Some(t) => t,
        None => return Ok(()), // No builds.json or no target field — let Vercel handle it
    };

    if current_target == requested_target {
        return Ok(());
    }

    let is_wordpress = ctx
        .framework
        .as_deref()
        .is_some_and(|f| f == "wordpress");

    if is_wordpress {
        // WordPress static exports are target-agnostic — just update builds.json
        let _ = ui::status::info(&format!(
            "Prebuilt output target is \"{}\", updating to \"{}\" for WordPress project...",
            current_target, requested_target
        ));
        update_prebuilt_target(&ctx.project_dir, requested_target)?;
    } else {
        // Non-WordPress: prompt user to rebuild with the correct target
        let _ = ui::status::warning(&format!(
            "Prebuilt output was built for \"{}\", but deploying to \"{}\".",
            current_target, requested_target
        ));

        let rebuild = ui::prompt::confirm(
            &format!(
                "Rebuild with `vercel build --prod` for {} target?",
                requested_target
            ),
            true,
        )
        .unwrap_or(false);

        if !rebuild {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: format!(
                    "Prebuilt output target \"{}\" does not match requested target \"{}\". \
                     Rebuild with the correct target or deploy without --prebuilt.",
                    current_target, requested_target
                ),
            });
        }

        // Rebuild with the correct target
        let _ = ui::status::info(&format!(
            "Rebuilding for {} target...",
            requested_target
        ));

        let token_flag = get_env_var("VERCEL_TOKEN")
            .map(|t| format!(" --token {}", t))
            .unwrap_or_default();

        let prod_flag = if requested_target == "production" {
            " --prod"
        } else {
            ""
        };

        let build_cmd = format!("vercel build --yes{}{}", prod_flag, token_flag);
        let status = ctx.exec_interactive(&build_cmd).await?;

        if !status.success() {
            return Err(DeployError::DeployFailed {
                provider: "Vercel".into(),
                reason: format!(
                    "vercel build exited with code {}",
                    status.code().unwrap_or(-1)
                ),
            });
        }
    }

    Ok(())
}

/// Best-effort project URL from directory name.
/// The actual deployment URL is printed by the Vercel CLI interactively.
fn read_vercel_project_url(project_dir: &std::path::Path) -> Option<String> {
    let name = project_dir.file_name()?.to_str()?;
    Some(format!("https://{}.vercel.app", name))
}
