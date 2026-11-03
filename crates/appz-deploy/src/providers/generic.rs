//! Providers described by data (`registry/deploy.toml`) instead of a struct
//! each: appz runs the platform's own CLI with the steps the registry lists.
//!
//! Safety rules, all enforced here:
//! * Steps are argument lists run without a shell. `{{key}}` is replaced
//!   inside one argument; it can never add, split or reorder arguments.
//! * Values come only from `deploy.targets.<slug>` in appz.jsonc, the
//!   registry's own defaults and the built-in `output_dir`: never from the
//!   process environment (the old blueprints substituted any env var).
//! * A value that starts with `-` or contains control characters is refused,
//!   so config cannot smuggle in a flag.
//! * A platform without a `preview` step has no preview deploys; asking for
//!   one is an error, never a silent production deploy.
//! * A step that reads the build output refuses to run when that folder is
//!   missing or empty (`aws s3 sync --delete` would otherwise empty a bucket).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use serde::Deserialize;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{AuthState, DeployOutput, DeployStatus, DetectedConfig, PrerequisiteStatus};
use crate::provider::{DEPLOY_TIMEOUT, DeployProvider, QUERY_TIMEOUT};

const REGISTRY: &str = include_str!("../../registry/deploy.toml");

#[derive(Deserialize)]
struct Registry {
    provider: Vec<Spec>,
}

#[derive(Deserialize)]
struct Spec {
    slug: String,
    name: String,
    cli: String,
    #[serde(default)]
    mise: String,
    install_hint: String,
    #[serde(default)]
    token_env: String,
    #[serde(default)]
    login: Vec<String>,
    login_hint: String,
    #[serde(default)]
    whoami: Vec<String>,
    #[serde(default)]
    detect: Vec<String>,
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    config: BTreeMap<String, String>,
    #[serde(default)]
    preview: Vec<Vec<String>>,
    #[serde(default)]
    production: Vec<Vec<String>>,
}

/// A parsed spec plus the arg lists the `DeployProvider` trait needs as
/// `&'static` slices. Built once for the process.
struct Loaded {
    spec: Spec,
    login: &'static [&'static str],
    whoami: &'static [&'static str],
}

fn leak(items: &[String]) -> &'static [&'static str] {
    let leaked: Vec<&'static str> = items
        .iter()
        .map(|s| &*Box::leak(s.clone().into_boxed_str()))
        .collect();
    Box::leak(leaked.into_boxed_slice())
}

fn loaded() -> &'static [Loaded] {
    static LOADED: OnceLock<Vec<Loaded>> = OnceLock::new();
    LOADED.get_or_init(|| {
        let registry: Registry =
            toml::from_str(REGISTRY).expect("embedded registry/deploy.toml is valid");
        registry
            .provider
            .into_iter()
            .map(|spec| Loaded {
                login: leak(&spec.login),
                whoami: leak(&spec.whoami),
                spec,
            })
            .collect()
    })
}

/// Every registry-described provider.
pub fn providers() -> Vec<Box<dyn DeployProvider>> {
    loaded()
        .iter()
        .map(|l| Box::new(GenericProvider(l)) as Box<dyn DeployProvider>)
        .collect()
}

pub struct GenericProvider(&'static Loaded);

/// Values available to `{{key}}`: registry defaults, overridden by
/// `deploy.targets.<slug>` (string values only), plus `output_dir`.
fn values(spec: &Spec, ctx: &DeployContext) -> BTreeMap<String, String> {
    let mut map = spec.config.clone();
    if let Some(obj) = ctx
        .deploy_config
        .targets
        .get(&spec.slug)
        .and_then(|t| t.as_object())
    {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                map.insert(k.clone(), s.to_string());
            }
        }
    }
    map.insert("output_dir".into(), ctx.output_dir.clone());
    map
}

