use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::DeployResult;
use crate::exec::{ExecOutput, exec_captured};

/// Deploy-time settings read from `appz.jsonc`'s `deploy` key — reuses the
/// same JSONC-with-comments convention as command overrides
/// (`appz_core::strip_jsonc`) rather than inventing a second config format.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DeployConfig {
    #[serde(default)]
    pub default: Option<String>,
    /// Extra vault secret names (beyond the provider's token and link ids)
    /// to pass to the platform CLI.
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub targets: HashMap<String, serde_json::Value>,
}

impl DeployConfig {
    pub fn get_target_config<T: serde::de::DeserializeOwned>(
        &self,
        slug: &str,
    ) -> DeployResult<Option<T>> {
        match self.targets.get(slug) {
            Some(v) => Ok(Some(serde_json::from_value(v.clone())?)),
            None => Ok(None),
        }
    }
}

/// Read the `deploy` section of `appz.jsonc`, if present. Returns `None` if
/// the file doesn't exist or has no `deploy` key — never an error for the
/// common case of "not configured yet".
pub fn read_deploy_config(project_dir: &Path) -> DeployResult<Option<DeployConfig>> {
    let path = project_dir.join("appz.jsonc");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let cleaned = appz_core::strip_jsonc(&content);
    let root: serde_json::Value = serde_json::from_str(&cleaned)?;
    match root.get("deploy") {
        Some(v) => Ok(Some(serde_json::from_value(v.clone())?)),
        None => Ok(None),
    }
}

pub struct DeployContext {
    pub project_dir: PathBuf,
    pub output_dir: String,
    pub is_preview: bool,
    /// Build-time env vars (`-b KEY=VALUE`).
    pub build_env: HashMap<String, String>,
    /// Runtime env vars (`-e KEY=VALUE`), injected into the provider CLI's
    /// process env. Upstream declared `supports_env_vars()` but never read
    /// `env_vars` anywhere — here it's actually wired into `exec()`.
    pub run_env: HashMap<String, String>,
    pub is_ci: bool,
    pub dry_run: bool,
    pub json_output: bool,
    pub framework: Option<String>,
    pub deploy_config: DeployConfig,
    /// Extra directories searched (and appended to `PATH`) when running the
    /// provider CLI, e.g. mise's tool bin dirs (`mise bin-paths`).
    pub extra_path: Vec<PathBuf>,
}

impl DeployContext {
    pub fn new(project_dir: PathBuf, output_dir: String) -> Self {
        Self {
            project_dir,
            output_dir,
            is_preview: false,
            build_env: HashMap::new(),
            run_env: HashMap::new(),
            is_ci: std::env::var("CI").is_ok(),
            dry_run: false,
            json_output: false,
            framework: None,
            deploy_config: DeployConfig::default(),
            extra_path: Vec::new(),
        }
    }

    pub fn output_path(&self) -> PathBuf {
        self.project_dir.join(&self.output_dir)
    }

