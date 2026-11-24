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
    /// Run a best-effort source backup after a successful non-dry-run deploy.
    pub backup: bool,
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
/// target, the project name, the environment's configured vars, and the `-e` / `-b` values. Only hashed into the
/// fingerprint, never stored.
fn fingerprint_extra(slug: &str, req: &DeployRequest, config: &DeployConfig) -> String {
    // The command uses the last value for repeated keys.
    let effective_env = |flags: &[String]| {
        flags
            .iter()
            .filter_map(|s| parse_kv(s))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    serde_json::json!([
        if slug == "appzdev" {
            "appzdev:asset-fingerprints-b3-v1"
        } else {
            slug
        },
        req.project,
        config.targets.get(slug),
        config
            .env_for(if req.prod { "production" } else { "preview" })
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
        effective_env(&req.env),
        effective_env(&req.build_env)
    ])
    .to_string()
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
        return Ok(());
    }
    // Hooks receive deployment credentials, so keep their captured output out of result JSON.
    Err(match output.code {
        Some(code) => format!("{name} hook '{bin}' failed with exit code {code}"),
        None => format!("{name} hook '{bin}' terminated without an exit code"),
    })
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
    let (mut output, fingerprint) = upload_target(&target)?;
    match finish_target(&target, &mut output, fingerprint.as_deref()) {
        Some(error) => Err(format!(
            "deployed ({}) but {error}",
            output.url.as_deref().unwrap_or("url unknown")
        )),
        None => Ok(output),
    }
}

