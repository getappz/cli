//! Render deploys an existing service's connected source through its own CLI.
//! Configure `deploy.targets.render.service` with its ID (srv-...), and set
//! `RENDER_API_KEY` or use `render login`. URLs come from service metadata.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct RenderProvider;

impl RenderProvider {
    fn service<'a>(&self, ctx: &'a DeployContext) -> DeployResult<&'a str> {
        ctx.deploy_config
            .targets
            .get("render")
            .and_then(|v| v.get("service"))
            .and_then(|v| v.as_str())
            .filter(|v| {
                v.starts_with("srv-")
                    && v.len() > 4
                    && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            .ok_or_else(|| {
                self.fail(
                    "set deploy.targets.render.service to a service ID (srv-...) in appz.jsonc"
                        .into(),
                )
            })
    }

    fn fail(&self, reason: String) -> DeployError {
        DeployError::DeployFailed {
            provider: self.name(),
            reason,
        }
    }

    fn query(&self, ctx: &DeployContext, args: &[&str]) -> DeployResult<String> {
        let out = ctx
            .exec("render", args, QUERY_TIMEOUT)
            .map_err(|e| self.fail(e))?;
        if !out.success {
            return Err(self.fail(format!(
                "render command failed:\n{}\n{}",
                out.stdout, out.stderr
            )));
        }
        Ok(out.stdout)
    }
}

impl DeployProvider for RenderProvider {
    fn name(&self) -> &'static str {
        "Render"
    }
    fn slug(&self) -> &'static str {
        "render"
    }
    fn cli_tool(&self) -> &'static str {
        "render"
    }
    fn install_package(&self) -> &'static str {
        ""
    }
    fn mise_tool(&self) -> String {
        String::new()
    }
    fn install_hint(&self) -> Option<String> {
        Some("https://render.com/docs/cli".into())
    }
    fn auth_env_var(&self) -> &'static str {
        "RENDER_API_KEY"
    }
    fn whoami_args(&self) -> &'static [&'static str] {
        &["whoami", "--output", "json", "--confirm"]
    }
    fn needs_prebuild(&self, _: &[String]) -> bool {
        false
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path(self.cli_tool()).is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: self.cli_tool().into(),
                install_hint: self.install_hint().unwrap(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        Ok(dir.join("render.yaml").is_file().then(|| DetectedConfig {
            config_file: "render.yaml".into(),
            is_linked: false,
            project_name: None,
            team: None,
        }))
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        let service = self.service(ctx)?;
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), false));
        }
        let start = Instant::now();
        let out = ctx
            .exec_live(
                "render",
                &[
                    "deploys",
                    "create",
                    service,
                    "--wait",
                    "--output",
                    "json",
                    "--confirm",
                ],
                DEPLOY_TIMEOUT,
            )
            .map_err(|e| self.fail(e))?;
        if !out.success {
            return Err(self.fail(format!(
                "render deploy failed:\n{}\n{}",
                out.stdout, out.stderr
            )));
        }
        // Failure to look up the URL must not report a completed deploy as failed.
        let url = self
            .query(ctx, &["services", "--output", "json", "--confirm"])
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<serde_json::Value>>(&text).ok())
            .and_then(|services| {
                services.into_iter().find_map(|entry| {
                    let s = entry.get("service").unwrap_or(&entry);
                    (s.get("id").and_then(|v| v.as_str()) == Some(service))
                        .then(|| {
                            s.pointer("/serviceDetails/url")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        })
                        .flatten()
                })
            });
        let mut result = DeployOutput::dry_run(self.slug(), false);
        result.status = if url.is_some() {
            DeployStatus::Ready
        } else {
            DeployStatus::Unknown
        };
        result.url = url;
        result.created_at = Some(chrono::Utc::now());
        result.duration_ms = Some(start.elapsed().as_millis() as u64);
        Ok(result)
    }

    fn deploy_preview(&self, _: &DeployContext) -> DeployResult<DeployOutput> {
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "preview deploy (use --prod)",
        })
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let text = self.query(
            ctx,
            &[
                "deploys",
                "list",
                self.service(ctx)?,
                "--output",
                "json",
                "--confirm",
            ],
        )?;
        let deployments: Vec<serde_json::Value> = serde_json::from_str(&text)?;
        deployments
            .into_iter()
            .map(|entry| {
                let d = entry.get("deploy").unwrap_or(&entry);
                let id = d
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        self.fail("deploy list response is missing a deployment ID".into())
                    })?;
                let state = d.get("status").and_then(|v| v.as_str()).unwrap_or("");
                Ok(DeploymentInfo {
                    id: id.into(),
                    version_id: None,
                    url: None,
                    status: match state {
                        "live" | "deactivated" => DeployStatus::Ready,
                        "created"
                        | "build_in_progress"
                        | "update_in_progress"
                        | "pre_deploy_in_progress" => DeployStatus::Building,
                        "build_failed" | "update_failed" | "pre_deploy_failed" | "canceled" => {
                            DeployStatus::Error
                        }
                        _ => DeployStatus::Unknown,
                    },
                    created_at: d
                        .get("createdAt")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.with_timezone(&chrono::Utc)),
                    is_current: state == "live",
                })
            })
            .collect()
    }
    // Render's CLI has no deployment-ID rollback command. Inherit Unsupported;
    // redeploying a commit isn't an artifact rollback.
}
