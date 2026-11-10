//! CLI/MCP-shared glue between appz's project detection and `appz-deploy`'s
//! providers: resolve the target, build a `DeployContext`, deploy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use appz_deploy::{
    AuthState, DeployConfig, DeployContext, DeployError, DeployOutput, DeployProvider,
    detect_providers, ensure_cli_available, get_provider, install_cli_with_mise, parse_kv,
    read_deploy_config,
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
const LINK_VARS: [&str; 5] = [
    "VERCEL_ORG_ID",
    "VERCEL_PROJECT_ID",
    "NETLIFY_SITE_ID",
    "APPZ_ORG_ID",
    "APPZ_PROJECT_ID",
];

#[derive(Clone)]
pub struct DeployRequest {
    pub target: Option<String>,
    pub prod: bool,
    /// Platform project name (skips the prompt when linking).
    pub project: Option<String>,
    /// Deploy even when nothing changed since the last deploy of the target.
    pub force: bool,
    pub env: Vec<String>,
    pub build_env: Vec<String>,
    pub dry_run: bool,
    pub json: bool,
    /// Installing a missing platform CLI (works without a terminal).
    pub install: Consent,
    /// Running the CLI's login. Only ever `Ask`/`Yes` with a real terminal:
    /// login waits on a browser and would hang CI.
    pub login: Consent,
    /// Creating/linking a platform project on the user's account.
    pub link: Consent,
}

/// The project is not linked to a platform project: link it if the policy
/// (or the user, when asked) allows, else fail with the way out. In a
/// terminal with no `--project`, the platform CLI runs its own link flow
/// (scope, project name, ...) so appz follows what that CLI does.
fn ensure_linked(
    p: &dyn DeployProvider,
    ctx: &DeployContext,
    consent: Consent,
    named: bool,
) -> Result<(), String> {
    if p.is_linked(ctx) {
        return Ok(());
    }
    let name = ctx.project_name(p.slug());
    let interactive = match consent {
        Consent::Never => {
            return Err(format!(
                "{}: no project linked. Re-run with --yes to create/link '{name}' (or --project <name>), or set the platform's project id variables",
                p.name()
            ));
        }
        Consent::Ask => !named,
        Consent::Yes => false,
    };
    if interactive {
        eprintln!("linking with `{} link`...", p.cli_tool());
    } else {
        eprintln!("linking {} project '{name}'...", p.name());
    }
    p.link(ctx, interactive).map_err(|e| e.to_string())?;
    if p.is_linked(ctx) {
        Ok(())
    } else {
        Err(format!("{}: project was not linked", p.name()))
    }
}