fn prepare_target(project_dir: &Path, req: DeployRequest) -> Result<PreparedTarget, String> {
    let mut config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let slug = match req.target.clone().or_else(|| config.default.clone()) {
        Some(slug) => slug,
        None => autodetect_target(project_dir)?,
    };

    if let Some(project) = &req.project {
        let target = config
            .targets
            .entry(slug.clone())
            .or_insert_with(|| serde_json::json!({}));
        target
            .as_object_mut()
            .ok_or_else(|| format!("deploy.targets.{slug} must be an object"))?
            .insert("project".into(), serde_json::Value::String(project.clone()));
    }
    let provider = get_provider(&slug).map_err(|e| e.to_string())?;
    let prebuilt_output = if slug == "github-pages" {
        config
            .targets
            .get(&slug)
            .and_then(|target| target.get("outputDir"))
            .map(|value| {
                value
                    .as_str()
                    .filter(|path| !path.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or(
                        "deploy.targets.github-pages.outputDir must be a non-empty directory path",
                    )
            })
            .transpose()?
    } else {
        None
    };
    let mut ctx = DeployContext::new(project_dir.to_path_buf(), "dist".into());

    // Nothing changed since the last deploy of this target: do not install,
    // build and upload it all again.
    // The check is only an optimization: if the project cannot be fingerprinted
    // the deploy goes ahead.
    // Render deploys its connected remote, whose revision isn't in this hash.
    // An explicitly supplied build may live outside the source fingerprint.
    let cache_local_files = slug != "render" && prebuilt_output.is_none();
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

    let toolchains = if prebuilt_output.is_some() {
        Vec::new()
    } else {
        appz_core::detect_toolchains(project_dir).map_err(|e| e.to_string())?
    };
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

    ctx.prebuilt = prebuilt_output.is_some();
    ctx.output_dir = prebuilt_output
        .unwrap_or_else(|| appz_core::primary_output_directory(&toolchains).to_string());
    ctx.is_preview = !req.prod;
    ctx.dry_run = req.dry_run;
    ctx.json_output = req.json;
    ctx.framework = framework;
    ctx.deploy_config = config;
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
    if !req.dry_run && !ctx.prebuilt {
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

    Ok(PreparedTarget {
        ctx,
        provider,
        req,
        setup_names,
        cached: None,
    })
}

fn upload_target(target: &PreparedTarget) -> Result<(DeployOutput, Option<String>), String> {
    if let Some(output) = &target.cached {
        return Ok((output.clone(), None));
    }
    let ctx = &target.ctx;
    let provider = &target.provider;
    let mut output = if ctx.is_preview {
        provider.deploy_preview(ctx)
    } else {
        provider.deploy(ctx)
    }
    .map_err(|e| e.to_string())?;
    output.setup_files = target.setup_names.clone();
    output.link_needed = ctx.dry_run && !provider.is_linked(ctx);
    if ctx.dry_run {
        if ensure_cli_available(provider.as_ref(), &ctx.extra_path).is_ok() {
            output.auth = Some(provider.auth_status(ctx));
        } else {
            output.cli_missing = true;
        }
    }
    // Capture the uploaded revision before another target builds or any after hook runs.
    let slug = provider.slug();
    let fingerprint = if !ctx.dry_run && slug != "render" && output.url.is_some() {
        crate::deploy_state::fingerprint(
            &ctx.project_dir,
            &fingerprint_extra(slug, &target.req, &ctx.deploy_config),
        )
        .ok()
    } else {
        None
    };
    Ok((output, fingerprint))
}

fn finish_target(
    target: &PreparedTarget,
    output: &mut DeployOutput,
    fingerprint: Option<&str>,
) -> Option<String> {
    finish_target_with(target, output, fingerprint, crate::backup::after_deploy)
}

fn finish_target_with(
    target: &PreparedTarget,
    output: &mut DeployOutput,
    fingerprint: Option<&str>,
    run_backup: impl FnOnce(
        &Path,
        &crate::backup::AfterDeploy<'_>,
    ) -> Option<appz_deploy::BackupSummary>,
) -> Option<String> {
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
    if let (Some(fp), Some(url)) = (fingerprint, &output.url) {
        crate::deploy_state::record(&ctx.project_dir, slug, target.req.prod, fp, url);
    }
    if target.req.backup {
        let build_env_names = build_env_var_names(&target.req);
        let interactive = if !target.req.json && crate::is_tty() {
            crate::backup::Interactive::Yes
        } else {
            crate::backup::Interactive::No
        };
        output.backup = run_backup(
            &ctx.project_dir,
            &crate::backup::AfterDeploy {
                // `after_deploy` derives its dedup identity from the backup selection itself.
                fingerprint: "",
                target: slug,
                production: target.req.prod,
                url: output.url.as_deref(),
                build_env_names,
                interactive,
                json: target.req.json,
            },
        );
    }
    None
}

fn build_env_var_names(req: &DeployRequest) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for entry in &req.build_env {
        let key = entry.split_once('=').map_or(entry.as_str(), |(k, _)| k);
        if !key.is_empty() {
            names.push(key.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
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
/// hide successful uploads. Remote-source uploads and dry-run probes can
/// overlap when no prompt is needed.
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
    let upload = |target: PreparedTarget| {
        let uploaded = upload_target(&target)?;
        Ok((target, uploaded))
    };
    let mut order = slugs;
    // Launch Render's remote-source deploy first so it can overlap a local upload.
    if parallel {
        order.sort_by_key(|slug| slug != "render");
    }
    let mut uploads = std::thread::scope(|scope| {
        let mut uploads = Vec::new();
        let mut workers = Vec::new();
        for slug in order {
            match prepare(&slug) {
                Ok(target) if parallel && (target.ctx.dry_run || slug == "render") => {
                    workers.push((slug, scope.spawn(move || upload(target))));
                }
                prepared => uploads.push((slug, prepared.and_then(upload))),
            }
        }
        for (slug, worker) in workers {
            uploads.push((
                slug,
                worker
                    .join()
                    .unwrap_or_else(|_| Err("deploy thread panicked".into())),
            ));
        }
        uploads
    });
    uploads.sort_by(|a, b| a.0.cmp(&b.0));
    uploads
        .into_iter()
        .map(|(target, result)| match result {
            Ok((prepared, (mut output, fingerprint))) => TargetResult {
                error: finish_target(&prepared, &mut output, fingerprint.as_deref()),
                target,
                output: Some(output),
            },
            Err(error) => TargetResult {
                target,
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

/// What `appz rollback` is about to do: the live production deployment and
/// the earlier one production will be repointed at.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RollbackPlan {
    pub current: Option<appz_deploy::TeamDeployment>,
    pub target: appz_deploy::TeamDeployment,
}

/// Pick the rollback target from a project's ready production deployments:
/// the newest one older than the current (`isCurrent`, else newest) deployment.
pub fn pick_rollback_target(
    mut items: Vec<appz_deploy::TeamDeployment>,
) -> Result<RollbackPlan, String> {
    items.retain(|d| d.version_id.is_some());
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let cur = items.iter().position(|d| d.is_current).unwrap_or(0);
    if items.is_empty() {
        return Err("no ready production deployments to roll back to".into());
    }
    let current = items[cur].clone();
    let target = items
        .into_iter()
        .skip(cur + 1)
        .find(|d| d.created_at < current.created_at && d.id != current.id)
        .ok_or_else(|| {
            "no earlier ready production deployment to roll back to; pass a version ID explicitly"
                .to_string()
        })?;
    Ok(RollbackPlan {
        current: Some(current),
        target,
    })
}

/// Find the previous ready production deployment for the project in `project_dir`.
pub fn plan_rollback(project_dir: &Path, target: Option<String>) -> Result<RollbackPlan, String> {
    if !lists_via_appzdev(project_dir, target.as_deref())? {
        return Err(
            "rolling back without a version is only supported on appz.dev; pass a version ID"
                .into(),
        );
    }
    let query = appz_deploy::DeploymentQuery {
        project: Some(appz_deploy::resolve_project_id_from_dir(project_dir)),
        production: true,
        status: Some("ready".into()),
        limit: Some(100),
        ..Default::default()
    };
    pick_rollback_target(list_page(project_dir, target, &query)?.items)
}

/// Whether `arg` is a version ID (`ver_...` or a 26-char ULID) rather than a
/// deployment ID or URL.
pub fn is_version_id(arg: &str) -> bool {
    arg.starts_with("ver_") || (arg.len() == 26 && arg.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Accept a version ID as-is; resolve a deployment ID (`dpl_...`) or URL to its version.
pub fn resolve_version(
    project_dir: &Path,
    target: Option<String>,
    arg: &str,
) -> Result<String, String> {
    if is_version_id(arg) {
        return Ok(arg.to_string());
    }
    inspect(project_dir, target, arg).map(|d| d.version_id)
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

/// Whether listing for `project_dir` goes to appz.dev: the resolved provider
/// (`--target`, then the configured default, then detection) is `appzdev`, or
/// nothing is detected, in which case the team-wide listing applies.
pub fn lists_via_appzdev(project_dir: &Path, target: Option<&str>) -> Result<bool, String> {
    let config = read_deploy_config(project_dir)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let slug = match target.map(String::from).or(config.default) {
        Some(slug) => slug,
        None => autodetect_target(project_dir).unwrap_or_else(|_| "appzdev".to_string()),
    };
    Ok(slug == "appzdev")
}

/// Project id of the project linked to `project_dir` (`.appz/project.json`), if any.
pub fn linked_project(project_dir: &Path) -> Option<String> {
    project_dir
        .join(".appz")
        .join("project.json")
        .is_file()
        .then(|| appz_deploy::resolve_project_id_from_dir(project_dir))
}

/// One page of deployments for a project, or for the whole team when
/// `query.project` is `None`.
pub fn list_page(
    project_dir: &Path,
    target: Option<String>,
    query: &appz_deploy::DeploymentQuery,
) -> Result<appz_deploy::Page<appz_deploy::TeamDeployment>, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .list_team_deployments(&ctx, query)
        .map_err(|e| e.to_string())
}

pub fn list_domains_page(
    project_dir: &Path,
    target: Option<String>,
    query: &appz_deploy::DomainQuery,
) -> Result<appz_deploy::Page<appz_deploy::TeamDomain>, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .list_team_domains(&ctx, query)
        .map_err(|e| e.to_string())
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

pub fn restore_project(
    project_dir: &Path,
    target: Option<String>,
    id_or_slug: &str,
) -> Result<appz_deploy::ProjectInfo, String> {
    let (provider, ctx) = domain_ctx(project_dir, target)?;
    provider
        .restore_project(&ctx, id_or_slug)
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

    #[test]
    fn lists_via_appzdev_follows_the_resolved_provider() {
        let dir = tempfile::tempdir().unwrap();
        assert!(lists_via_appzdev(dir.path(), None).unwrap());
        assert!(!lists_via_appzdev(dir.path(), Some("vercel")).unwrap());
        std::fs::write(
            dir.path().join("appz.jsonc"),
            r#"{ "deploy": { "default": "vercel" } }"#,
        )
        .unwrap();
        assert!(!lists_via_appzdev(dir.path(), None).unwrap());
        assert!(lists_via_appzdev(dir.path(), Some("appzdev")).unwrap());
    }

    struct UploadProbe {
        slug: &'static str,
        started: Arc<AtomicUsize>,
    }

    impl DeployProvider for UploadProbe {
        fn name(&self) -> &'static str {
            "test"
        }
        fn slug(&self) -> &'static str {
            self.slug
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
            if self.slug != "render" {
                assert_eq!(
                    std::fs::read_to_string(ctx.project_dir.join("artifact")).unwrap(),
                    self.slug
                );
            }
            self.started.fetch_add(1, Ordering::SeqCst);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while self.started.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(
                self.started.load(Ordering::SeqCst) >= 2,
                "remote and local uploads did not overlap"
            );
            std::fs::write(
                ctx.project_dir
                    .join(format!(".appz/uploaded-{}", self.slug)),
                "done",
            )
            .unwrap();
            let mut output = DeployOutput::dry_run(self.slug, false);
            output.url = Some("https://example.com".into());
            Ok(output)
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
        assert!(root.join(".appz/uploaded-a").exists());
        assert!(root.join(".appz/uploaded-b").exists());
        assert!(root.join(".appz/uploaded-render").exists());
        std::fs::write(root.join("artifact"), "after hook changed output").unwrap();
    }

    #[test]
    fn batch_uploads_each_local_build_before_preparing_the_next_and_fingerprints_before_hooks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".appz")).unwrap();
        let started = Arc::new(AtomicUsize::new(0));
        let results = run_targets(
            vec!["a".into(), "b".into(), "render".into()],
            true,
            |slug| {
                // Model a build that clears and rewrites the shared output.
                let slug = match slug {
                    "a" => "a",
                    "b" => "b",
                    "render" => "render",
                    _ => unreachable!(),
                };
                std::fs::write(dir.path().join("artifact"), slug).unwrap();
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
                        slug,
                        started: started.clone(),
                    }),
                    req: req(&[], &[], None),
                    setup_names: Vec::new(),
                    cached: None,
                })
            },
        );
        assert_eq!(
            results
                .iter()
                .map(|r| r.target.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "render"]
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("artifact")).unwrap(),
            "after hook changed output"
        );
        for result in results {
            assert!(result.error.is_none(), "{result:?}");
            if result.target != "render" {
                let extra = fingerprint_extra(
                    &result.target,
                    &req(&[], &[], None),
                    &DeployConfig::default(),
                );
                std::fs::write(dir.path().join("artifact"), "after hook changed output").unwrap();
                let fingerprint = crate::deploy_state::fingerprint(dir.path(), &extra).unwrap();
                assert!(
                    crate::deploy_state::unchanged(dir.path(), &result.target, false, &fingerprint)
                        .is_none()
                );
                std::fs::write(dir.path().join("artifact"), &result.target).unwrap();
                let uploaded = crate::deploy_state::fingerprint(dir.path(), &extra).unwrap();
                assert!(
                    crate::deploy_state::unchanged(dir.path(), &result.target, false, &uploaded)
                        .is_some()
                );
            }
        }
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
        target.req.backup = false;
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
            assert!(finish_target(&target, &mut output, Some(&fp)).is_some());
            assert!(crate::deploy_state::unchanged(dir.path(), "vercel", false, &fp).is_none());
        }
        target.ctx.deploy_config.hooks.after_deploy.clear();
        assert!(finish_target(&target, &mut output, Some(&fp)).is_none());
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
        let bad_arg = vec![argv[0].clone(), "--appz-invalid-hook-argument".into()];
        let error = run_hook(&ctx, "after_deploy", &bad_arg).unwrap_err();
        assert!(error.contains("exit code"), "{error}");
        assert!(!error.contains("appz-invalid-hook-argument"), "{error}");
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
            backup: true,
            install: Consent::Never,
            login: Consent::Never,
            link: Consent::Never,
        }
    }

    #[test]
    fn github_pages_dry_run_uses_zolas_default_output() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("zola.toml"),
            "base_url = 'https://example.org/site'",
        )
        .unwrap();
        let output = dir.path().join("public");
        std::fs::create_dir(&output).unwrap();
        std::fs::write(output.join("index.html"), "<h1>Zola site</h1>").unwrap();
        let mut request = req(&[], &[], None);
        request.target = Some("github-pages".into());
        request.prod = true;
        request.dry_run = true;

        let result = run(dir.path(), request).unwrap();
        assert_eq!(result.provider, "github-pages");
    }

    #[test]
    fn github_pages_dry_run_accepts_output_from_an_unknown_generator() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("appz.jsonc"),
            r#"{"deploy":{"default":"github-pages","targets":{"github-pages":{"outputDir":"rendered site"}}}}"#,
        )
        .unwrap();
        let output = dir.path().join("rendered site");
        std::fs::create_dir(&output).unwrap();
        std::fs::write(output.join("index.html"), "<h1>Static build</h1>").unwrap();
        let mut request = req(&[], &[], None);
        request.prod = true;
        request.dry_run = true;

        let result = run(dir.path(), request).unwrap();
        assert_eq!(result.provider, "github-pages");
        assert!(!result.is_preview);
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
        assert_ne!(
            fingerprint_extra("vercel", &req(&["A=1", "A=2"], &[], None), &cfg),
            fingerprint_extra("vercel", &req(&["A=2", "A=1"], &[], None), &cfg)
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

    fn prepared_for_test(dir: &Path, mut request: DeployRequest) -> PreparedTarget {
        request.target = Some("vercel".into());
        let mut ctx = DeployContext::new(dir.to_path_buf(), "dist".into());
        ctx.dry_run = request.dry_run;
        ctx.json_output = request.json;
        PreparedTarget {
            ctx,
            provider: get_provider("vercel").unwrap(),
            req: request,
            setup_names: Vec::new(),
            cached: None,
        }
    }

    #[test]
    fn dry_run_never_backs_up() {
        let dir = tempfile::tempdir().unwrap();
        let mut request = req(&[], &[], None);
        request.dry_run = true;
        let target = prepared_for_test(dir.path(), request);
        let mut output = DeployOutput::dry_run("vercel", true);
        output.url = Some("https://preview.example.com".into());

        let mut called = false;
        let err = finish_target_with(&target, &mut output, Some("fp-1"), |_, _| {
            called = true;
            None
        });
        assert!(err.is_none());
        assert!(!called);
        assert!(output.backup.is_none());
    }

    #[test]
    fn no_backup_flag_skips() {
        let dir = tempfile::tempdir().unwrap();
        let mut request = req(&[], &[], None);
        request.backup = false;
        let target = prepared_for_test(dir.path(), request);
        let mut output = DeployOutput::dry_run("vercel", true);
        output.status = appz_deploy::DeployStatus::Ready;
        output.url = Some("https://preview.example.com".into());

        let mut called = false;
        let err = finish_target_with(&target, &mut output, Some("fp-1"), |_, _| {
            called = true;
            None
        });
        assert!(err.is_none());
        assert!(!called);
        assert!(output.backup.is_none());
    }

    #[test]
    fn backup_failure_does_not_change_the_deploy_result() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hi</h1>").unwrap();
        let request = req(&[], &[], None);
        let target = prepared_for_test(dir.path(), request);
        let mut output = DeployOutput::dry_run("vercel", true);
        output.status = appz_deploy::DeployStatus::Ready;
        output.url = Some("https://preview.example.com".into());

        let err = finish_target_with(&target, &mut output, Some("fp-1"), |root, ad| {
            crate::backup::run_after_deploy_failed_fixture(root, ad)
        });
        assert!(err.is_none());
        assert_eq!(output.status, appz_deploy::DeployStatus::Ready);
        assert_eq!(output.url.as_deref(), Some("https://preview.example.com"));
        let backup = output
            .backup
            .expect("skipped backup summary should be recorded");
        assert_eq!(backup.snapshot, None);
        assert!(
            backup
                .skipped
                .as_deref()
                .is_some_and(|s| s.contains("boom")),
            "{:?}",
            backup.skipped
        );
    }

    #[test]
    fn json_output_includes_backup_summary() {
        let dir = tempfile::tempdir().unwrap();
        let mut request = req(&[], &[], None);
        request.json = true;
        let target = prepared_for_test(dir.path(), request);
        let mut output = DeployOutput::dry_run("vercel", true);
        output.status = appz_deploy::DeployStatus::Ready;
        output.url = Some("https://preview.example.com".into());

        let err = finish_target_with(&target, &mut output, Some("fp-1"), |_, ad| {
            assert_eq!(ad.interactive, crate::backup::Interactive::No);
            assert!(ad.json);
            Some(appz_deploy::BackupSummary {
                snapshot: Some("snap-42".into()),
                added_bytes: 2048,
                skipped: None,
            })
        });
        assert!(err.is_none());

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["backup"]["snapshot"], "snap-42");
        assert_eq!(json["backup"]["added_bytes"], 2048);
        assert!(json["backup"].get("skipped").is_none());

        // When backup is None, the field is omitted (`skip_serializing_if = "Option::is_none"`).
        let bare = serde_json::to_value(DeployOutput::dry_run("vercel", true)).unwrap();
        assert!(bare.get("backup").is_none());
    }

    #[test]
    fn url_less_and_render_deploys_still_run_backup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hi</h1>").unwrap();
        let target = prepared_for_test(dir.path(), req(&[], &[], None));
        let mut output = DeployOutput::dry_run("render", true);
        output.status = appz_deploy::DeployStatus::Ready;
        output.url = None;

        let mut called = false;
        let err = finish_target_with(&target, &mut output, None, |_, ad| {
            called = true;
            assert_eq!(ad.fingerprint, "");
            assert_eq!(ad.url, None);
            Some(appz_deploy::BackupSummary {
                snapshot: Some("snap-url-less".into()),
                added_bytes: 12,
                skipped: None,
            })
        });
        assert!(err.is_none());
        assert!(called);
        assert_eq!(
            output.backup.as_ref().and_then(|b| b.snapshot.as_deref()),
            Some("snap-url-less")
        );
        assert!(crate::deploy_state::unchanged(dir.path(), "vercel", false, "").is_none());
    }

    #[test]
    fn backup_runs_after_after_deploy_hook_edits() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>before</h1>").unwrap();
        let mut target = prepared_for_test(dir.path(), req(&[], &[], None));
        #[cfg(windows)]
        let hook = vec![
            "cmd".into(),
            "/C".into(),
            "echo after > generated.txt".into(),
        ];
        #[cfg(not(windows))]
        let hook = vec![
            "sh".into(),
            "-c".into(),
            "echo after > generated.txt".into(),
        ];
        target.ctx.deploy_config.hooks.after_deploy = hook;
        let pre_hook_fp = crate::deploy_state::fingerprint(
            dir.path(),
            &fingerprint_extra("vercel", &target.req, &target.ctx.deploy_config),
        )
        .unwrap();

        let mut output = DeployOutput::dry_run("vercel", true);
        output.status = appz_deploy::DeployStatus::Ready;
        output.url = Some("https://preview.example.com".into());

        let mut saw_hook_output = false;
        let err = finish_target_with(&target, &mut output, Some(&pre_hook_fp), |root, _| {
            saw_hook_output = root.join("generated.txt").exists();
            None
        });
        assert!(err.is_none(), "{err:?}");
        assert!(saw_hook_output);
        assert!(
            crate::deploy_state::unchanged(dir.path(), "vercel", false, &pre_hook_fp).is_some()
        );
    }
}

