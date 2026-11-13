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
        crate::step(&format!("linking with `{} link`...", p.cli_tool()));
    } else {
        crate::step(&format!("linking {} project '{name}'...", p.name()));
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
    crate::step(&format!("installing {tool} with mise..."));
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
/// directory with the deployment environment. Output goes to stderr (captured
/// in JSON mode) so hooks cannot corrupt the deployment result. An empty argv
/// is a no-op.
fn run_hook(ctx: &DeployContext, name: &str, argv: &[String]) -> Result<(), String> {
    if argv.is_empty() {
        return Ok(());
    }
    let (bin, args) = argv.split_first().expect("non-empty argv");
    if !ctx.json_output {
        crate::step(&format!("running {name} hook: {}", argv.join(" ")));
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // exec_live resolves `bin` against PATH then extra_path (mise-installed tools).
    let output = ctx
        .exec_live(bin, &args, appz_deploy::DEPLOY_TIMEOUT)
        .map_err(|e| format!("{name} hook: {e}"))?;
    if output.success {
        Ok(())
    } else {
        Err(format!("{name} hook '{bin}' failed"))
    }
}

struct PreparedTarget {
    ctx: DeployContext,
    provider: Box<dyn DeployProvider>,
    req: DeployRequest,
    setup_names: Vec<String>,
    cached: Option<DeployOutput>,
}

pub fn run(project_dir: &Path, req: DeployRequest) -> Result<DeployOutput, String> {
    let target = prepare_target(project_dir, req)?;
    let output = upload_target(&target)?;
    match finish_target(&target, &output) {
        Some(error) => Err(format!(
            "deployed ({}) but {error}",
            output.url.as_deref().unwrap_or("url unknown")
        )),
        None => Ok(output),
    }
}

fn prepare_target(project_dir: &Path, req: DeployRequest) -> Result<PreparedTarget, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match req.target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    let provider = get_provider(&slug).map_err(|e| e.to_string())?;
    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".into());

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
        return Ok(PreparedTarget {
            ctx,
            provider,
            req,
            setup_names: Vec::new(),
            cached: Some(out),
        });
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
        crate::info("note: vault is locked; run `appz env unlock` to use stored credentials");
    }

    ctx.output_dir = output_dir;
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
            crate::info(&format!(
                "note: {} builds this project itself; skipping appz's build stage",
                provider.name()
            ));
        }
    }

    if !req.dry_run {
        run_hook(
            &ctx,
            "before_deploy",
            &ctx.deploy_config.hooks.before_deploy,
        )?;
    }

    if !req.dry_run {
        provider.prepare_deploy(&ctx).map_err(|e| e.to_string())?;
    }
    Ok(PreparedTarget {
        ctx,
        provider,
        req,
        setup_names,
        cached: None,
    })
}

fn upload_target(target: &PreparedTarget) -> Result<DeployOutput, String> {
    if let Some(output) = &target.cached {
        return Ok(output.clone());
    }
    let ctx = &target.ctx;
    let provider = &target.provider;
    let mut output = provider.deploy_prepared(ctx).map_err(|e| e.to_string())?;
    output.setup_files = target.setup_names.clone();
    output.link_needed = ctx.dry_run && !provider.is_linked(ctx);
    if ctx.dry_run {
        if ensure_cli_available(provider.as_ref(), &ctx.extra_path).is_ok() {
            output.auth = Some(provider.auth_status(ctx));
        } else {
            output.cli_missing = true;
        }
    }
    Ok(output)
}

fn finish_target(target: &PreparedTarget, output: &DeployOutput) -> Option<String> {
    let ctx = &target.ctx;
    if ctx.dry_run || target.cached.is_some() {
        return None;
    }
    let slug = target.provider.slug();
    if let Err(error) = run_hook(ctx, "after_deploy", &ctx.deploy_config.hooks.after_deploy) {
        // A forced retry may have an older matching success in the cache.
        crate::deploy_state::forget(&ctx.project_dir, slug, target.req.prod);
        return Some(error);
    }
    // Render deploys its connected remote, whose revision isn't in the fingerprint.
    // Fingerprint after the deploy: the run itself may have written config
    // (wrangler.jsonc, a .gitignore line) that is part of the project now.
    if slug != "render"
        && let Some(url) = &output.url
        && let Ok(fp) = crate::deploy_state::fingerprint(
            &ctx.project_dir,
            &fingerprint_extra(slug, &target.req, &ctx.deploy_config),
        )
    {
        crate::deploy_state::record(&ctx.project_dir, slug, target.req.prod, &fp, url);
    }
    None
}