/// Apply the install `policy` before installing `tool` (providing `cli`)
/// with mise: fail with `missing` when installing is not allowed, ask when
/// interactive, and announce the install otherwise.
pub(crate) fn confirm_mise_install(
    policy: Consent,
    cli: &str,
    tool: &str,
    missing: &str,
) -> Result<(), String> {
    match policy {
        Consent::Never => {
            return Err(format!("{missing}\n(or re-run with --yes to install it)"));
        }
        Consent::Ask => {
            let prompt = format!("{cli} is not installed. Install {tool} with mise?");
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
    eprintln!("installing {tool} with mise...");
    Ok(())
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
    if p.mise_tool().is_empty() {
        // Nothing appz can install: `missing` already carries the install hint.
        return Err(missing.to_string());
    }
    confirm_mise_install(policy, p.cli_tool(), &p.mise_tool(), missing)?;
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

/// Everything besides the source files that changes what gets deployed: the
/// target, the project name, and the `-e` / `-b` values. Only hashed into the
/// fingerprint, never stored.
fn fingerprint_extra(slug: &str, req: &DeployRequest, config: &DeployConfig) -> String {
    let mut env = req.env.clone();
    env.sort();
    let mut build = req.build_env.clone();
    build.sort();
    let project = req
        .project
        .clone()
        .or_else(|| {
            config
                .targets
                .get(slug)?
                .get("project")?
                .as_str()
                .map(String::from)
        })
        .unwrap_or_default();
    // The whole target config (site, output settings, ...) also changes the deploy.
    let target = config
        .targets
        .get(slug)
        .map(|v| v.to_string())
        .unwrap_or_default();
    format!(
        "{slug}
{project}
{}
{}
{target}",
        env.join("\u{1f}"),
        build.join("\u{1f}")
    )
}

/// Run a deploy lifecycle hook as an arg vector (no shell), in the project
/// directory, with the terminal inherited so its output shows live. `extra_path`
/// (mise tool bin dirs) is on `PATH` so a hook's own tools resolve. An empty
/// argv is a no-op.
fn run_hook(ctx: &DeployContext, name: &str, argv: &[String]) -> Result<(), String> {
    if argv.is_empty() {
        return Ok(());
    }
    let (bin, args) = argv.split_first().expect("non-empty argv");
    eprintln!("running {name} hook: {}", argv.join(" "));
    let mut cmd = command::Command::new(ctx.resolve(bin));
    cmd.args(args.iter().map(String::as_str))
        .cwd(&ctx.project_dir)
        .without_shell()
        .append_paths(&ctx.extra_path);
    let status = cmd
        .exec_interactive()
        .map_err(|e| format!("{name} hook: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} hook '{bin}' failed"))
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

    // Nothing changed since the last deploy of this target: do not install,
    // build and upload it all again.
    // The check is only an optimization: if the project cannot be fingerprinted
    // the deploy goes ahead.
    // Render deploys its connected remote, whose revision isn't in this hash.
    let cache_local_files = slug != "render";
    if cache_local_files
        && !req.force
        && let Ok(fp) =
            crate::deploy_state::fingerprint(project_dir, &fingerprint_extra(&slug, &req, &config))
        && let Some(last) = crate::deploy_state::unchanged(project_dir, &slug, req.prod, &fp)
    {
        let mut out = DeployOutput::dry_run(provider.slug(), !req.prod);
        out.url = Some(last.url);
        out.status = appz_deploy::DeployStatus::Ready;
        out.unchanged = true;
        return Ok(out);
    }

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
    let frameworks: Vec<String> = toolchains
        .iter()
        .flat_map(|tc| tc.frameworks.iter())
        .map(|f| f.name.to_string())
        .collect();
    let framework = frameworks.first().cloned();

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
    if let Some(project) = &req.project {
        ctx.deploy_config
            .targets
            .entry(slug.clone())
            .or_insert_with(|| serde_json::json!({}))["project"] =
            serde_json::Value::String(project.clone());
    }
    ctx.extra_path = extra_path;
    // Runtime env for the provider CLI. Precedence (lowest → highest):
    // `deploy.environments.<env>` < vault secrets < real process env < `-e`.
    // Config and vault values never override the user's real environment.
    let mut run_env: HashMap<String, String> = ctx
        .deploy_config
        .env_for(ctx.environment_name())
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();
    for (k, v) in vault.vars {
        if std::env::var_os(&k).is_none() {
            run_env.insert(k, v);
        }
    }
    run_env.extend(req.env.iter().filter_map(|s| parse_kv(s)));
    ctx.run_env = run_env;
    ctx.build_env = req
        .build_env
        .iter()
        .filter_map(|s| parse_kv(s))
        .collect::<HashMap<_, _>>();

    if !req.dry_run {
        ensure_authenticated(provider.as_ref(), &ctx, req.login)?;
        ensure_linked(provider.as_ref(), &ctx, req.link, req.project.is_some())?;
    }

    // Platform config the project lacks (e.g. wrangler.jsonc, netlify.toml):
    // appz writes it; a dry run only reports it.
    let setup = provider.setup_files(&ctx).map_err(|e| e.to_string())?;
    let setup_names: Vec<String> = setup.iter().map(|(n, _)| n.clone()).collect();
    if !req.dry_run {
        for (name, content) in &setup {
            std::fs::write(ctx.project_dir.join(name), content)
                .map_err(|e| format!("cannot write {name}: {e}"))?;
        }
    }

    // Install + build through mise's cached tasks (skipped when unchanged);
    // providers then deploy the built output instead of building from scratch.
    if !req.dry_run {
        if provider.needs_prebuild(&frameworks) {
            ctx.prebuilt = crate::build_stage::run(&ctx)?;
        } else if !req.json {
            eprintln!(
                "note: {} builds this project itself; skipping appz's build stage",
                provider.name()
            );
        }
    }

    if !req.dry_run {
        run_hook(
            &ctx,
            "before_deploy",
            &ctx.deploy_config.hooks.before_deploy,
        )?;
    }

    let mut output = if ctx.is_preview {
        provider.deploy_preview(&ctx)
    } else {
        provider.deploy(&ctx)
    }
    .map_err(|e| e.to_string())?;

    output.setup_files = setup_names;
    if cache_local_files
        && !req.dry_run
        && let Some(url) = &output.url
        && let Ok(fp) = crate::deploy_state::fingerprint(
            project_dir,
            &fingerprint_extra(&slug, &req, &ctx.deploy_config),
        )
    {
        // Fingerprint after the deploy: the run itself may have written
        // config (wrangler.jsonc, a .gitignore line) that is part of the project now.
        crate::deploy_state::record(project_dir, &slug, req.prod, &fp, url);
    }
    if !req.dry_run {
        run_hook(&ctx, "after_deploy", &ctx.deploy_config.hooks.after_deploy).map_err(|e| {
            format!(
                "deployed ({}) but {e}",
                output.url.as_deref().unwrap_or("url unknown")
            )
        })?;
    }
    output.link_needed = req.dry_run && !provider.is_linked(&ctx);

    // A dry run is a readiness report: probe sign-in when the CLI is present
    // (read-only; never installs or logs in).
    if req.dry_run {
        if ensure_cli_available(provider.as_ref(), &ctx.extra_path).is_ok() {
            output.auth = Some(provider.auth_status(&ctx));
        } else {
            output.cli_missing = true;
        }
    }
    Ok(output)
}

/// Deploy every configured target (`deploy.targets` keys in appz.jsonc),
/// collecting one `DeployOutput` per target. Deploys in sorted slug order and
/// fails with a combined error when any target fails.
pub fn run_all(project_dir: &Path, req: DeployRequest) -> Result<Vec<DeployOutput>, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let mut slugs: Vec<String> = config.targets.keys().cloned().collect();
    slugs.sort();
    if slugs.is_empty() {
        return Err(
            "no deploy targets configured — add targets under \"deploy\": { \"targets\": { ... } } in appz.jsonc"
                .to_string(),
        );
    }

    let mut outputs = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for slug in slugs {
        let mut one = req.clone();
        one.target = Some(slug.clone());
        match run(project_dir, one) {
            Ok(output) => outputs.push(output),
            Err(e) => errors.push(format!("{slug}: {e}")),
        }
    }

    if errors.is_empty() {
        Ok(outputs)
    } else {
        Err(format!(
            "{} of {} target(s) failed:\n{}",
            errors.len(),
            outputs.len() + errors.len(),
            errors.join("\n")
        ) + &outputs
            .iter()
            .map(|o| {
                format!(
                    "\ndeployed: {} ({})",
                    o.provider,
                    o.url.as_deref().unwrap_or("url unknown")
                )
            })
            .collect::<String>())
    }
}

pub fn rollback(
    project_dir: &Path,
    target: Option<String>,
    version_id: &str,
) -> Result<DeployOutput, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    provider
        .rollback(&ctx, version_id)
        .map_err(|e| e.to_string())
}

pub fn link(
    project_dir: &Path,
    target: Option<String>,
    project: Option<String>,
) -> Result<String, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    if let Some(p) = project {
        ctx.deploy_config
            .targets
            .entry(slug.clone())
            .or_insert_with(|| serde_json::json!({}))["project"] = serde_json::Value::String(p);
    }
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    provider.link(&ctx, true).map_err(|e| e.to_string())?;
    Ok(provider.name().to_string())
}

