//! Vercel deploy provider.
//!
//! ## Detection
//! - `.vercel/project.json` — linked project state (projectId/orgId)
//! - `vercel.json` — project configuration
//!
//! ## Authentication
//! - `VERCEL_TOKEN` environment variable (CI/CD) or `vercel login` (local)
//!
//! ## Build model
//! Vercel builds through its OWN pipeline, not appz's `mise run build`: this
//! provider runs the documented prebuilt-deploy recipe (`vercel build` then
//! `vercel deploy --prebuilt`) rather than handing Vercel an arbitrary static
//! folder, which it only accepts in the Build Output API format `vercel
//! build` itself produces. "Drive the platform's own CLI" means Vercel keeps
//! doing what it already does well.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

pub struct VercelProvider;

/// Output dirs/files that mean the build produced something to run, not just
/// files to serve.
const SERVER_MARKERS: [&str; 4] = ["server", "functions", "_worker.js", "_worker"];

/// Turn a static build (`index.html` at the output root, nothing to run) into
/// the Build Output API layout `vercel deploy --prebuilt` takes:
/// `.vercel/output/{config.json, static/}`. `Ok(false)` when the output is not
/// a plain static site (the caller falls back to `vercel pull` + `vercel build`).
fn assemble_static_output(ctx: &DeployContext) -> Result<bool, String> {
    let src = ctx.output_path();
    if !src.join("index.html").is_file() || SERVER_MARKERS.iter().any(|m| src.join(m).exists()) {
        return Ok(false);
    }
    let out = ctx.project_dir.join(".vercel").join("output");
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("cannot clear .vercel/output: {e}"))?;
    }
    copy_dir(&src, &out.join("static"))
        .map_err(|e| format!("cannot assemble .vercel/output: {e}"))?;
    std::fs::write(out.join("config.json"), r#"{"version":3}"#)
        .map_err(|e| format!("cannot write .vercel/output/config.json: {e}"))?;
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

impl VercelProvider {
    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        // A static site appz already built: hand Vercel that output directly
        // (no `vercel pull` / `vercel build`, which start from scratch).
        let static_site = ctx.prebuilt
            && assemble_static_output(ctx).map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;

        if !static_site {
            // Pull the linked project's settings; `vercel build` needs them locally.
            let env_name = if is_preview { "preview" } else { "production" };
            let pull = ctx
                .exec_live(
                    "vercel",
                    &["pull", "--yes", "--environment", env_name],
                    DEPLOY_TIMEOUT,
                )
                .map_err(|reason| DeployError::DeployFailed {
                    provider: self.name(),
                    reason,
                })?;
            if !pull.success {
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!(
                        "vercel pull failed:
{}
{}",
                        pull.stdout, pull.stderr
                    ),
                });
            }

            let mut build_args = vec!["build"];
            if !is_preview {
                build_args.push("--prod");
            }
            let build = ctx
                .exec_live("vercel", &build_args, DEPLOY_TIMEOUT)
                .map_err(|reason| DeployError::DeployFailed {
                    provider: self.name(),
                    reason,
                })?;
            if !build.success {
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: format!("vercel build failed:\n{}\n{}", build.stdout, build.stderr),
                });
            }
        }

        let mut deploy_args = vec!["deploy", "--prebuilt", "--yes"];
        if !is_preview {
            deploy_args.push("--prod");
        }
        let deploy = ctx
            .exec_live("vercel", &deploy_args, DEPLOY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !deploy.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!(
                    "vercel deploy failed:\n{}\n{}",
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

/// Frameworks Vercel builds into functions with its own `vercel build`: a
/// prebuild by appz would only be thrown away.
const SERVER_FRAMEWORKS: [&str; 3] = ["Next.js", "Nuxt", "Remix"];

impl DeployProvider for VercelProvider {
    fn needs_prebuild(&self, frameworks: &[String]) -> bool {
        !frameworks
            .iter()
            .any(|f| SERVER_FRAMEWORKS.contains(&f.as_str()))
    }
    fn name(&self) -> &'static str {
        "Vercel"
    }
    fn slug(&self) -> &'static str {
        "vercel"
    }
    fn cli_tool(&self) -> &'static str {
        "vercel"
    }
    fn install_package(&self) -> &'static str {
        "vercel"
    }
    fn auth_env_var(&self) -> &'static str {
        "VERCEL_TOKEN"
    }
    fn whoami_args(&self) -> &'static [&'static str] {
        &["whoami"]
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path("vercel").is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: "vercel".into(),
                install_hint: "npm i -g vercel".into(),
            });
        }
        if std::env::var("VERCEL_TOKEN").is_err() {
            return Ok(PrerequisiteStatus::AuthMissing {
                env_var: "VERCEL_TOKEN".into(),
                login_hint: "vercel login".into(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn is_linked(&self, ctx: &DeployContext) -> bool {
        let has = |k: &str| ctx.run_env.contains_key(k) || std::env::var_os(k).is_some();
        // The id variables pin the project outright.
        if has("VERCEL_ORG_ID") && has("VERCEL_PROJECT_ID") {
            return true;
        }
        let Ok(text) = std::fs::read_to_string(ctx.project_dir.join(".vercel/project.json")) else {
            return false;
        };
        // A project the user named explicitly (--project / appz.jsonc) must be
        // the linked one: pull, build and deploy all use the link, so a link
        // to another project would publish there (even with --prod).
        let wanted = ctx
            .deploy_config
            .targets
            .get(self.slug())
            .and_then(|t| t.get("project"))
            .and_then(|v| v.as_str());
        match wanted {
            None => true,
            Some(name) => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("projectName")?.as_str().map(str::to_lowercase))
                .is_some_and(|linked| linked == name.to_lowercase()),
        }
    }

    fn link_args(&self, ctx: &DeployContext, interactive: bool) -> Option<Vec<String>> {
        if interactive {
            return Some(vec!["link".into()]);
        }
        Some(vec![
            "link".into(),
            "--yes".into(),
            "--project".into(),
            ctx.project_name(self.slug()),
        ])
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let state_path = project_dir.join(".vercel/project.json");
        if state_path.exists() {
            let content = std::fs::read_to_string(&state_path)?;
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(DetectedConfig {
                    config_file: ".vercel/project.json".into(),
                    is_linked: true,
                    project_name: state
                        .get("projectId")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    team: state
                        .get("orgId")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                }));
            }
        }
        if project_dir.join("vercel.json").exists() {
            return Ok(Some(DetectedConfig {
                config_file: "vercel.json".into(),
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

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        if !ctx.project_dir.join(".vercel/project.json").exists() {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: "project not linked to Vercel — run `vercel link` first".into(),
            });
        }
        let out = ctx
            .exec("vercel", &["ls", "--yes"], QUERY_TIMEOUT)
            .map_err(|reason| DeployError::DeployFailed {
                provider: self.name(),
                reason,
            })?;
        if !out.success {
            return Err(DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("vercel ls failed:\n{}\n{}", out.stdout, out.stderr),
            });
        }
        // Best-effort text scan, not a schema we assert with false confidence
        // (`vercel ls`'s plain-text table format isn't guaranteed stable
        // across CLI versions; verify against a live run before relying on
        // this for anything beyond a human-readable listing).
        let deployments = out
            .stdout
            .lines()
            .filter_map(|line| {
                let url = line
                    .split_whitespace()
                    .find(|tok| tok.starts_with("https://"))?;
                Some(DeploymentInfo {
                    id: url.to_string(),
                    url: Some(url.to_string()),
                    status: DeployStatus::Unknown,
                    created_at: None,
                    is_current: false,
                })
            })
            .collect();
        Ok(deployments)
    }
}

/// Extract the real deployment URL from `vercel deploy`'s stdout. Vercel's
/// CLI contract is designed for scripting (`URL=$(vercel deploy ...)`) — its
/// stdout is the deployment URL. Scans for the LAST bare `https://` line
/// rather than the first (the upstream bug this fixes: some flows print an
/// inspect/status URL before the deploy URL) and returns `None` — never a
/// guessed URL like `https://{dirname}.vercel.app` — if nothing matches.
fn extract_deploy_url(stdout: &str) -> Option<String> {
    // Find `https://...` tokens anywhere (bare line, or inside the JSON the
    // CLI prints when it has no terminal). Dashboard/inspect links on
    // vercel.com are not the deployment, so they only count as a last resort.
    let urls: Vec<&str> = stdout
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '<' | '>' | '(' | ')'))
        .filter(|t| t.starts_with("https://"))
        .collect();
    let is_dashboard = |u: &&str| u.starts_with("https://vercel.com/");
    urls.iter()
        .rfind(|u| !is_dashboard(u))
        .or_else(|| urls.last())
        .map(|u| u.trim_end_matches(['.', ';']).to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn server_frameworks_are_left_to_vercels_own_build() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Next.js also detects React: any server framework wins.
        assert!(!VercelProvider.needs_prebuild(&names(&["React", "Next.js"])));
        assert!(VercelProvider.needs_prebuild(&names(&["Astro"])));
        assert!(VercelProvider.needs_prebuild(&[]));
    }

    #[test]
    fn a_static_build_is_assembled_into_the_build_output_layout() {
        let dir = std::env::temp_dir().join(format!("appz-vercel-static-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("dist/assets")).unwrap();
        std::fs::write(dir.join("dist/index.html"), "hi").unwrap();
        std::fs::write(dir.join("dist/assets/a.js"), "x").unwrap();
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        assert!(assemble_static_output(&ctx).unwrap());
        assert!(dir.join(".vercel/output/static/assets/a.js").is_file());
        assert!(dir.join(".vercel/output/config.json").is_file());
        // A server dir means it is not a plain static site.
        std::fs::create_dir_all(dir.join("dist/server")).unwrap();
        assert!(!assemble_static_output(&ctx).unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    use super::*;

    #[test]
    fn takes_the_last_url_not_the_first() {
        // Regression test for the upstream bug this fixes: some flows print
        // an inspect/status URL before the real deploy URL.
        let stdout = "https://vercel.com/acme/app/inspect/abc123\nhttps://app-abc123.vercel.app\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://app-abc123.vercel.app".to_string())
        );
    }

    #[test]
    fn finds_the_url_inside_json_output() {
        let stdout = r#"{"status":"ok","deployment":{"url":"https://site-abc.vercel.app","inspectorUrl":"https://vercel.com/acme/site/abc"}}"#;
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://site-abc.vercel.app".to_string())
        );
    }

    #[test]
    fn returns_none_instead_of_fabricating_a_url() {
        let stdout = "Deploying...\nDone.\n";
        assert_eq!(extract_deploy_url(stdout), None);
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;

    fn linked_dir(name: &str) -> (std::path::PathBuf, DeployContext) {
        let dir =
            std::env::temp_dir().join(format!("appz-vercel-link-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".vercel")).unwrap();
        std::fs::write(
            dir.join(".vercel/project.json"),
            r#"{"projectId":"prj_1","orgId":"team_1","projectName":"Alpha"}"#,
        )
        .unwrap();
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        (dir, ctx)
    }

    #[test]
    fn an_existing_link_counts_when_no_project_was_named() {
        let (dir, ctx) = linked_dir("any");
        assert!(VercelProvider.is_linked(&ctx));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_named_project_must_be_the_linked_one() {
        let (dir, mut ctx) = linked_dir("named");
        let set = |ctx: &mut DeployContext, p: &str| {
            ctx.deploy_config
                .targets
                .insert("vercel".into(), serde_json::json!({ "project": p }));
        };
        set(&mut ctx, "alpha");
        assert!(
            VercelProvider.is_linked(&ctx),
            "same project, case-insensitive"
        );
        set(&mut ctx, "beta");
        assert!(
            !VercelProvider.is_linked(&ctx),
            "a link to another project must not satisfy --project beta"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