/// One target's outcome from `deploy --all`. `output` is present whenever the
/// deploy went out; `error` is a failure (then `output` is absent) or a failed
/// `after_deploy` hook (then `output` is kept).
#[derive(Debug, serde::Serialize)]
pub struct TargetResult {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<DeployOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Deploy every configured target (`deploy.targets` keys in appz.jsonc) and
/// return one result per target, in sorted slug order, so a failure does not
/// hide the targets that went out. Targets deploy in parallel unless a prompt
/// (install / link / login consent = Ask) could be needed: prompts from
/// concurrent deploys would interleave on one terminal, so those run in order.
pub fn run_all(project_dir: &Path, req: DeployRequest) -> Result<Vec<TargetResult>, String> {
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
    let interactive = [&req.install, &req.link, &req.login]
        .iter()
        .any(|c| matches!(c, Consent::Ask));
    Ok(run_targets(slugs, !interactive, |slug| {
        let mut one = req.clone();
        one.target = Some(slug.to_string());
        prepare_target(project_dir, one)
    }))
}

fn run_targets(
    slugs: Vec<String>,
    parallel: bool,
    mut prepare: impl FnMut(&str) -> Result<PreparedTarget, String>,
) -> Vec<TargetResult> {
    // Finish every shared-directory build and before hook before any upload.
    let prepared: Vec<_> = slugs.iter().map(|slug| prepare(slug)).collect();
    let upload = |target: &Result<PreparedTarget, String>| match target {
        Ok(target) => upload_target(target),
        Err(error) => Err(error.clone()),
    };
    let outputs: Vec<_> = if parallel && slugs.len() > 1 {
        // Unknown CLI steps may build locally. Finish them before upload-only CLIs.
        let mut outputs: Vec<_> = prepared
            .iter()
            .map(|target| match target {
                Ok(target)
                    if !target.ctx.dry_run
                        && target.cached.is_none()
                        && !target.provider.parallel_upload_safe(&target.ctx) =>
                {
                    Some(upload_target(target))
                }
                _ => None,
            })
            .collect();
        std::thread::scope(|scope| {
            let handles: Vec<_> = prepared
                .iter()
                .enumerate()
                .filter(|(i, _)| outputs[*i].is_none())
                .map(|(i, target)| (i, scope.spawn(move || upload(target))))
                .collect();
            for (i, handle) in handles {
                outputs[i] = Some(
                    handle
                        .join()
                        .unwrap_or_else(|_| Err("deploy thread panicked".into())),
                );
            }
        });
        outputs
            .into_iter()
            .map(|output| output.expect("every target ran"))
            .collect()
    } else {
        prepared.iter().map(upload).collect()
    };
    // After hooks can mutate build output too: wait for ALL uploads to finish.
    slugs
        .into_iter()
        .zip(prepared)
        .zip(outputs)
        .map(|((slug, target), output)| match output {
            Ok(output) => {
                let error =
                    finish_target(target.as_ref().expect("successful preparation"), &output);
                TargetResult {
                    target: slug,
                    output: Some(output),
                    error,
                }
            }
            Err(error) => TargetResult {
                target: slug,
                output: None,
                error: Some(error),
            },
        })
        .collect()
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct UploadProbe {
        exclusive: bool,
        prepared: Arc<AtomicUsize>,
        started: Arc<AtomicUsize>,
    }

    impl DeployProvider for UploadProbe {
        fn parallel_upload_safe(&self, _: &DeployContext) -> bool {
            !self.exclusive
        }
        fn name(&self) -> &'static str {
            "test"
        }
        fn slug(&self) -> &'static str {
            "test"
        }
        fn cli_tool(&self) -> &'static str {
            "test"
        }
        fn install_package(&self) -> &'static str {
            "test"
        }
        fn auth_env_var(&self) -> &'static str {
            "APPZ_TEST_TOKEN"
        }
        fn whoami_args(&self) -> &'static [&'static str] {
            &[]
        }
        fn check_prerequisites(
            &self,
        ) -> appz_deploy::DeployResult<appz_deploy::PrerequisiteStatus> {
            unreachable!()
        }
        fn detect_config(
            &self,
            _: &Path,
        ) -> appz_deploy::DeployResult<Option<appz_deploy::DetectedConfig>> {
            unreachable!()
        }
        fn deploy(&self, ctx: &DeployContext) -> appz_deploy::DeployResult<DeployOutput> {
            assert_eq!(
                self.prepared.load(Ordering::SeqCst),
                3,
                "upload started during preparation"
            );
            if self.exclusive {
                std::fs::write(ctx.project_dir.join("a"), "prepared").unwrap();
                std::fs::write(ctx.project_dir.join("b"), "prepared").unwrap();
                std::fs::write(ctx.project_dir.join("exclusive-finished"), "done").unwrap();
                return Ok(DeployOutput::dry_run("test", false));
            }
            assert!(ctx.project_dir.join("exclusive-finished").exists());
            self.started.fetch_add(1, Ordering::SeqCst);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while self.started.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert_eq!(
                self.started.load(Ordering::SeqCst),
                2,
                "uploads did not overlap"
            );
            assert_eq!(
                std::fs::read_to_string(ctx.output_path()).unwrap(),
                "prepared"
            );
            std::fs::write(
                ctx.project_dir.join(format!("uploaded-{}", ctx.output_dir)),
                "done",
            )
            .unwrap();
            Ok(DeployOutput::dry_run("test", false))
        }
        fn deploy_preview(&self, ctx: &DeployContext) -> appz_deploy::DeployResult<DeployOutput> {
            self.deploy(ctx)
        }
    }

    #[test]
    fn after_hook_waits_for_all_uploads_child() {
        let Ok(root) = std::env::var("APPZ_BATCH_TEST_ROOT") else {
            return;
        };
        let root = Path::new(&root);
        assert!(root.join("uploaded-a").exists());
        assert!(root.join("uploaded-b").exists());
        std::fs::write(root.join("a"), "after hook changed output").unwrap();
        std::fs::write(root.join("b"), "after hook changed output").unwrap();
    }

    #[test]
    fn batch_prepares_before_parallel_uploads_and_runs_after_hooks_last() {
        let dir = tempfile::tempdir().unwrap();
        let prepared = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let results = run_targets(
            vec!["a".into(), "serial".into(), "b".into()],
            true,
            |slug| {
                // Model a build that clears and rewrites the shared output.
                std::fs::write(dir.path().join("a"), "preparing").unwrap();
                std::fs::write(dir.path().join("b"), "preparing").unwrap();
                prepared.fetch_add(1, Ordering::SeqCst);
                let mut ctx = DeployContext::new(dir.path().to_path_buf(), slug.into());
                ctx.json_output = true;
                ctx.run_env.insert(
                    "APPZ_BATCH_TEST_ROOT".into(),
                    dir.path().to_string_lossy().into_owned(),
                );
                ctx.deploy_config.hooks.after_deploy = vec![
                    std::env::current_exe()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    "--exact".into(),
                    "deploy::tests::after_hook_waits_for_all_uploads_child".into(),
                ];
                Ok(PreparedTarget {
                    ctx,
                    provider: Box::new(UploadProbe {
                        exclusive: slug == "serial",
                        prepared: prepared.clone(),
                        started: started.clone(),
                    }),
                    req: req(&[], &[], None),
                    setup_names: Vec::new(),
                    cached: None,
                })
            },
        );
        assert_eq!(results.len(), 3);
        for result in results {
            assert!(result.error.is_none(), "{result:?}");
        }
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a")).unwrap(),
            "after hook changed output"
        );
    }

    #[test]
    fn failed_after_hook_invalidates_old_success_until_a_successful_retry() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".into());
        ctx.json_output = true;
        ctx.deploy_config.hooks.after_deploy = vec!["appz-no-such-hook-xyz".into()];
        let mut target = PreparedTarget {
            ctx,
            provider: get_provider("vercel").unwrap(),
            req: req(&[], &[], None),
            setup_names: Vec::new(),
            cached: None,
        };
        target.req.force = true;
        let mut output = DeployOutput::dry_run("vercel", true);
        output.url = Some("https://example.com".into());
        let fp = crate::deploy_state::fingerprint(
            dir.path(),
            &fingerprint_extra("vercel", &target.req, &target.ctx.deploy_config),
        )
        .unwrap();
        crate::deploy_state::record(
            dir.path(),
            "vercel",
            false,
            &fp,
            output.url.as_ref().unwrap(),
        );
        for _ in 0..2 {
            assert!(finish_target(&target, &output).is_some());
            assert!(crate::deploy_state::unchanged(dir.path(), "vercel", false, &fp).is_none());
        }
        target.ctx.deploy_config.hooks.after_deploy.clear();
        assert!(finish_target(&target, &output).is_none());
        assert!(crate::deploy_state::unchanged(dir.path(), "vercel", false, &fp).is_some());
    }

    #[test]
    fn hook_child_process() {
        let Ok(path) = std::env::var("APPZ_HOOK_TEST_OUTPUT") else {
            return;
        };
        assert_eq!(
            std::env::var("APPZ_HOOK_TEST_VALUE").unwrap(),
            "literal ; & value"
        );
        std::fs::write(path, "hook ran").unwrap();
        println!("hook stdout must not pollute JSON");
    }

    #[test]
    fn hooks_receive_the_deployment_environment() {
        let dir = project_with_targets("hook", "{}");
        let mut ctx = DeployContext::new(dir.clone(), "dist".into());
        ctx.json_output = true;
        ctx.run_env.insert(
            "APPZ_HOOK_TEST_OUTPUT".into(),
            dir.join("result").to_string_lossy().into_owned(),
        );
        ctx.run_env
            .insert("APPZ_HOOK_TEST_VALUE".into(), "literal ; & value".into());
        let argv = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--exact".into(),
            "deploy::tests::hook_child_process".into(),
            "--nocapture".into(),
        ];
        run_hook(&ctx, "before_deploy", &argv).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("result")).unwrap(),
            "hook ran"
        );
        assert!(
            run_hook(&ctx, "after_deploy", &["appz-nonexistent-hook-xyz".into()])
                .unwrap_err()
                .contains("after_deploy hook")
        );
        run_hook(&ctx, "before_deploy", &[]).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hooks_do_not_write_to_json_stdout() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "deploy::tests::hooks_receive_the_deployment_environment",
                "--nocapture",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !stdout.contains("hook stdout must not pollute JSON"),
            "{stdout}"
        );
    }

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

    fn project_with_targets(name: &str, targets: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("appz-run-all-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("appz.jsonc"),
            format!(r#"{{ "deploy": {{ "targets": {targets} }} }}"#),
        )
        .unwrap();
        dir
    }

    #[test]
    fn run_all_without_targets_is_an_error() {
        let dir = project_with_targets("none", "{}");
        let err = run_all(&dir, req(&[], &[], None)).unwrap_err();
        assert!(err.contains("no deploy targets configured"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn run_all_reports_every_target_in_sorted_order_even_when_they_fail() {
        let dir = project_with_targets("fail", r#"{ "zz-unknown": {}, "aa-unknown": {} }"#);
        let results = run_all(&dir, req(&[], &[], None)).unwrap();
        let slugs: Vec<_> = results.iter().map(|r| r.target.as_str()).collect();
        assert_eq!(slugs, ["aa-unknown", "zz-unknown"]);
        assert!(
            results
                .iter()
                .all(|r| r.output.is_none() && r.error.is_some())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn target_result_json_omits_the_absent_side() {
        let ok = TargetResult {
            target: "vercel".into(),
            output: Some(DeployOutput::dry_run("vercel", true)),
            error: None,
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert!(v.get("output").is_some() && v.get("error").is_none());

        // A failed after_deploy hook keeps the output next to the error.
        let hook_failed = TargetResult {
            error: Some("after_deploy hook 'x' failed".into()),
            ..ok
        };
        let v = serde_json::to_value(&hook_failed).unwrap();
        assert!(v.get("output").is_some() && v.get("error").is_some());

        let failed = TargetResult {
            target: "netlify".into(),
            output: None,
            error: Some("boom".into()),
        };
        let v = serde_json::to_value(&failed).unwrap();
        assert!(v.get("output").is_none());
        assert_eq!(v["error"], "boom");
    }
}