fn safe_value(key: &str, value: &str) -> Result<(), String> {
    if value.starts_with('-') {
        return Err(format!("'{key}' must not start with '-' (got '{value}')"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("'{key}' contains a control character"));
    }
    Ok(())
}

/// Replace every `{{key}}` in each argument. Unknown keys are an error.
fn render(
    steps: &[Vec<String>],
    values: &BTreeMap<String, String>,
) -> Result<Vec<Vec<String>>, String> {
    steps
        .iter()
        .map(|argv| {
            argv.iter()
                .map(|arg| {
                    let mut out = String::new();
                    let mut rest = arg.as_str();
                    while let Some(start) = rest.find("{{") {
                        let after = &rest[start + 2..];
                        let end = after
                            .find("}}")
                            .ok_or_else(|| format!("unclosed placeholder in '{arg}'"))?;
                        let key = after[..end].trim();
                        let value = values
                            .get(key)
                            .ok_or_else(|| format!("unknown placeholder '{{{{{key}}}}}'"))?;
                        safe_value(key, value)?;
                        out.push_str(&rest[..start]);
                        out.push_str(value);
                        rest = &after[end + 2..];
                    }
                    out.push_str(rest);
                    Ok(out)
                })
                .collect()
        })
        .collect()
}

fn uses_output_dir(steps: &[Vec<String>]) -> bool {
    steps.iter().flatten().any(|a| a.contains("{{output_dir}}"))
}

fn non_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut it| it.next().is_some())
}

