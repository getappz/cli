//! CLI/MCP-shared glue between appz's project detection and `appz-deploy`'s
//! providers: resolve the target, build a `DeployContext`, deploy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use appz_deploy::{
    DeployContext, DeployError, DeployOutput, DeployProvider, detect_providers,
    ensure_cli_available, get_provider, install_cli_with_mise, parse_kv, read_deploy_config,
};

/// What to do when the platform CLI is not installed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InstallPolicy {
    /// Fail with the install command (non-interactive runs, `--json`, MCP).
    Never,
    /// Ask first (interactive terminal).
    Ask,
    /// Install through mise without asking (`--yes`).
    Yes,
}

pub struct DeployRequest {
    pub target: Option<String>,
    pub prod: bool,
    pub env: Vec<String>,
    pub build_env: Vec<String>,
    pub dry_run: bool,
    pub json: bool,
    pub install: InstallPolicy,
}

/// The CLI was not found on `PATH` or in mise's bin dirs: install it through
/// mise if the policy (or the user, when asked) allows, else fail with the
/// install command. Returns mise's bin dirs, which then contain the CLI.
fn provision_cli(
    p: &dyn DeployProvider,
    policy: InstallPolicy,
    project_dir: &Path,
    missing: &str,
) -> Result<Vec<PathBuf>, String> {
    match policy {
        InstallPolicy::Never => {
            return Err(format!("{missing}\n(or re-run with --yes to install it)"));
        }
        InstallPolicy::Ask => {
            let prompt = format!(
                "{} is not installed. Install {} with mise?",
                p.cli_tool(),
                p.mise_tool()
            );
            if !cliclack::confirm(prompt)
                .initial_value(true)
                .interact()
                .unwrap_or(false)
            {
                return Err(missing.to_string());
            }
        }
        InstallPolicy::Yes => {}
    }
    eprintln!("installing {} with mise...", p.mise_tool());
    install_cli_with_mise(p, project_dir).map_err(|e| e.to_string())
}

/// Pick the target from provider config files in the project (`wrangler.*`,
/// `vercel.json`/`.vercel`, `netlify.toml`/`.netlify`) when neither `--target`
/// nor `deploy.default` is set. Exactly one match is used; several is
/// ambiguous, so the user must choose rather than appz guessing.
fn autodetect_target(project_dir: &Path) -> Result<String, String> {
    let mut found = detect_providers(project_dir);
    match found.len() {
        1 => Ok(found.remove(0).to_string()),
        0 => Err("no deploy target specified — pass --target <provider> or set \"deploy\": { \"default\": \"vercel\" } in appz.jsonc".to_string()),
        _ => Err(format!(
            "multiple deploy targets detected ({}) — pass --target <provider> or set \"deploy\": {{ \"default\": \"{}\" }} in appz.jsonc",
            found.join(", "),
            found[0]
        )),
    }
}

pub fn run(project_dir: &Path, req: DeployRequest) -> Result<DeployOutput, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match req.target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    // mise-installed CLIs (global config or this project's mise.toml) are
    // discovered via `mise bin-paths` and appended after the user's own PATH.
    let mut extra_path = Vec::new();
    if !req.dry_run {
        extra_path = appz_core::mise_bin_paths(project_dir);
        match ensure_cli_available(provider.as_ref(), &extra_path) {
            Ok(()) => {}
            Err(e @ DeployError::CliMissing { .. }) => {
                extra_path =
                    provision_cli(provider.as_ref(), req.install, project_dir, &e.to_string())?;
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    let toolchains = appz_core::detect_toolchains(project_dir).map_err(|e| e.to_string())?;
    let output_dir = toolchains
        .iter()
        .find_map(|tc| tc.output_directory)
        .unwrap_or("dist")
        .to_string();
    let framework = toolchains
        .iter()
        .flat_map(|tc| tc.frameworks.iter())
        .next()
        .map(|f| f.name.to_string());

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), output_dir);
    ctx.is_preview = !req.prod;
    ctx.dry_run = req.dry_run;
    ctx.json_output = req.json;
    ctx.framework = framework;
    ctx.deploy_config = config;
    ctx.extra_path = extra_path;
    ctx.run_env = req
        .env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();
    ctx.build_env = req
        .build_env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();

    if ctx.is_preview {
        provider.deploy_preview(&ctx).map_err(|e| e.to_string())
    } else {
        provider.deploy(&ctx).map_err(|e| e.to_string())
    }
}