pub fn alias(project_dir: &Path, target: Option<String>, domain: &str) -> Result<String, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    provider.alias(&ctx, domain).map_err(|e| e.to_string())
}

pub fn list(
    project_dir: &Path,
    target: Option<String>,
) -> Result<Vec<appz_deploy::DeploymentInfo>, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    provider.list_deployments(&ctx).map_err(|e| e.to_string())
}

pub fn inspect(
    project_dir: &Path,
    target: Option<String>,
    id_or_url: &str,
) -> Result<appz_deploy::DeploymentInspect, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();

    provider
        .inspect_deployment(&ctx, id_or_url)
        .map_err(|e| e.to_string())
}

/// Resolve the provider and a vault-backed context for a domain operation.
fn domain_ctx(
    project_dir: &Path,
    target: Option<String>,
) -> Result<(Box<dyn DeployProvider>, DeployContext), String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match target.or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir).unwrap_or_else(|_| "appzdev".to_string()),
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;

    let allowed: Vec<String> = std::iter::once(provider.auth_env_var().to_string())
        .chain(LINK_VARS.map(String::from))
        .chain(config.env.iter().cloned())
        .collect();
    let vault = crate::secrets::deploy_env(project_dir, &allowed)?;

    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".to_string());
    ctx.deploy_config = config;
    ctx.run_env = vault
        .vars
        .into_iter()
        .filter(|(k, _)| std::env::var_os(k).is_none())
        .collect();
    Ok((provider, ctx))
}

