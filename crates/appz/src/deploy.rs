//! CLI/MCP-shared glue between appz's project detection and `appz-deploy`'s
//! providers: resolve the target, build a `DeployContext`, deploy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use appz_deploy::{
    AuthState, DeployContext, DeployError, DeployOutput, DeployProvider, detect_providers,
    ensure_cli_available, get_provider, install_cli_with_mise, parse_kv, read_deploy_config,
};

/// Whether appz may take a side-effecting step (install a CLI, run a login)
/// on the user's behalf.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// Fail with the command to run instead (non-interactive runs, `--json`, MCP).
    Never,
    /// Ask first (interactive terminal).
    Ask,
    /// Go ahead without asking (`--yes`).
    Yes,
}

/// Link ids the platform CLIs read from the environment instead of `link`.
const LINK_VARS: [&str; 3] = ["VERCEL_ORG_ID", "VERCEL_PROJECT_ID", "NETLIFY_SITE_ID"];

pub struct DeployRequest {
    pub target: Option<String>,
    pub prod: bool,
    pub env: Vec<String>,
    pub build_env: Vec<String>,
    pub dry_run: bool,
    pub json: bool,
    /// Installing a missing platform CLI (works without a terminal).
    pub install: Consent,
    /// Running the CLI's login. Only ever `Ask`/`Yes` with a real terminal:
    /// login waits on a browser and would hang CI.
    pub login: Consent,
}

/// The CLI was not found on `PATH` or in mise's bin dirs: install it through
/// mise if the policy (or the user, when asked) allows, else fail with the
/// install command. Returns mise's bin dirs, which then contain the CLI.
fn provision_cli(
    p: &dyn DeployProvider,
    policy: Consent,
    project_dir: &Path,
    missing: &str,
) -> Result<Vec<PathBuf>, String> {
    match policy {
        Consent::Never => {
            return Err(format!("{missing}\n(or re-run with --yes to install it)"));
        }
        Consent::Ask => {
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
        Consent::Yes => {}
    }
    eprintln!("installing {} with mise...", p.mise_tool());
    install_cli_with_mise(p, project_dir).map_err(|e| e.to_string())
}

/// Make sure the CLI is signed in. Signed in or `Unknown` passes (the CLI
/// reports its own error); not signed in logs in when `consent` allows, else
/// fails with the token variable and the login command.
fn ensure_authenticated(
    p: &dyn DeployProvider,
    ctx: &DeployContext,
    consent: Consent,
) -> Result<(), String> {
    if p.auth_status(ctx) != AuthState::NotAuthenticated {
        return Ok(());
    }
    let hint = format!(
        "{}: not authenticated. Set {} or run: {}",
        p.name(),
        p.auth_env_var(),
        p.login_hint()
    );
    match consent {
        Consent::Never => return Err(hint),
        Consent::Ask => {
            let prompt = format!(
                "Not signed in to {}. Run `{}` now?",
                p.name(),
                p.login_hint()
            );
            if !cliclack::confirm(prompt)
                .initial_value(true)
                .interact()
                .unwrap_or(false)
            {
                return Err(hint);
            }
        }
        Consent::Yes => {}
    }
    p.login(ctx).map_err(|e| e.to_string())?;
    if p.auth_status(ctx) == AuthState::NotAuthenticated {
        return Err(format!(
            "{hint}
(login did not complete)"
        ));
    }
    Ok(())
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
    let mut extra_path = appz_core::mise_bin_paths(project_dir);
    if !req.dry_run {
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

    // Vault values for this provider only (its token, link ids, `deploy.env`).
    // Precedence: vault < real environment < `-e`.
    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;
    if vault.locked && std::env::var_os(provider.auth_env_var()).is_none() {
        eprintln!("note: vault is locked; run `appz env unlock` to use stored credentials");
    }

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), output_dir);
    ctx.is_preview = !req.prod;
    ctx.dry_run = req.dry_run;
    ctx.json_output = req.json;
    ctx.framework = framework;
    ctx.deploy_config = config;
    ctx.extra_path = extra_path;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();
    ctx.run_env
        .extend(req.env.iter().filter_map(|s| parse_kv(s)));
    ctx.build_env = req
        .build_env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();

    if !req.dry_run {
        ensure_authenticated(provider.as_ref(), &ctx, req.login)?;
    }

    let mut output = if ctx.is_preview {
        provider.deploy_preview(&ctx)
    } else {
        provider.deploy(&ctx)
    }
    .map_err(|e| e.to_string())?;

    // A dry run is a readiness report: probe sign-in when the CLI is present
    // (read-only; never installs or logs in).
    if req.dry_run && ensure_cli_available(provider.as_ref(), &ctx.extra_path).is_ok() {
        output.auth = Some(provider.auth_status(&ctx));
    }
    Ok(output)
}
