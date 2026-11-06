//! Azure Static Web Apps deploy provider (`swa`).
//!
//! ## Scope
//! Static Web Apps only. `swa deploy` uploads a directory; Azure Functions
//! APIs bundled with an SWA deployment are not configured by appz.
//!
//! ## Detection
//! `swa-cli.config.json` (written by `swa init`) or
//! `staticwebapp.config.json` (SWA runtime configuration). Both mean the
//! project already knows it targets Azure Static Web Apps.
//!
//! ## Authentication
//! `SWA_CLI_DEPLOYMENT_TOKEN` is the deployment token from the Static Web App
//! resource (CI/CD), or `swa login` for an interactive Azure sign-in. There is
//! no stable read-only "who am I" probe, so sign-in is only reported when the
//! token variable is set; otherwise the CLI reports its own error.
//!
//! ## Build model
//! `swa deploy <dir> --env <name>` uploads appz's build output. appz builds
//! first, so the output directory must exist and be non-empty.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider};

pub struct AzureStaticProvider;

impl AzureStaticProvider {
    fn fail(&self, reason: String) -> DeployError {
        DeployError::DeployFailed {
            provider: self.name(),
            reason,
        }
    }

    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let output = ctx.output_path();
        if !std::fs::read_dir(&output).is_ok_and(|mut it| it.next().is_some()) {
            return Err(self.fail(format!(
                "build output '{}' is missing or empty: build the project first (appz build)",
                ctx.output_dir
            )));
        }
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let start = Instant::now();
        let env = if is_preview { "preview" } else { "production" };
        let args = ["deploy", ctx.output_dir.as_str(), "--env", env];
        let deploy = ctx
            .exec_live(self.cli_tool(), &args, DEPLOY_TIMEOUT)
            .map_err(|reason| self.fail(reason))?;
        if !deploy.success {
            return Err(self.fail(format!(
                "swa {} failed:\n{}\n{}",
                args.join(" "),
                deploy.stdout,
                deploy.stderr
            )));
        }

        let url = extract_deploy_url(&deploy.stdout);
        Ok(DeployOutput {
            provider: self.slug().to_string(),
            status: if url.is_some() {
                DeployStatus::Ready
            } else {
                DeployStatus::Unknown
            },
            url,
            is_preview,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        })
    }
}

impl DeployProvider for AzureStaticProvider {
    fn name(&self) -> &'static str {
        "Azure Static Web Apps"
    }
    fn slug(&self) -> &'static str {
        "azure-static"
    }
    fn cli_tool(&self) -> &'static str {
        "swa"
    }
    fn install_package(&self) -> &'static str {
        "@azure/static-web-apps-cli"
    }
    fn auth_env_var(&self) -> &'static str {
        "SWA_CLI_DEPLOYMENT_TOKEN"
    }
    fn whoami_args(&self) -> &'static [&'static str] {
        &[]
    }

    fn auth_status(&self, ctx: &DeployContext) -> AuthState {
        let var = self.auth_env_var();
        if ctx.run_env.get(var).is_some_and(|v| !v.is_empty())
            || std::env::var(var).is_ok_and(|v| !v.is_empty())
        {
            AuthState::Authenticated
        } else {
            AuthState::Unknown
        }
    }

    fn login_hint(&self) -> String {
        "swa login (or set SWA_CLI_DEPLOYMENT_TOKEN)".to_string()
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path(self.cli_tool()).is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: self.cli_tool().into(),
                install_hint: format!("npm install -g {}", self.install_package()),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let found = ["staticwebapp.config.json", "swa-cli.config.json"]
            .into_iter()
            .find(|f| project_dir.join(f).exists());
        Ok(found.map(|f| DetectedConfig {
            config_file: f.to_string(),
            // No link step: a deployment token is the whole binding.
            is_linked: true,
            project_name: None,
            team: None,
        }))
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }
}

/// The deployment URL from `swa deploy`'s output — only links on
/// `.azurestaticapps.net`, and the last one, because the trailing summary is
/// the deploy that just finished. Never fabricates a URL.
fn extract_deploy_url(stdout: &str) -> Option<String> {
    stdout
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '<' | '>' | '(' | ')'))
        .filter_map(|token| token.strip_prefix("https://").map(|rest| (token, rest)))
        .rfind(|(_, rest)| {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            host.ends_with(".azurestaticapps.net")
        })
        .map(|(token, _)| token.trim_end_matches(['.', ';']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("appz-deploy-azure-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_both_static_web_app_config_files() {
        let dir = temp("detect");
        std::fs::write(dir.join("staticwebapp.config.json"), "{}").unwrap();
        let config = AzureStaticProvider
            .detect_config(&dir)
            .unwrap()
            .expect("config should be detected");
        assert_eq!(config.config_file, "staticwebapp.config.json");
        assert!(config.is_linked);

        std::fs::remove_file(dir.join("staticwebapp.config.json")).unwrap();
        std::fs::write(dir.join("swa-cli.config.json"), "{}").unwrap();
        let config = AzureStaticProvider
            .detect_config(&dir)
            .unwrap()
            .expect("swa-cli config should be detected");
        assert_eq!(config.config_file, "swa-cli.config.json");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_azure_static_hosts_are_the_deployment_url() {
        let stdout = "Project deployed\nhttps://nice-name-0123.azurestaticapps.net\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://nice-name-0123.azurestaticapps.net".to_string())
        );
        assert_eq!(
            extract_deploy_url("https://portal.azure.com/x\nhttps://x.azurewebsites.net"),
            None
        );
    }

    #[test]
    fn refuses_to_deploy_a_missing_or_empty_build_output() {
        let dir = temp("empty");
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        let err = AzureStaticProvider.deploy(&ctx).unwrap_err().to_string();
        assert!(err.contains("missing or empty"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deploys_the_output_directory_and_reads_the_url() {
        let dir = temp("run");
        std::fs::create_dir_all(dir.join("dist")).unwrap();
        std::fs::write(dir.join("dist/index.html"), "hi").unwrap();
        #[cfg(windows)]
        std::fs::write(
            dir.join("swa.cmd"),
            "@echo off\r\necho https://nice-name-0123.azurestaticapps.net\r\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join("swa");
            std::fs::write(
                &p,
                "#!/bin/sh\necho 'https://nice-name-0123.azurestaticapps.net'\n",
            )
            .unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.extra_path = vec![dir.clone()];
        let out = AzureStaticProvider.deploy(&ctx).unwrap();
        assert_eq!(
            out.url.as_deref(),
            Some("https://nice-name-0123.azurestaticapps.net")
        );
        assert_eq!(out.status, DeployStatus::Ready);

        let out = AzureStaticProvider.deploy_preview(&ctx).unwrap();
        assert!(out.is_preview);
        let _ = std::fs::remove_dir_all(dir);
    }
}
