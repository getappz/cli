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
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct NetlifyProvider;

impl NetlifyProvider {
    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        // appz's cached build already produced the output: upload it as is.
        let mut args = vec!["deploy", "--no-build"];
        if ctx.prebuilt {
            args.extend(["--dir", ctx.output_dir.as_str()]);
        }
        if !is_preview {
            args.push("--prod");
        }
        let deploy = ctx
            .exec_live("netlify", &args, DEPLOY_TIMEOUT)
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
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        })
    }
}

impl DeployProvider for NetlifyProvider {
    fn parallel_upload_safe(&self, _ctx: &DeployContext) -> bool {
        true
    }

    fn prepare_deploy(&self, ctx: &DeployContext) -> DeployResult<()> {
        if ctx.dry_run || ctx.prebuilt {
            return Ok(());
        }
        let output = ctx
            .exec_live("netlify", &["build"], DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !output.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "netlify build failed:\n{}\n{}",
                    output.stdout, output.stderr
                ),
            });
        }
        Ok(())
    }

    fn deploy_prepared(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, ctx.is_preview)
    }
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
    fn whoami_args(&self) -> &'static [&'static str] {
        &["status", "--json"]
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

    fn is_linked(&self, ctx: &DeployContext) -> bool {
        // The site id variable pins the project outright.
        if ctx.run_env.contains_key("NETLIFY_SITE_ID")
            || std::env::var_os("NETLIFY_SITE_ID").is_some()
        {
            return true;
        }
        // Otherwise the link lives in `.netlify/state.json`. Unlike Vercel's
        // `project.json` this stores only the `siteId`, not the site name, so
        // there is no local name to compare against a `--project` choice; the
        // linked site is simply whatever this directory is linked to.
        std::fs::read_to_string(ctx.project_dir.join(".netlify/state.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|state| state.get("siteId")?.as_str().map(str::to_string))
            .is_some_and(|id| !id.is_empty())
    }

    fn link_args(&self, ctx: &DeployContext, interactive: bool) -> Option<Vec<String>> {
        if interactive {
            // `netlify link` asks existing-vs-new and the site name itself.
            return Some(vec!["link".into()]);
        }
        // `netlify link --name` only links to an EXISTING site, so it would
        // fail on first deploy. `sites:create --name` creates the site and
        // links the current directory to it.
        Some(vec![
            "sites:create".into(),
            "--name".into(),
            ctx.project_name(self.slug()),
        ])
    }

    /// `netlify.toml` with the build command and publish directory, so the
    /// build output is what gets published.
    fn setup_files(&self, ctx: &DeployContext) -> DeployResult<Vec<(String, String)>> {
        if ctx.project_dir.join("netlify.toml").exists() {
            return Ok(Vec::new());
        }
        let has_build_script = std::fs::read_to_string(ctx.project_dir.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .is_some_and(|v| v.pointer("/scripts/build").is_some());
        let mut toml = String::from(
            "[build]
",
        );
        if has_build_script {
            toml.push_str(
                "  command = \"npm run build\"
",
            );
        }
        toml.push_str(&format!(
            "  publish = \"{}\"
",
            ctx.output_dir
        ));
        Ok(vec![("netlify.toml".to_string(), toml)])
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.prepare_deploy(ctx)?;
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.prepare_deploy(ctx)?;
        self.deploy_internal(ctx, true)
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let data = serde_json::json!({ "site_id": site_id(ctx)? });
        let response = api(ctx, "listSiteDeploys", &data, QUERY_TIMEOUT)?;
        let deployments: Vec<NetlifyDeployment> = serde_json::from_value(response)?;
        Ok(deployments
            .into_iter()
            .map(|d| DeploymentInfo {
                id: d.id,
                version_id: None,
                url: d.deploy_ssl_url.or(d.ssl_url).or(d.url),
                status: netlify_status(&d.state),
                created_at: d.created_at,
                is_current: false,
            })
            .collect())
    }

    fn rollback(&self, ctx: &DeployContext, deployment_id: &str) -> DeployResult<DeployOutput> {
        crate::provider::validate_deployment_id(self.name(), deployment_id)?;
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), false));
        }
        let start = Instant::now();
        let data = serde_json::json!({ "site_id": site_id(ctx)?, "deploy_id": deployment_id });
        let d: NetlifyDeployment =
            serde_json::from_value(api(ctx, "restoreSiteDeploy", &data, DEPLOY_TIMEOUT)?)?;
        let mut result = DeployOutput::dry_run(self.slug(), false);
        result.url = d.ssl_url.or(d.url);
        result.status = netlify_status(&d.state);
        result.created_at = d.created_at;
        result.duration_ms = Some(start.elapsed().as_millis() as u64);
        Ok(result)
    }
}

