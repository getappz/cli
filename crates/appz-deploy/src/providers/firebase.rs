//! Firebase Hosting deploy provider.
//!
//! ## Scope
//! Hosting only: `firebase deploy --only hosting` (and a Hosting channel for
//! previews). Firestore rules, Cloud Functions, Storage and the rest of
//! Firebase are separate products with their own deploy semantics and are not
//! covered here.
//!
//! ## Detection and project
//! `firebase.json` marks a Firebase project. The project id is resolved from,
//! in order: `deploy.targets.firebase.project` in `appz.jsonc`, then the
//! `.firebaserc` `projects.default` alias — the same file the CLI itself
//! writes on `firebase use`. That means an already-initialised project needs
//! no appz config at all.
//!
//! ## Authentication
//! `FIREBASE_TOKEN` (CI) or the CLI's own interactive login state
//! (`firebase login`). `firebase login:ci` is the command that prints a token
//! for CI; its `login` is interactive and writes credentials to the user's
//! config dir.
//!
//! ## Build model
//! `firebase deploy --only hosting` uploads the directory configured in
//! `firebase.json` (`hosting.public`). appz builds first and, when there is no
//! `firebase.json`, writes one pointing at appz's output directory.

use std::path::Path;
use std::time::Instant;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider};

pub struct FirebaseProvider;

/// Preview deploys go to a fixed Hosting channel so repeated previews of the
/// same project update one recognizable URL instead of creating a new one.
const PREVIEW_CHANNEL: &str = "preview";

impl FirebaseProvider {
    fn fail(&self, reason: String) -> DeployError {
        DeployError::DeployFailed {
            provider: self.name(),
            reason,
        }
    }

    fn missing_project(&self) -> DeployError {
        self.fail(
            "no Firebase project: pass --project <id>, set \
             deploy.targets.firebase.project in appz.jsonc, or run `firebase use <project>` \
             to write .firebaserc"
                .to_string(),
        )
    }

    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let Some(project) = resolve_project(ctx) else {
            return Err(self.missing_project());
        };
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let start = Instant::now();
        let args: Vec<&str> = if is_preview {
            vec![
                "hosting:channel:deploy",
                PREVIEW_CHANNEL,
                "--project",
                &project,
                "--non-interactive",
            ]
        } else {
            vec![
                "deploy",
                "--only",
                "hosting",
                "--project",
                &project,
                "--non-interactive",
            ]
        };
        let deploy = ctx
            .exec_live(self.cli_tool(), &args, DEPLOY_TIMEOUT)
            .map_err(|reason| self.fail(reason))?;
        if !deploy.success {
            return Err(self.fail(format!(
                "firebase {} failed:\n{}\n{}",
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

impl DeployProvider for FirebaseProvider {
    fn name(&self) -> &'static str {
        "Firebase Hosting"
    }
    fn slug(&self) -> &'static str {
        "firebase"
    }
    fn cli_tool(&self) -> &'static str {
        "firebase"
    }
    fn install_package(&self) -> &'static str {
        "firebase-tools"
    }
    fn auth_env_var(&self) -> &'static str {
        "FIREBASE_TOKEN"
    }
    /// No read-only probe that is stable across firebase-tools versions, so
    /// sign-in is only reported when the token variable is set; otherwise the
    /// CLI reports its own error.
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
        "firebase login (or firebase login:ci for a CI token)".to_string()
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
        if !project_dir.join("firebase.json").exists() {
            return Ok(None);
        }
        let project = firebaserc_project(project_dir);
        Ok(Some(DetectedConfig {
            config_file: "firebase.json".into(),
            is_linked: project.is_some(),
            project_name: project,
            team: None,
        }))
    }

    fn is_linked(&self, ctx: &DeployContext) -> bool {
        resolve_project(ctx).is_some()
    }

    fn link_args(&self, ctx: &DeployContext, interactive: bool) -> Option<Vec<String>> {
        if let Some(project) = resolve_project(ctx) {
            return Some(vec!["use".into(), project]);
        }
        interactive.then(|| vec!["use".into(), "--add".into()])
    }

    /// Overridden so a non-interactive link without a known project names the
    /// actual ways to provide one instead of the generic "not linked" error.
    fn link(&self, ctx: &DeployContext, interactive: bool) -> DeployResult<()> {
        if resolve_project(ctx).is_none() && !interactive {
            return Err(self.missing_project());
        }
        let Some(args) = self.link_args(ctx, interactive) else {
            return Err(self.missing_project());
        };
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        if interactive {
            return match ctx.exec_interactive(self.cli_tool(), &refs) {
                Ok(true) => Ok(()),
                Ok(false) => {
                    Err(self.fail(format!("firebase {} did not complete", args.join(" "))))
                }
                Err(reason) => Err(self.fail(reason)),
            };
        }
        let out = ctx
            .exec(self.cli_tool(), &refs, crate::provider::QUERY_TIMEOUT)
            .map_err(|reason| self.fail(reason))?;
        if out.success {
            Ok(())
        } else {
            Err(self.fail(format!(
                "firebase {} failed:\n{}\n{}",
                args.join(" "),
                out.stdout,
                out.stderr
            )))
        }
    }

    /// A minimal Hosting config over appz's build output, so a Firebase
    /// project that never ran `firebase init` still deploys the built site
    /// instead of whichever (possibly missing) `public` dir the CLI defaults to.
    fn setup_files(&self, ctx: &DeployContext) -> DeployResult<Vec<(String, String)>> {
        if ctx.project_dir.join("firebase.json").exists() {
            return Ok(Vec::new());
        }
        let config = serde_json::json!({
            "hosting": {
                "public": ctx.output_dir,
                "ignore": ["firebase.json", "**/.*", "**/node_modules/**"],
            }
        });
        Ok(vec![(
            "firebase.json".to_string(),
            serde_json::to_string_pretty(&config)? + "\n",
        )])
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }
}