impl GenericProvider {
    fn spec(&self) -> &'static Spec {
        &self.0.spec
    }

    fn fail(&self, reason: String) -> DeployError {
        DeployError::DeployFailed {
            provider: self.name(),
            reason,
        }
    }

    fn run(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let spec = self.spec();
        let steps = if is_preview {
            &spec.preview
        } else {
            &spec.production
        };
        if steps.is_empty() {
            return Err(self.fail(if is_preview {
                format!(
                    "{} has no preview deploys; pass --prod to deploy to production",
                    spec.name
                )
            } else {
                format!("{} has no production deploy step", spec.name)
            }));
        }

        let values = values(spec, ctx);
        let missing: Vec<String> = spec
            .required
            .iter()
            .filter(|k| values.get(*k).is_none_or(|v| v.trim().is_empty()))
            .map(|k| format!("deploy.targets.{}.{k}", spec.slug))
            .collect();
        if !missing.is_empty() {
            return Err(self.fail(format!(
                "missing config: set {} in appz.jsonc",
                missing.join(", ")
            )));
        }
        let argvs = render(steps, &values).map_err(|e| self.fail(e))?;

        if uses_output_dir(steps) && !non_empty_dir(&ctx.project_dir.join(&ctx.output_dir)) {
            return Err(self.fail(format!(
                "build output '{}' is missing or empty: build the project first (appz build)",
                ctx.output_dir
            )));
        }

        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(&spec.slug, is_preview));
        }

        let start = Instant::now();
        let mut stdout = String::new();
        for argv in &argvs {
            let Some((program, args)) = argv.split_first() else {
                continue;
            };
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = ctx
                .exec_live(program, &args, DEPLOY_TIMEOUT)
                .map_err(|e| self.fail(e))?;
            if !out.success {
                return Err(self.fail(format!(
                    "{} failed:\n{}\n{}",
                    argv.join(" "),
                    out.stdout,
                    out.stderr
                )));
            }
            stdout.push_str(&out.stdout);
            stdout.push('\n');
        }

        let url = crate::providers::vercel::extract_deploy_url(&stdout);
        Ok(DeployOutput {
            provider: spec.slug.clone(),
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

impl DeployProvider for GenericProvider {
    fn name(&self) -> &'static str {
        &self.0.spec.name
    }

    fn slug(&self) -> &'static str {
        &self.0.spec.slug
    }

    fn cli_tool(&self) -> &'static str {
        &self.0.spec.cli
    }

    fn install_package(&self) -> &'static str {
        &self.0.spec.mise
    }

    fn auth_env_var(&self) -> &'static str {
        &self.0.spec.token_env
    }

    fn mise_tool(&self) -> String {
        self.0.spec.mise.clone()
    }

    fn install_hint(&self) -> Option<String> {
        Some(self.0.spec.install_hint.clone())
    }

    fn whoami_args(&self) -> &'static [&'static str] {
        self.0.whoami
    }

    fn login_args(&self) -> &'static [&'static str] {
        self.0.login
    }

    fn login_hint(&self) -> String {
        self.0.spec.login_hint.clone()
    }

    fn auth_status(&self, ctx: &DeployContext) -> AuthState {
        let var = self.auth_env_var();
        if !var.is_empty()
            && (ctx.run_env.get(var).is_some_and(|v| !v.is_empty())
                || std::env::var(var).is_ok_and(|v| !v.is_empty()))
        {
            return AuthState::Authenticated;
        }
        // No read-only probe known for this CLI: let it report its own error.
        if self.0.whoami.is_empty() {
            return AuthState::Unknown;
        }
        match ctx.exec(self.cli_tool(), self.whoami_args(), QUERY_TIMEOUT) {
            Ok(out) if out.success => AuthState::Authenticated,
            Ok(_) => AuthState::NotAuthenticated,
            Err(_) => AuthState::Unknown,
        }
    }

    fn login(&self, ctx: &DeployContext) -> DeployResult<()> {
        if self.0.login.is_empty() {
            return Err(self.fail(format!("no interactive login: {}", self.login_hint())));
        }
        match ctx.exec_interactive(self.cli_tool(), self.login_args()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(self.fail(format!("`{}` did not complete", self.login_hint()))),
            Err(reason) => Err(self.fail(reason)),
        }
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        if command::shell::find_command_on_path(self.cli_tool()).is_none() {
            return Ok(PrerequisiteStatus::CliMissing {
                tool: self.cli_tool().into(),
                install_hint: self.0.spec.install_hint.clone(),
            });
        }
        Ok(PrerequisiteStatus::Ready)
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        Ok(self
            .0
            .spec
            .detect
            .iter()
            .find(|f| project_dir.join(f).exists())
            .map(|f| DetectedConfig {
                config_file: f.clone(),
                is_linked: false,
                project_name: None,
                team: None,
            }))
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.run(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.run(ctx, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn provider(slug: &str) -> GenericProvider {
        GenericProvider(
            loaded()
                .iter()
                .find(|l| l.spec.slug == slug)
                .unwrap_or_else(|| panic!("no registry provider {slug}")),
        )
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-generic-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn configured(dir: &Path, slug: &str, cfg: serde_json::Value) -> DeployContext {
        let mut ctx = DeployContext::new(dir.to_path_buf(), "dist".into());
        ctx.deploy_config.targets.insert(slug.into(), cfg);
        ctx
    }

    /// Every placeholder in the registry resolves, every required key is used,
    /// slugs are unique and never shadow a native provider.
    #[test]
    fn the_embedded_registry_is_consistent() {
        let native = ["vercel", "netlify", "cloudflare"];
        let mut slugs = BTreeSet::new();
        for l in loaded() {
            let s = &l.spec;
            assert!(slugs.insert(s.slug.clone()), "duplicate slug {}", s.slug);
            assert!(!native.contains(&s.slug.as_str()), "{} is native", s.slug);
            assert!(
                !s.production.is_empty(),
                "{} has no production step",
                s.slug
            );
            assert!(
                s.token_env.is_empty()
                    || s.token_env
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                "{} token_env",
                s.slug
            );
            let mut known: BTreeSet<&str> = s.config.keys().map(String::as_str).collect();
            known.insert("output_dir");
            let mut used = BTreeSet::new();
            for argv in s.preview.iter().chain(&s.production) {
                assert!(!argv.is_empty(), "{} has an empty step", s.slug);
                for arg in argv {
                    let mut rest = arg.as_str();
                    while let Some(i) = rest.find("{{") {
                        let after = &rest[i + 2..];
                        let end = after.find("}}").expect("closed placeholder");
                        used.insert(after[..end].trim());
                        rest = &after[end + 2..];
                    }
                }
            }
            for key in &used {
                assert!(
                    known.contains(key) || s.required.iter().any(|r| r == key),
                    "{}: placeholder {{{{{key}}}}} has no default and is not required",
                    s.slug
                );
            }
            for r in &s.required {
                assert!(
                    used.contains(r.as_str()),
                    "{}: required '{r}' unused",
                    s.slug
                );
            }
            // Secrets are never config keys.
            for key in used.iter().chain(known.iter()) {
                let k = key.to_lowercase();
                assert!(
                    !(k.contains("token") || k.contains("secret") || k.contains("password")),
                    "{}: '{key}' looks like a secret",
                    s.slug
                );
            }
        }
        assert!(slugs.len() >= 9);
    }

    #[test]
    fn a_platform_without_preview_refuses_a_preview_deploy() {
        let dir = temp("nopreview");
        let ctx = configured(&dir, "fly", serde_json::json!({ "app": "x" }));
        let err = provider("fly")
            .deploy_preview(&ctx)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("no preview deploys") && err.contains("--prod"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_required_config_names_the_key_to_set() {
        let dir = temp("required");
        let ctx = DeployContext::new(dir.clone(), "dist".into());
        let err = provider("fly").deploy(&ctx).unwrap_err().to_string();
        assert!(err.contains("deploy.targets.fly.app"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn config_values_cannot_look_like_flags() {
        let dir = temp("flags");
        let ctx = configured(&dir, "fly", serde_json::json!({ "app": "--config=evil" }));
        let err = provider("fly").deploy(&ctx).unwrap_err().to_string();
        assert!(err.contains("must not start with '-'"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_process_environment_is_never_a_value() {
        // SAFETY-free check: render only sees the map it is given.
        let steps = vec![vec!["x".to_string(), "{{HOME}}".to_string()]];
        let err = render(&steps, &BTreeMap::new()).unwrap_err();
        assert!(err.contains("unknown placeholder"), "{err}");
    }

    #[test]
    fn a_step_that_reads_the_build_output_refuses_when_it_is_missing_or_empty() {
        let dir = temp("guard");
        let ctx = configured(&dir, "aws-s3", serde_json::json!({ "bucket": "b" }));
        let missing = provider("aws-s3").deploy(&ctx).unwrap_err().to_string();
        assert!(missing.contains("missing or empty"), "{missing}");
        std::fs::create_dir(dir.join("dist")).unwrap();
        let empty = provider("aws-s3").deploy(&ctx).unwrap_err().to_string();
        assert!(empty.contains("missing or empty"), "{empty}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_dry_run_validates_config_but_runs_nothing() {
        let dir = temp("dry");
        let mut ctx = configured(&dir, "fly", serde_json::json!({ "app": "my-app" }));
        ctx.dry_run = true;
        let out = provider("fly").deploy(&ctx).expect("dry run is ok");
        assert_eq!(out.provider, "fly");
        assert!(out.url.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detection_uses_only_the_listed_marker_files() {
        let dir = temp("detect");
        assert!(provider("fly").detect_config(&dir).unwrap().is_none());
        std::fs::write(dir.join("fly.toml"), "").unwrap();
        assert!(provider("fly").detect_config(&dir).unwrap().is_some());
        // A generic file like deno.json must not make the project "deno-deploy".
        std::fs::write(dir.join("deno.json"), "{}").unwrap();
        assert!(
            provider("deno-deploy")
                .detect_config(&dir)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Runs a fake `flyctl` found only through `extra_path`, and checks the
    /// exact arguments it received and the URL read from its output.
    #[test]
    fn runs_the_steps_as_an_argument_list_and_reads_the_url() {
        let dir = temp("e2e");
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(windows)]
        std::fs::write(
            bin.join("flyctl.cmd"),
            "@echo off\r\necho %*> args.txt\r\necho Visit https://my-app.fly.dev\r\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = bin.join("flyctl");
            std::fs::write(
                &p,
                "#!/bin/sh\necho \"$@\" > args.txt\necho Visit https://my-app.fly.dev\n",
            )
            .unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut ctx = configured(&dir, "fly", serde_json::json!({ "app": "my-app" }));
        ctx.extra_path = vec![bin];
        let out = provider("fly").deploy(&ctx).expect("deploy runs");
        assert_eq!(out.url.as_deref(), Some("https://my-app.fly.dev"));
        assert_eq!(out.status, DeployStatus::Ready);
        let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
        assert_eq!(args.trim(), "deploy --app my-app");
        let _ = std::fs::remove_dir_all(dir);
    }
}