#[cfg(test)]
mod rollback_tests {
    use super::*;

    fn dep(id: &str, at: i64, current: bool) -> appz_deploy::TeamDeployment {
        serde_json::from_value(serde_json::json!({
            "id": id, "versionId": format!("ver_{id}"), "createdAt": at, "isCurrent": current
        }))
        .unwrap()
    }

    #[test]
    fn picks_next_older_than_current() {
        let plan = pick_rollback_target(vec![
            dep("a", 1, false),
            dep("c", 3, true),
            dep("b", 2, false),
        ])
        .unwrap();
        assert_eq!(plan.current.unwrap().id, "c");
        assert_eq!(plan.target.id, "b");
    }

    #[test]
    fn skips_newer_than_current_and_defaults_to_newest() {
        // After a rollback the current one is not the newest.
        let plan = pick_rollback_target(vec![
            dep("c", 3, false),
            dep("b", 2, true),
            dep("a", 1, false),
        ])
        .unwrap();
        assert_eq!(plan.target.id, "a");
        let plan = pick_rollback_target(vec![dep("b", 2, false), dep("a", 1, false)]).unwrap();
        assert_eq!(plan.target.id, "a");
    }

    #[test]
    fn errors_without_earlier_deployment() {
        assert!(pick_rollback_target(vec![dep("a", 1, true)]).is_err());
        assert!(pick_rollback_target(vec![]).is_err());
    }

    #[test]
    fn tells_versions_from_deployments() {
        assert!(is_version_id("ver_abc123def456"));
        assert!(is_version_id("01HZX3J5K6M7N8P9Q0R1S2T3V4"));
        assert!(!is_version_id("dpl_abc123def456"));
        assert!(!is_version_id("https://x.appz.page"));
    }
}