/// The project id appz will deploy to: the configured target wins, then the
/// `.firebaserc` default alias written by `firebase use`.
fn resolve_project(ctx: &DeployContext) -> Option<String> {
    configured_project(ctx).or_else(|| firebaserc_project(&ctx.project_dir))
}

fn configured_project(ctx: &DeployContext) -> Option<String> {
    ctx.deploy_config
        .targets
        .get("firebase")?
        .get("project")?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn firebaserc_project(project_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join(".firebaserc")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .pointer("/projects/default")?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// The deployment URL from Firebase's own output. Firebase prints several
/// links (project console, Hosting URL, channel URL); only links on the
/// Hosting hosts count, and the last one wins because the final block is the
/// one the deploy just produced. Never fabricates `https://{project}.web.app`.
fn extract_deploy_url(stdout: &str) -> Option<String> {
    const HOSTS: [&str; 2] = [".web.app", ".firebaseapp.com"];
    stdout
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '<' | '>' | '(' | ')'))
        .filter_map(|token| token.strip_prefix("https://").map(|rest| (token, rest)))
        .rfind(|(_, rest)| {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            HOSTS.iter().any(|h| host.ends_with(h))
        })
        .map(|(token, _)| token.trim_end_matches(['.', ';']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "appz-deploy-firebase-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detection_reads_the_project_from_firebaserc() {
        let dir = temp("detect");
        std::fs::write(dir.join("firebase.json"), "{}").unwrap();
        std::fs::write(
            dir.join(".firebaserc"),
            r#"{"projects":{"default":"my-project"}}"#,
        )
        .unwrap();

        let config = FirebaseProvider
            .detect_config(&dir)
            .unwrap()
            .expect("firebase.json should be detected");
        assert_eq!(config.config_file, "firebase.json");
        assert!(config.is_linked);
        assert_eq!(config.project_name.as_deref(), Some("my-project"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn firebaserc_alone_is_not_a_project_without_firebase_json() {
        let dir = temp("rc-only");
        std::fs::write(
            dir.join(".firebaserc"),
            r#"{"projects":{"default":"my-project"}}"#,
        )
        .unwrap();
        assert!(FirebaseProvider.detect_config(&dir).unwrap().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn configured_project_wins_over_firebaserc() {
        let dir = temp("precedence");
        std::fs::write(
            dir.join(".firebaserc"),
            r#"{"projects":{"default":"from-rc"}}"#,
        )
        .unwrap();
        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.deploy_config.targets.insert(
            "firebase".into(),
            serde_json::json!({ "project": "from-config" }),
        );
        assert_eq!(resolve_project(&ctx).as_deref(), Some("from-config"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_hosting_config_over_the_build_output_only_when_none_exists() {
        let dir = temp("setup");
        let ctx = DeployContext::new(dir.clone(), "dist".into());

        let files = FirebaseProvider.setup_files(&ctx).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "firebase.json");
        let v: serde_json::Value = serde_json::from_str(&files[0].1).unwrap();
        assert_eq!(v["hosting"]["public"], "dist");

        std::fs::write(dir.join("firebase.json"), "{}").unwrap();
        assert!(FirebaseProvider.setup_files(&ctx).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_hosting_links_are_the_deployment_url() {
        let stdout = "Project Console: https://console.firebase.google.com/project/x/overview\n\
                      Hosting URL: https://x.web.app\n\
                      Channel URL: https://x--preview-abc.web.app\n";
        assert_eq!(
            extract_deploy_url(stdout),
            Some("https://x--preview-abc.web.app".to_string())
        );
        assert_eq!(
            extract_deploy_url("Nothing deployed\nhttps://console.firebase.google.com/x"),
            None
        );
        assert_eq!(extract_deploy_url("https://x.web.app.evil.com/"), None);
    }

    #[test]
    fn a_deploy_without_a_project_names_the_ways_to_set_one() {
        let dir = temp("no-project");
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        let err = FirebaseProvider.deploy(&ctx).unwrap_err().to_string();
        assert!(err.contains("--project"), "{err}");
        assert!(err.contains("firebase use"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deploys_the_configured_project_and_reads_the_url() {
        let dir = temp("run");
        #[cfg(windows)]
        std::fs::write(
            dir.join("firebase.cmd"),
            "@echo off\r\necho Hosting URL: https://my-project.web.app\r\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join("firebase");
            std::fs::write(
                &p,
                "#!/bin/sh\necho 'Hosting URL: https://my-project.web.app'\n",
            )
            .unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.extra_path = vec![dir.clone()];
        ctx.deploy_config.targets.insert(
            "firebase".into(),
            serde_json::json!({ "project": "my-project" }),
        );

        let out = FirebaseProvider.deploy(&ctx).unwrap();
        assert_eq!(out.url.as_deref(), Some("https://my-project.web.app"));
        assert_eq!(out.status, DeployStatus::Ready);

        let out = FirebaseProvider.deploy_preview(&ctx).unwrap();
        assert!(out.is_preview);
        assert_eq!(out.url.as_deref(), Some("https://my-project.web.app"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