pub fn add_domain(
    project_dir: &Path,
    target: Option<String>,
    domain: &str,
) -> Result<appz_deploy::ProjectDomain, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider.add_domain(&ctx, domain).map_err(|e| e.to_string())
}

pub fn list_domains(
    project_dir: &Path,
    target: Option<String>,
) -> Result<Vec<appz_deploy::ProjectDomain>, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider.list_domains(&ctx).map_err(|e| e.to_string())
}

pub fn verify_domain(
    project_dir: &Path,
    target: Option<String>,
    domain: &str,
) -> Result<appz_deploy::DomainVerifyResult, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .verify_domain(&ctx, domain)
        .map_err(|e| e.to_string())
}

pub fn remove_domain(
    project_dir: &Path,
    target: Option<String>,
    domain: &str,
) -> Result<bool, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .remove_domain(&ctx, domain)
        .map_err(|e| e.to_string())
}

pub fn list_projects(
    project_dir: &Path,
    target: Option<String>,
) -> Result<Vec<appz_deploy::ProjectInfo>, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider.list_projects(&ctx).map_err(|e| e.to_string())
}

pub fn add_project(
    project_dir: &Path,
    target: Option<String>,
    name: &str,
) -> Result<appz_deploy::ProjectInfo, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider.add_project(&ctx, name).map_err(|e| e.to_string())
}

pub fn get_project(
    project_dir: &Path,
    target: Option<String>,
    id_or_slug: &str,
) -> Result<appz_deploy::ProjectInfo, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .get_project(&ctx, id_or_slug)
        .map_err(|e| e.to_string())
}

pub fn rename_project(
    project_dir: &Path,
    target: Option<String>,
    id_or_slug: &str,
    new_name: &str,
) -> Result<appz_deploy::ProjectInfo, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .rename_project(&ctx, id_or_slug, new_name)
        .map_err(|e| e.to_string())
}

pub fn remove_project(
    project_dir: &Path,
    target: Option<String>,
    id_or_slug: &str,
) -> Result<bool, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .remove_project(&ctx, id_or_slug)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(env: &[&str], build: &[&str], project: Option<&str>) -> DeployRequest {
        DeployRequest {
            target: None,
            project: project.map(String::from),
            force: false,
            prod: false,
            env: env.iter().map(|s| s.to_string()).collect(),
            build_env: build.iter().map(|s| s.to_string()).collect(),
            dry_run: false,
            json: false,
            install: Consent::Never,
            login: Consent::Never,
            link: Consent::Never,
        }
    }

    #[test]
    fn render_does_not_skip_remote_deploys_based_on_local_files() {
        let dir = std::env::temp_dir().join(format!("appz-render-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("appz.jsonc"),
            r#"{"deploy":{"default":"render","targets":{"render":{"service":"srv-one"}}}}"#,
        )
        .unwrap();
        let mut request = req(&["RENDER_API_KEY=test-key"], &[], None);
        request.prod = true;
        request.dry_run = true;
        request.json = true;
        let config = read_deploy_config(&dir).unwrap().unwrap();
        let fp =
            crate::deploy_state::fingerprint(&dir, &fingerprint_extra("render", &request, &config))
                .unwrap();
        crate::deploy_state::record(&dir, "render", true, &fp, "https://old.example");
        let output = run(&dir, request).unwrap();
        assert!(
            !output.unchanged,
            "local files cannot identify Render's remote source revision"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_fingerprint_input_tracks_what_changes_the_deployment() {
        let cfg = DeployConfig::default();
        let base = fingerprint_extra("vercel", &req(&["A=1", "B=2"], &["X=1"], None), &cfg);
        // Order of the flags does not matter.
        assert_eq!(
            base,
            fingerprint_extra("vercel", &req(&["B=2", "A=1"], &["X=1"], None), &cfg)
        );
        // A changed -e, -b, project or target does.
        assert_ne!(
            base,
            fingerprint_extra("vercel", &req(&["A=9", "B=2"], &["X=1"], None), &cfg)
        );
        assert_ne!(
            base,
            fingerprint_extra("vercel", &req(&["A=1", "B=2"], &["X=2"], None), &cfg)
        );
        assert_ne!(
            base,
            fingerprint_extra(
                "vercel",
                &req(&["A=1", "B=2"], &["X=1"], Some("other")),
                &cfg
            )
        );
        assert_ne!(
            base,
            fingerprint_extra("netlify", &req(&["A=1", "B=2"], &["X=1"], None), &cfg)
        );
    }
}
