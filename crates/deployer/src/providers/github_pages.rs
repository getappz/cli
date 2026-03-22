//! GitHub Pages deploy provider.
//!
//! Deploys static sites to GitHub Pages via `gh-pages` npm package
//! or direct git push to the `gh-pages` branch.
//!
//! ## Detection
//!
//! - `.github/workflows/*.yml` containing pages deploy actions
//! - `CNAME` file in output directory
//!
//! ## Authentication
//!
//! - `GITHUB_TOKEN` environment variable (CI/CD, auto-set in GitHub Actions)
//! - Git credentials (local)

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
use crate::providers::helpers::{combined_output, has_env_var};

/// GitHub Pages provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPagesConfig {
    /// Custom domain (CNAME).
    pub custom_domain: Option<String>,
    /// Branch to deploy to (default: "gh-pages").
    pub branch: Option<String>,
    /// Repository (owner/repo format). Auto-detected from git remote.
    pub repository: Option<String>,
}

/// GitHub Pages deploy provider.
pub struct GitHubPagesProvider;

#[async_trait]
impl DeployProvider for GitHubPagesProvider {
    fn name(&self) -> &str {
        "GitHub Pages"
    }

    fn slug(&self) -> &str {
        "github-pages"
    }

    fn description(&self) -> &str {
        "Deploy to GitHub Pages (free static hosting from GitHub)"
    }

    fn cli_tool(&self) -> &str {
        "gh"
    }

    fn auth_env_var(&self) -> &str {
        "GITHUB_TOKEN"
    }

    async fn check_prerequisites(
        &self,
        sandbox: Arc<dyn SandboxProvider>,
    ) -> DeployResult<PrerequisiteStatus> {
        let has_gh = sandbox.exec("gh --version").await.map(|o| o.success()).unwrap_or(false);
        let has_npx = sandbox.exec("npx --version").await.map(|o| o.success()).unwrap_or(false);
        let has_auth = has_env_var("GITHUB_TOKEN") || has_env_var("GH_TOKEN");

        if !has_gh && !has_npx {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "gh".into(),
                install_hint: "Install GitHub CLI: https://cli.github.com/ or use npx gh-pages".into(),
            });
        }

        if !has_auth && has_gh {
            // Check if gh is authenticated
            let result = sandbox.exec("gh auth status").await;
            if result.map(|o| o.success()).unwrap_or(false) {
                return Ok(PrerequisiteStatus::Ready);
            }
        }

        if has_auth {
            Ok(PrerequisiteStatus::Ready)
        } else {
            Ok(PrerequisiteStatus::AuthMissing {
                env_var: "GITHUB_TOKEN".into(),
                login_hint: "gh auth login".into(),
            })
        }
    }

    async fn detect_config(&self, project_dir: PathBuf) -> DeployResult<Option<DetectedConfig>> {
        let workflows_dir = project_dir.join(".github/workflows");
        if workflows_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "yml" || ext == "yaml") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            let lower = content.to_lowercase();
                            if lower.contains("deploy-pages") || lower.contains("gh-pages") {
                                return Ok(Some(DetectedConfig {
                                    config_file: format!(
                                        ".github/workflows/{}",
                                        path.file_name().unwrap_or_default().to_string_lossy()
                                    ),
                                    is_linked: true,
                                    project_name: None,
                                    team: None,
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn setup(&self, ctx: &mut SetupContext) -> DeployResult<serde_json::Value> {
        if ctx.is_ci {
            return Err(DeployError::CiMissingConfig);
        }

        let _ = ui::layout::section_title("Setting up GitHub Pages deployment");
        let _ = ui::status::info("GitHub Pages will deploy from the 'gh-pages' branch.");

        let config = GitHubPagesConfig {
            branch: Some("gh-pages".into()),
            ..Default::default()
        };

        serde_json::to_value(config).map_err(|e| DeployError::JsonError {
            reason: e.to_string(),
        })
    }

    async fn deploy(&self, ctx: DeployContext) -> DeployResult<DeployOutput> {
        let start = std::time::Instant::now();

        if ctx.dry_run {
            return Ok(DeployOutput::dry_run("github-pages", false));
        }

        // Use npx gh-pages to deploy via sandbox
        let cmd = format!("npx gh-pages -d {} --dotfiles", ctx.output_dir);
        let result = ctx.exec(&cmd).await?;

        if !result.success() {
            return Err(DeployError::DeployFailed {
                provider: "GitHub Pages".into(),
                reason: combined_output(&result),
            });
        }

        // Try to determine the pages URL from git remote
        let url = detect_github_pages_url(&ctx).await
            .unwrap_or_else(|| "https://github.io".into());

        Ok(DeployOutput {
            provider: "github-pages".into(),
            url,
            additional_urls: vec![],
            deployment_id: None,
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }

    async fn deploy_preview(&self, _ctx: DeployContext) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: "GitHub Pages".into(),
            operation: "preview deployments".into(),
        })
    }
}

/// Try to detect the GitHub Pages URL from the git remote via sandbox.
async fn detect_github_pages_url(ctx: &DeployContext) -> Option<String> {
    let result = ctx.exec("git remote get-url origin").await.ok()?;
    if !result.success() {
        return None;
    }
    let remote = result.stdout_trimmed();

    // Parse owner/repo from remote URL
    let (owner, repo) = if remote.contains("github.com") {
        let parts: Vec<&str> = remote.trim_end_matches(".git").split('/').collect();
        if parts.len() >= 2 {
            let repo = parts[parts.len() - 1];
            let owner = parts[parts.len() - 2];
            // Handle SSH format (git@github.com:owner/repo)
            let owner = owner.split(':').next_back().unwrap_or(owner);
            (owner.to_string(), repo.to_string())
        } else {
            return None;
        }
    } else {
        return None;
    };

    Some(format!("https://{}.github.io/{}", owner, repo))
}