    pub fn environment_name(&self) -> &'static str {
        if self.is_preview {
            "preview"
        } else {
            "production"
        }
    }

    /// Name to give a new platform project: `project` from this target's
    /// config in `appz.jsonc`, else `package.json`'s name, else the folder
    /// name; lowercased to `[a-z0-9-]`.
    pub fn project_name(&self, slug: &str) -> String {
        let configured = self
            .deploy_config
            .targets
            .get(slug)
            .and_then(|t| t.get("project"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let from_package = || {
            let text = std::fs::read_to_string(self.project_dir.join("package.json")).ok()?;
            let v: serde_json::Value = serde_json::from_str(&text).ok()?;
            v.get("name")?.as_str().map(String::from)
        };
        let dir_name = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let mut raw = configured
            .or_else(from_package)
            .unwrap_or_else(|| dir_name(&self.project_dir));
        // "site", "app", ... say nothing; the parent folder usually does.
        const GENERIC: [&str; 6] = ["site", "app", "web", "website", "frontend", "client"];
        if GENERIC.contains(&raw.to_lowercase().as_str())
            && let Some(parent) = self.project_dir.parent()
            && !dir_name(parent).is_empty()
        {
            raw = dir_name(parent);
        }
        let slugged: String = raw
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let trimmed = slugged.trim_matches('-');
        if trimmed.is_empty() {
            "site".to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Full path of `bin`, looked up in the same `PATH` the child will get: a
    /// `PATH` the caller put in `run_env`, else the process `PATH`, then
    /// `extra_path`. Falls back to the bare name.
    fn resolve(&self, bin: &str) -> String {
        let base = self.run_env.get("PATH").map(std::ffi::OsString::from);
        command::shell::find_command_with_paths(bin, base.as_deref(), &self.extra_path)
            .map_or_else(|| bin.to_string(), |p| p.to_string_lossy().into_owned())
    }

    /// Run a provider CLI non-interactively, arg-vector (no shell), capturing
    /// output with a timeout. `run_env` is always injected on top of the
    /// inherited process environment (auth tokens flow through env, never as
    /// CLI flags — the upstream version passed tokens as `--token <value>`,
    /// visible in process listings and shell history).
    ///
    /// `bin` is resolved against `PATH` then `extra_path`, so a CLI that mise
    /// installed off-PATH (or an npm `.cmd` shim on Windows, which a
    /// shell-less spawn cannot find by bare name) still runs; `extra_path` is
    /// also appended to the child's `PATH` (the user's own tools keep priority).
    pub fn exec(&self, bin: &str, args: &[&str], timeout: Duration) -> Result<ExecOutput, String> {
        let base_path = self.run_env.get("PATH").map(std::ffi::OsString::from);
        let program = self.resolve(bin);

        let mut env: Vec<(String, String)> = self
            .run_env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !self.extra_path.is_empty() {
            let current = base_path
                .clone()
                .or_else(|| std::env::var_os("PATH"))
                .unwrap_or_default();
            let all = std::env::split_paths(&current).chain(self.extra_path.iter().cloned());
            if let Ok(joined) = std::env::join_paths(all) {
                env.push(("PATH".into(), joined.to_string_lossy().into_owned()));
            }
        }
        let env_refs: Vec<(&str, &str)> =
            env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        exec_captured(&program, args, &self.project_dir, &env_refs, timeout)
    }

    /// Run a provider CLI with the terminal inherited (login flows). Same
    /// binary resolution and `PATH` handling as [`Self::exec`]. `Ok(true)` when
    /// it exited successfully.
    pub fn exec_interactive(&self, bin: &str, args: &[&str]) -> Result<bool, String> {
        let program = self.resolve(bin);
        let mut cmd = command::Command::new(&program);
        cmd.args(args.iter().copied())
            .cwd(&self.project_dir)
            .without_shell()
            .append_paths(&self.extra_path);
        for (k, v) in &self.run_env {
            cmd.env(k, v);
        }
        cmd.exec_interactive()
            .map(|status| status.success())
            .map_err(|e| e.to_string())
    }
}

/// Parse a `KEY=VALUE` string (used for `-b`/`-e` repeatable flags).
pub fn parse_kv(s: &str) -> Option<(String, String)> {
    let (k, v) = s.split_once('=')?;
    if k.is_empty() {
        return None;
    }
    Some((k.to_string(), v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CLI that lives only in `extra_path` (like a mise-installed tool, or an
    /// npm `.cmd` shim on Windows) must run by its bare name.
    #[test]
    fn exec_runs_a_shim_found_only_via_extra_path() {
        let dir = std::env::temp_dir().join(format!("appz-deploy-shim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        #[cfg(windows)]
        std::fs::write(
            dir.join("appz-test-shim.cmd"),
            "@echo off\r\necho shim-ok\r\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join("appz-test-shim");
            std::fs::write(&p, "#!/bin/sh\necho shim-ok\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.extra_path = vec![dir.clone()];
        let out = ctx
            .exec("appz-test-shim", &[], Duration::from_secs(10))
            .expect("shim should resolve via extra_path");
        assert!(out.success, "{out:?}");
        assert!(out.stdout.contains("shim-ok"), "{out:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `PATH` the caller sets in `run_env` is the lookup base too, not just
    /// the child's environment.
    #[test]
    fn exec_resolves_against_a_path_set_in_run_env() {
        let dir = std::env::temp_dir().join(format!("appz-deploy-runenv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(windows)]
        std::fs::write(dir.join("appz-test-shim2.cmd"), "@echo off\r\necho ok2\r\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join("appz-test-shim2");
            std::fs::write(&p, "#!/bin/sh\necho ok2\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        let base = std::env::var_os("PATH").unwrap_or_default();
        let joined =
            std::env::join_paths(std::iter::once(dir.clone()).chain(std::env::split_paths(&base)))
                .unwrap();
        ctx.run_env
            .insert("PATH".into(), joined.to_string_lossy().into_owned());
        ctx.extra_path = vec![std::env::temp_dir()];
        let out = ctx
            .exec("appz-test-shim2", &[], Duration::from_secs(10))
            .expect("should resolve via run_env PATH");
        assert!(out.stdout.contains("ok2"), "{out:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exec_fails_when_the_cli_is_in_neither_path() {
        let ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        assert!(
            ctx.exec("appz-no-such-cli-xyz", &[], Duration::from_secs(5))
                .is_err()
        );
    }
}
