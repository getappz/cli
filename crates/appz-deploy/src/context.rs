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

    /// Run a provider CLI non-interactively, arg-vector (no shell), capturing
    /// output with a timeout. `run_env` is always injected on top of the
    /// inherited process environment (auth tokens flow through env, never as
    /// CLI flags — the upstream version passed tokens as `--token <value>`,
    /// visible in process listings and shell history).
    pub fn exec(&self, bin: &str, args: &[&str], timeout: Duration) -> Result<ExecOutput, String> {
        let env: Vec<(&str, &str)> = self
            .run_env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        exec_captured(bin, args, &self.project_dir, &env, timeout)
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