#[derive(serde::Deserialize)]
struct NetlifyDeployment {
    id: String,
    state: String,
    #[serde(default)]
    deploy_ssl_url: Option<String>,
    #[serde(default)]
    ssl_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    created_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn netlify_status(state: &str) -> DeployStatus {
    match state {
        "ready" => DeployStatus::Ready,
        "new" | "enqueued" | "building" | "uploading" | "uploaded" | "processing" | "prepared" => {
            DeployStatus::Building
        }
        "error" | "rejected" => DeployStatus::Error,
        _ => DeployStatus::Unknown,
    }
}

fn site_id(ctx: &DeployContext) -> DeployResult<String> {
    let env = ctx
        .run_env
        .get("NETLIFY_SITE_ID")
        .cloned()
        .or_else(|| std::env::var("NETLIFY_SITE_ID").ok())
        .filter(|v| !v.is_empty());
    let id = match env {
        Some(id) => id,
        None => {
            let text = std::fs::read_to_string(ctx.project_dir.join(".netlify/state.json"))
                .map_err(|_| DeployError::DeployFailed {
                    provider: "Netlify",
                    reason: "set NETLIFY_SITE_ID or run netlify link first".into(),
                })?;
            let state: serde_json::Value = serde_json::from_str(&text)?;
            state
                .get("siteId")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| DeployError::DeployFailed {
                    provider: "Netlify",
                    reason: "no siteId in .netlify/state.json: set NETLIFY_SITE_ID or run netlify link first".into(),
                })?
                .to_string()
        }
    };
    crate::provider::validate_deployment_id("Netlify", &id)?;
    Ok(id)
}

fn api(
    ctx: &DeployContext,
    method: &str,
    data: &serde_json::Value,
    timeout: std::time::Duration,
) -> DeployResult<serde_json::Value> {
    let data = data.to_string();
    let out = ctx
        .exec("netlify", &["api", method, "--data", &data], timeout)
        .map_err(|reason| DeployError::DeployFailed {
            provider: "Netlify",
            reason,
        })?;
    if !out.success {
        return Err(DeployError::DeployFailed {
            provider: "Netlify",
            reason: format!(
                "netlify api {method} failed:\n{}\n{}",
                out.stdout, out.stderr
            ),
        });
    }
    Ok(serde_json::from_str(&out.stdout)?)
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
    fn prepared_upload_does_not_run_another_build() {
        let dir = std::env::temp_dir().join(format!("appz-netlify-prepare-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("commands.log");
        let _ = std::fs::remove_file(&log);
        #[cfg(windows)]
        std::fs::write(
            dir.join("netlify.cmd"),
            "@echo off\r\necho %*>>commands.log\r\necho https://test.netlify.app\r\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join("netlify");
            std::fs::write(
                &path,
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> commands.log\necho https://test.netlify.app\n",
            )
            .unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.json_output = true;
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(dir.clone()).chain(std::env::split_paths(&inherited)),
        )
        .unwrap();
        ctx.run_env
            .insert("PATH".into(), path.to_string_lossy().into_owned());
        NetlifyProvider.prepare_deploy(&ctx).unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), "build");
        NetlifyProvider.deploy_prepared(&ctx).unwrap();
        ctx.prebuilt = true;
        NetlifyProvider.prepare_deploy(&ctx).unwrap();
        NetlifyProvider.deploy_prepared(&ctx).unwrap();
        let commands = std::fs::read_to_string(log).unwrap();
        let lines: Vec<_> = commands.lines().map(str::trim).collect();
        assert_eq!(
            lines,
            [
                "build",
                "deploy --no-build --prod",
                "deploy --no-build --dir dist --prod"
            ]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

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

#[cfg(test)]
mod link_tests {
    use super::*;

    fn ctx_with_state(tag: &str, contents: Option<&str>) -> (std::path::PathBuf, DeployContext) {
        let dir =
            std::env::temp_dir().join(format!("appz-netlify-link-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".netlify")).unwrap();
        if let Some(c) = contents {
            std::fs::write(dir.join(".netlify/state.json"), c).unwrap();
        }
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        (dir, ctx)
    }

    #[test]
    fn a_state_file_with_a_site_id_means_linked() {
        let (dir, ctx) = ctx_with_state("state", Some(r#"{"siteId":"abc-123"}"#));
        assert!(NetlifyProvider.is_linked(&ctx));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_state_file_and_no_env_means_not_linked() {
        let (dir, ctx) = ctx_with_state("none", None);
        assert!(!NetlifyProvider.is_linked(&ctx));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn site_id_in_the_environment_means_linked_without_a_state_file() {
        let (dir, mut ctx) = ctx_with_state("env", None);
        ctx.run_env
            .insert("NETLIFY_SITE_ID".into(), "abc-123".into());
        assert!(NetlifyProvider.is_linked(&ctx));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_interactive_link_creates_the_named_site() {
        let dir =
            std::env::temp_dir().join(format!("appz-netlify-link-args-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.deploy_config.targets.insert(
            "netlify".into(),
            serde_json::json!({ "project": "My Site" }),
        );
        assert_eq!(
            NetlifyProvider.link_args(&ctx, false),
            Some(vec![
                "sites:create".to_string(),
                "--name".to_string(),
                "my-site".to_string()
            ])
        );
        // Interactive mode lets the CLI ask existing-vs-new itself.
        assert_eq!(
            NetlifyProvider.link_args(&ctx, true),
            Some(vec!["link".to_string()])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod setup_tests {
    use super::*;

    #[test]
    fn writes_netlify_toml_only_when_none_exists() {
        let dir = std::env::temp_dir().join(format!("appz-nl-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"build":"astro build"}}"#,
        )
        .unwrap();
        let ctx = DeployContext::new(dir.clone(), "dist".into());

        let files = NetlifyProvider.setup_files(&ctx).unwrap();
        assert_eq!(files[0].0, "netlify.toml");
        assert!(files[0].1.contains("command = \"npm run build\""));
        assert!(files[0].1.contains("publish = \"dist\""));

        std::fs::write(dir.join("netlify.toml"), "").unwrap();
        assert!(NetlifyProvider.setup_files(&ctx).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
