//! Netlify deploy provider.
//!
//! ## Detection
//! - `.netlify/state.json` — linked site state (siteId)
//! - `netlify.toml` — project configuration
//!
//! ## Authentication
//! - `NETLIFY_AUTH_TOKEN` environment variable (CI/CD) or `netlify login`
//!
//! ## Build model
//! `netlify deploy --build` runs Netlify's own configured build (from
//! `netlify.toml` / site settings) before deploying — same "drive the
//! platform's own CLI" principle as Vercel.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider};

pub struct NetlifyProvider;

impl NetlifyProvider {
    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let mut args = vec!["deploy", "--build"];
        if !is_preview {
            args.push("--prod");
        }
        let deploy = ctx
            .exec("netlify", &args, DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !deploy.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "netlify deploy failed:\n{}\n{}",
                    deploy.stdout, deploy.stderr
                ),
            });
        }

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: extract_deploy_url(&deploy.stdout),
            is_preview,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
        })
    }
}

impl DeployProvider for NetlifyProvider {
    fn name(&self) -> &'static str {
        "Netlify"
    }
    fn slug(&self) -> &'static str {
        "netlify"
    }
    fn cli_tool(&self) -> &'static str {
        "netlify"
    }
    fn install_package(&self) -> &'static str {
        // The binary is `netlify`; the npm package is `netlify-cli` — the
        // upstream defect this fixes assumed `npm install -g netlify`.
        "netlify-cli"
    }
    fn auth_env_var(&self) -> &'static str {
        "NETLIFY_AUTH_TOKEN"
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("netlify").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "netlify".into(),
                install_hint: "npm i -g netlify-cli".into(),
            });
        }
        if std::env::var("NETLIFY_AUTH_TOKEN").is_err() {
            return Ok(PrerequisiteStatus::AuthMissing {
                env_var: "NETLIFY_AUTH_TOKEN".into(),
                login_hint: "netlify login".into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let state_path = project_dir.join(".netlify/state.json");
        if state_path.exists() {
            let content = std::fs::read_to_string(&state_path)?;
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(DetectedConfig {
                    config_file: ".netlify/state.json".into(),
                    is_linked: true,
                    project_name: state
                        .get("siteId")
                        .and_then(|v| v.as_str())
                        .map(String::from),
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

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }
}

/// Extract the real deployment URL from `netlify deploy`'s output.
///
/// `netlify deploy --json` is documented to emit deploy metadata as JSON, but
/// exact field names have shifted across CLI major versions and this hasn't
/// been verified against a live run — so this tries several known-plausible
/// keys and falls back to a bare `https://` line scan rather than asserting
/// one schema with false confidence. Returns `None` — never a guessed URL —
/// if nothing matches.
fn extract_deploy_url(stdout: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        for key in ["deploy_url", "url", "site_url"] {
            if let Some(url) = value.get(key).and_then(|v| v.as_str()) {
                return Some(url.to_string());
            }
        }
    }
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deploy_url_from_json_output() {
        let stdout = r#"{"deploy_url": "https://abc123--site.netlify.app", "url": "https://site.netlify.app"}"#;
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://abc123--site.netlify.app".to_string())
        );
    }

    #[test]
    fn falls_back_to_last_https_line_when_not_json() {
        let stdout = "Building...\nDeploy is live!\nhttps://abc123--site.netlify.app\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://abc123--site.netlify.app".to_string())
        );
    }

    #[test]
    fn returns_none_instead_of_fabricating_a_url() {
        assert_eq!(extract_deploy_url("no url here"), None);
    }
}
