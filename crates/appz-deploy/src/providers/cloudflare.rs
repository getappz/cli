//! Cloudflare Workers deploy provider.
//!
//! ## Scope
//! Workers only (`wrangler deploy`) — Cloudflare Pages is a separate product
//! with its own `wrangler pages deploy` subcommand and isn't covered here.
//! Workers is Cloudflare's current recommended path for new static/full-stack
//! projects (via the `assets` config, as appz-dev-site itself uses).
//!
//! ## Detection
//! No linked-state file exists for Wrangler (unlike Vercel/Netlify's
//! `.vercel/project.json` / `.netlify/state.json`) — project identity comes
//! purely from the config file's `name` field. Checked in the priority order
//! Cloudflare's own docs recommend: `wrangler.jsonc`, `wrangler.json`,
//! `wrangler.toml`.
//!
//! ## Authentication
//! `CLOUDFLARE_API_TOKEN` environment variable (CI/CD) or `wrangler login`
//! (local). `CLOUDFLARE_ACCOUNT_ID` is only required when the token has
//! access to more than one account — not checked here, same as Vercel/Netlify
//! only checking their primary auth token's presence.
//!
//! ## Build model
//! `wrangler deploy` uploads a new Worker version and immediately routes all
//! production traffic to it. `wrangler versions upload` uploads a version
//! without shifting traffic — it gets its own preview URL — which is the
//! idiomatic Workers equivalent of a "preview deploy".

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider};

pub struct CloudflareProvider;

impl CloudflareProvider {
    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let bin = "wrangler";
        let args: &[&str] = if is_preview {
            &["versions", "upload"]
        } else {
            &["deploy"]
        };
        let deploy =
            ctx.exec(bin, args, DEPLOY_TIMEOUT)
                .map_err(|reason| DeployError::DeployFailed {
                    provider: self.name(),
                    reason,
                })?;
        if !deploy.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "wrangler {} failed:\n{}\n{}",
                    args.join(" "),
                    deploy.stdout,
                    deploy.stderr
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

impl DeployProvider for CloudflareProvider {
    fn name(&self) -> &'static str {
        "Cloudflare"
    }
    fn slug(&self) -> &'static str {
        "cloudflare"
    }
    fn cli_tool(&self) -> &'static str {
        "wrangler"
    }
    fn install_package(&self) -> &'static str {
        "wrangler"
    }
    fn auth_env_var(&self) -> &'static str {
        "CLOUDFLARE_API_TOKEN"
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("wrangler").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "wrangler".into(),
                install_hint: "npm i -g wrangler".into(),
            });
        }
        if std::env::var("CLOUDFLARE_API_TOKEN").is_err() {
            return Ok(PrerequisiteStatus::AuthMissing {
                env_var: "CLOUDFLARE_API_TOKEN".into(),
                login_hint: "wrangler login".into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        for name in ["wrangler.jsonc", "wrangler.json"] {
            let path = project_dir.join(name);
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let cleaned = appz_core::strip_jsonc(&content);
            let project_name = serde_json::from_str::<serde_json::Value>(&cleaned)
                .ok()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from));
            return Ok(Some(DetectedConfig {
                config_file: name.to_string(),
                is_linked: false,
                project_name,
                team: None,
            }));
        }
        if project_dir.join("wrangler.toml").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "wrangler.toml".into(),
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

/// Extract the real deployment URL from `wrangler`'s output. Scans for the
/// last bare `https://` line — same approach as the Vercel/Netlify providers
/// — and returns `None` — never a guessed `https://{name}.workers.dev` URL —
/// if nothing matches (a custom domain or `workers_dev: false` may mean no
/// URL appears in stdout at all).
fn extract_deploy_url(stdout: &str) -> Option<String> {
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
    fn takes_the_last_url_not_the_first() {
        let stdout = "Uploaded my-worker (1.23 sec)\nDeployed my-worker triggers\nhttps://my-worker.example.workers.dev\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://my-worker.example.workers.dev".to_string())
        );
    }

    #[test]
    fn returns_none_instead_of_fabricating_a_url() {
        let stdout = "Uploaded my-worker (1.23 sec)\nDeployed my-worker triggers (custom domain)\n";
        assert_eq!(extract_deploy_url(stdout), None);
    }

    #[test]
    fn detect_config_reads_name_from_wrangler_jsonc() {
        let dir = std::env::temp_dir().join(format!("appz-deploy-cf-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("wrangler.jsonc"),
            "{\n  // a comment\n  \"name\": \"my-worker\"\n}\n",
        )
        .unwrap();

        let config = CloudflareProvider
            .detect_config(&dir)
            .unwrap()
            .expect("wrangler.jsonc should be detected");
        assert_eq!(config.config_file, "wrangler.jsonc");
        assert!(!config.is_linked);
        assert_eq!(config.project_name.as_deref(), Some("my-worker"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_config_none_when_no_config_file() {
        let dir =
            std::env::temp_dir().join(format!("appz-deploy-cf-test-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert!(CloudflareProvider.detect_config(&dir).unwrap().is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
