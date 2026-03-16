//! Deploy command implementation.
//!
//! Orchestrates the deployment flow:
//! 1. Create a sandbox for the project directory.
//! 2. Resolve provider (from arg, detection, or config).
//! 3. Check prerequisites via sandbox.
//! 4. Optionally build before deploying.
//! 5. Run deploy hooks via sandbox.
//! 6. Deploy via the provider (all exec goes through sandbox).
//! 7. Display results (or JSON output for CI).

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use deployer::{
    self, detect_all, get_provider, is_ci_environment, read_deploy_config, write_deploy_config,
    create_provider_registry, DeployConfig, DeployContext, DeployOutput,
    DeployProvider, DetectedPlatform, SetupContext,
};
use miette::{miette, Report, Result};
use starbase::AppResult;

use crate::app_error::UserCancellation;
use crate::session::AppzSession;

// ---------------------------------------------------------------------------
// Appz platform deploy (when linked)
// ---------------------------------------------------------------------------

/// Parse KEY=VALUE strings into a JSON map for deployment meta.
fn parse_meta_kv(meta: &[String]) -> Option<serde_json::Map<String, serde_json::Value>> {
    if meta.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    for kv in meta {
        if let Some((k, v)) = kv.split_once('=') {
            map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Add git_branch and git_commit to meta when in a git repo (Vercel parity).
fn enrich_meta_with_git(
    project_dir: &std::path::Path,
    base: Option<serde_json::Map<String, serde_json::Value>>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(project_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match (base, branch, commit) {
        (None, None, None) => None,
        (mut map, br, cm) => {
            if map.is_none() && (br.is_some() || cm.is_some()) {
                map = Some(serde_json::Map::new());
            }
            if let Some(m) = &mut map {
                if let Some(b) = br {
                    m.insert("git_branch".to_string(), serde_json::Value::String(b));
                }
                if let Some(c) = cm {
                    m.insert("git_commit".to_string(), serde_json::Value::String(c));
                }
            }
            map
        }
    }
}

/// Deploy to Appz when project is linked (.appz/project.json).
#[cfg(feature = "deploy")]
async fn deploy_to_appz(
    session: AppzSession,
    project_dir: std::path::PathBuf,
    output_dir: String,
    preview: bool,
    no_build: bool,
    dry_run: bool,
    json_output: bool,
    meta: Vec<String>,
    skip_domain: bool,
    force: bool,
) -> AppResult {
    let link = crate::project::read_project_link(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .ok_or_else(|| miette!("Project link not found"))?;

    let output_path = project_dir.join(&output_dir);
    if !dry_run && !output_path.exists() {
        if !no_build {
            crate::commands::build(session.clone()).await?;
        }
        if !output_path.exists() {
            return Err(miette!(
                "Build output directory not found: {}\n\
                 Run 'appz build' first or check outputDirectory in appz.json.",
                output_path.display()
            ));
        }
    }

    if dry_run {
        let _ = ui::status::info(&format!(
            "Would deploy {} to Appz (preview={})",
            output_path.display(),
            preview
        ));
        return Ok(None);
    }

    let client = session.get_api_client();
    if client.get_token().await.is_none() {
        return Err(miette!(
            "Not logged in. Run 'appz login' or set APPZ_TOKEN."
        ));
    }

    let ctx = appz_client::DeployContext {
        output_dir: output_path,
        project_id: link.link.project_id.clone(),
        team_id: Some(link.link.team_id.clone()),
        target: if preview { "preview" } else { "production" }.to_string(),
        name: link.link.project_name.clone(),
        meta: enrich_meta_with_git(&project_dir, parse_meta_kv(&meta)),
        skip_domain,
        force,
    };

    if !json_output {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title("Deploying to Appz");
    }

    let is_tty = atty::is(atty::Stream::Stderr);
    let sp = if !json_output {
        Some(ui::progress::spinner("Deploying to Appz..."))
    } else {
        None
    };

    let output = if json_output {
        appz_client::deploy_prebuilt(client.clone(), ctx.clone())
            .await
            .map_err(|e| miette!("{}", e))?
    } else {
        let last_printed_pct = Arc::new(AtomicU8::new(0));
        appz_client::deploy_prebuilt_stream(client, ctx, move |ev| {
            if let Some(sp) = sp.as_ref() {
                match ev {
                    appz_client::DeployEvent::Preparing => {
                        sp.set_message("Deploying to Appz...");
                    }
                    appz_client::DeployEvent::FileCount {
                        total_bytes,
                        ..
                    } => {
                        // Show initial upload progress so user sees we're uploading
                        if total_bytes > 0 {
                            let msg = format!(
                                "Uploading [{}] (0 B/{})",
                                ui::progress::bar_string(0, total_bytes, 20),
                                ui::format::bytes(total_bytes)
                            );
                            sp.set_message(&msg);
                        }
                    }
                    appz_client::DeployEvent::UploadProgress {
                        uploaded_bytes,
                        total_bytes,
                    } => {
                        let should_update = if is_tty {
                            true
                        } else {
                            let pct = if total_bytes > 0 {
                                ((uploaded_bytes as f64 / total_bytes as f64) * 100.0) as u8
                            } else {
                                100
                            };
                            let stepped = (pct / 25) * 25;
                            let prev = last_printed_pct.load(Ordering::Relaxed);
                            if stepped > prev {
                                last_printed_pct.store(stepped, Ordering::Relaxed);
                                true
                            } else {
                                false
                            }
                        };
                        if should_update {
                            let msg = format!(
                                "Uploading [{}] ({}/{})",
                                ui::progress::bar_string(uploaded_bytes, total_bytes, 20),
                                ui::format::bytes(uploaded_bytes),
                                ui::format::bytes(total_bytes)
                            );
                            sp.set_message(&msg);
                        }
                    }
                    appz_client::DeployEvent::Processing => {
                        sp.set_message("Processing deployment...");
                    }
                    appz_client::DeployEvent::Ready {
                        url,
                        inspect_url,
                        is_production,
                        created_at,
                        ..
                    } => {
                        sp.finish();
                        // Vercel: print to stderr (same as spinner) for incremental display
                        let _ = std::io::stderr().write_all(b"\n");
                        let _ = std::io::stderr().flush();
                        if let Some(inspect) = &inspect_url {
                            let _ = std::io::stderr().write_all(format!("  Inspect: {}\n", inspect).as_bytes());
                            let _ = std::io::stderr().flush();
                        }
                        let deploy_type = if is_production { "Production" } else { "Preview" };
                        let _ = std::io::stderr().write_all(
                            format!("  {}: {} {}\n", deploy_type, url, ui::format::deploy_stamp(created_at)).as_bytes()
                        );
                        let _ = std::io::stderr().flush();
                        sp.finish_with_message("Deployed!");
                    }
                    appz_client::DeployEvent::Error(_) => {
                        sp.finish();
                    }
                    _ => {}
                }
            }
        })
        .await
        .map_err(|e| miette!("{}", e))?
    };

    let deploy_output = deployer::DeployOutput {
        url: output.url.clone(),
        status: deployer::DeployStatus::Ready,
        provider: "appz".to_string(),
        is_preview: preview,
        deployment_id: Some(output.deployment_id),
        additional_urls: vec![],
        duration_ms: None,
        created_at: Some(chrono::Utc::now()),
    };

    display_deploy_result(&deploy_output, json_output, true)?;
    write_ci_output(&deploy_output);

    Ok(None)
}

// ---------------------------------------------------------------------------
// Sandbox creation helper
// ---------------------------------------------------------------------------

/// Create and initialise a sandbox for the project directory.
///
/// The sandbox provides command execution (via mise), scoped filesystem
/// access, and tool management. All provider operations go through it
/// instead of raw `tokio::process::Command`.
pub async fn create_deploy_sandbox(
    project_dir: std::path::PathBuf,
) -> Result<Arc<dyn sandbox::SandboxProvider>> {
    let config = sandbox::SandboxConfig::new(&project_dir)
        .with_settings(sandbox::SandboxSettings::default().quiet());

    let sb = sandbox::create_sandbox(config)
        .await
        .map_err(|e| miette!("Failed to create deploy sandbox: {}", e))?;

    Ok(Arc::from(sb))
}

/// Main deploy command entry point (Vercel-parity options).
#[allow(clippy::too_many_arguments)]
pub async fn deploy(
    session: AppzSession,
    project_path: Option<std::path::PathBuf>,
    provider_arg: Option<String>,
    prod: bool,
    _preview: bool,
    target: Option<String>,
    prebuilt: bool,
    no_build: bool,
    build_env: Vec<String>,
    env: Vec<String>,
    force: bool,
    guidance: bool,
    logs: bool,
    meta: Vec<String>,
    no_wait: bool,
    public: bool,
    skip_domain: bool,
    with_cache: bool,
    dry_run: bool,
    json_output: bool,
    deploy_all: bool,
    yes: bool,
) -> AppResult {
    // Resolve project directory (Vercel: [project-path])
    let project_dir = match project_path {
        Some(p) => session
            .working_dir
            .join(&p)
            .canonicalize()
            .unwrap_or_else(|_| session.working_dir.join(p)),
        None => session.working_dir.clone(),
    };

    // Resolve target (Vercel-parity): --prod => production, --target production => production, else preview
    let is_production_target = target
        .as_ref()
        .map(|t| t == "production" || t == "prod")
        .unwrap_or(false);
    let is_preview = !prod && !is_production_target;

    // Skip build when --prebuilt or --no-build
    let no_build = no_build || prebuilt;

    let is_ci = is_ci_environment() || yes;

    // Stub for options not yet wired to backend: logs, no_wait, public, with_cache, build_env
    let _ = (logs, no_wait, public, with_cache, build_env);

    // 1. Load existing deploy config
    let deploy_config = read_deploy_config(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    // 2. Determine output directory
    let output_dir = resolve_output_dir(&project_dir);

    // 2b. If linked to Appz project and no explicit provider (or appz), use Appz platform
    let use_appz = provider_arg.as_ref().map(|s| s == "appz").unwrap_or(true);
    if crate::project::is_project_linked(&project_dir)
        && (provider_arg.is_none() || use_appz)
        && !deploy_all
    {
        return deploy_to_appz(
            session.clone(),
            project_dir.clone(),
            output_dir.clone(),
            is_preview,
            no_build,
            dry_run,
            json_output,
            meta.clone(),
            skip_domain,
            force,
        )
            .await;
    }

    // 3. Create sandbox for command execution
    let sandbox = create_deploy_sandbox(project_dir.clone()).await?;

    // 4. Handle --all flag
    if deploy_all {
        return deploy_to_all_targets(
            sandbox.clone(),
            output_dir.clone(),
            deploy_config.clone(),
            is_preview,
            no_build,
            dry_run,
            json_output,
            is_ci,
        )
        .await;
    }

    // 5. Resolve the provider
    let provider = resolve_provider(
        provider_arg.clone(),
        project_dir.clone(),
        deploy_config.clone(),
        is_ci,
    )
    .await?;

    if !json_output {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title(&format!("Deploying to {}", provider.name()));
    }

    // 6. Check prerequisites via sandbox
    let provider = handle_prerequisites(provider, sandbox.clone(), is_ci).await?;

    // 7. Build before deploy (unless --no-build)
    if !no_build && !dry_run {
        run_build_step(session.clone(), project_dir.clone(), json_output).await?;
    }

    // 8. Run before_deploy hook via sandbox
    if let Some(ref hooks) = deploy_config.hooks {
        if let Some(ref before) = hooks.before_deploy {
            run_hook(
                "before_deploy".to_string(),
                before.clone(),
                sandbox.clone(),
                json_output,
            )
            .await?;
        }
    }

    // 9. Build deploy context with sandbox
    let mut env_vars = deploy_config.env_for(if is_preview { "preview" } else { "production" });
    for kv in &env {
        if let Some((k, v)) = kv.split_once('=') {
            env_vars.insert(k.to_string(), v.to_string());
        }
    }
    let ctx = DeployContext::new(sandbox.clone(), output_dir)
        .with_preview(is_preview)
        .with_config(deploy_config.clone())
        .with_env(env_vars)
        .with_dry_run(dry_run)
        .with_json_output(json_output);

    // 10. Validate output directory exists (unless dry run)
    if !dry_run {
        let output_path = ctx.output_path();
        if !output_path.exists() {
            return Err(miette!(
                "Build output directory not found: {}\n\
                 Run 'appz build' first or check 'outputDirectory' in appz.json.",
                output_path.display()
            ));
        }
    }

    // 11. Deploy (with spinner when interactive, like sandbox init)
    let deploy_label = if is_preview {
        format!("{} (preview)", provider.name())
    } else {
        format!("{} (production)", provider.name())
    };

    let result = if !json_output && !dry_run {
        let sp = ui::progress::spinner(&format!("Deploying to {}...", deploy_label));
        let r = if is_preview {
            provider.deploy_preview(ctx.clone()).await
        } else {
            provider.deploy(ctx.clone()).await
        };
        let msg = if r.is_ok() { "Deployed!" } else { "Failed" };
        sp.finish_with_message(msg);
        r
    } else if is_preview {
        provider.deploy_preview(ctx).await
    } else {
        provider.deploy(ctx).await
    };

    let output = result.map_err(|e| miette!("{}", e))?;

    // 12. Run after_deploy hook via sandbox
    if let Some(ref hooks) = deploy_config.hooks {
        if let Some(ref after) = hooks.after_deploy {
            run_hook(
                "after_deploy".to_string(),
                after.clone(),
                sandbox.clone(),
                json_output,
            )
            .await?;
        }
    }

    // 13. Display results
    display_deploy_result(&output, json_output, false)?;

    // 14. Write GitHub Actions output if applicable
    write_ci_output(&output);

    // 15. --guidance: show suggested next steps and examples (Vercel-parity)
    if guidance {
        ui::guidance::deploy_guidance(&output.url, output.is_preview).display(json_output);
    }

    Ok(None)
}

/// Deploy init command — set up deployment for a provider.
pub async fn deploy_init(
    session: AppzSession,
    provider_arg: Option<String>,
) -> AppResult {
    let project_dir = session.working_dir.clone();
    let is_ci = is_ci_environment();

    if is_ci {
        return Err(miette!(
            "Deploy init requires interactive mode. Cannot run in CI/CD."
        ));
    }

    // If no provider specified, show list and prompt
    let provider_slug = match provider_arg {
        Some(slug) => slug,
        None => prompt_provider_selection().await?,
    };

    let provider = get_provider(&provider_slug).map_err(|e| miette!("{}", e))?;

    let _ = ui::layout::blank_line();

    // Read existing config
    let deploy_config = read_deploy_config(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    let output_dir = resolve_output_dir(&project_dir);

    // Create sandbox for setup operations
    let sandbox = create_deploy_sandbox(project_dir.clone()).await?;

    let mut setup_ctx = SetupContext::new(sandbox);
    setup_ctx.deploy_config = deploy_config.clone();
    setup_ctx.output_dir = Some(output_dir);

    // Run the provider's setup wizard
    let provider_config = provider.setup(&mut setup_ctx).await.map_err(|e| miette!("{}", e))?;

    // Save to appz.json
    let mut new_config = deploy_config;
    new_config
        .targets
        .insert(provider_slug.clone(), provider_config);
    if new_config.default.is_none() {
        new_config.default = Some(provider_slug.clone());
    }

    write_deploy_config(&project_dir, &new_config).map_err(|e| miette!("{}", e))?;

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "{} deployment configured! Run 'appz deploy' to deploy.",
        provider.name()
    ));

    Ok(None)
}

/// Deploy list command — show recent deployments.
pub async fn deploy_list(
    session: AppzSession,
    provider_arg: Option<String>,
) -> AppResult {
    let project_dir = session.working_dir.clone();
    let deploy_config = read_deploy_config(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    let output_dir = resolve_output_dir(&project_dir);

    let provider_slug = match provider_arg {
        Some(slug) => slug,
        None => deploy_config
            .default
            .clone()
            .ok_or_else(|| miette!("No default provider configured. Specify a provider: appz deploy-list <provider>"))?,
    };

    let provider = get_provider(&provider_slug).map_err(|e| miette!("{}", e))?;

    // Create sandbox for the list operation
    let sandbox = create_deploy_sandbox(project_dir.clone()).await?;

    let ctx = DeployContext::new(sandbox, output_dir)
        .with_config(deploy_config);

    let deployments = provider
        .list_deployments(ctx)
        .await
        .map_err(|e| miette!("{}", e))?;

    if deployments.is_empty() {
        let _ = ui::status::info("No deployments found.");
    } else {
        let _ = ui::layout::section_title(&format!("Recent {} deployments", provider.name()));
        for dep in &deployments {
            let status_str = format!("{}", dep.status);
            let current = if dep.is_current { " (current)" } else { "" };
            println!("  {} {} {}{}", dep.id, dep.url, status_str, current);
        }
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Resolve which provider to use based on arg, detection, and config.
async fn resolve_provider(
    provider_arg: Option<String>,
    project_dir: std::path::PathBuf,
    deploy_config: DeployConfig,
    is_ci: bool,
) -> Result<Box<dyn DeployProvider>> {
    // 1. Explicit provider arg
    if let Some(slug) = provider_arg {
        return get_provider(&slug).map_err(|e| miette!("{}", e));
    }

    // 2. Default from config
    if let Some(ref default_slug) = deploy_config.default {
        return get_provider(default_slug).map_err(|e| miette!("{}", e));
    }

    // 3. Auto-detect
    let detected = detect_all(project_dir).await.map_err(|e| miette!("{}", e))?;

    match detected.len() {
        0 => {
            if is_ci {
                return Err(miette!(
                    "No deployment provider detected or configured.\n\
                     In CI/CD, you must configure a provider in appz.json or specify one: appz deploy <provider>"
                ));
            }
            // Prompt user to select a provider
            let slug = prompt_provider_selection().await?;
            get_provider(&slug).map_err(|e| miette!("{}", e))
        }
        1 => {
            let platform = &detected[0];
            if !is_ci {
                let _ = ui::status::info(&format!(
                    "Auto-detected: {} (from {})",
                    platform.name,
                    platform.config_files.join(", ")
                ));
            }
            get_provider(&platform.slug).map_err(|e| miette!("{}", e))
        }
        _ => {
            if is_ci {
                // In CI, use the first detected (highest confidence)
                let platform = &detected[0];
                get_provider(&platform.slug).map_err(|e| miette!("{}", e))
            } else {
                // Prompt the user to choose
                let slug = prompt_detected_selection(detected).await?;
                get_provider(&slug).map_err(|e| miette!("{}", e))
            }
        }
    }
}

/// Handle prerequisite checks — install CLI tools or show auth instructions.
async fn handle_prerequisites(
    provider: Box<dyn DeployProvider>,
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    is_ci: bool,
) -> Result<Box<dyn DeployProvider>> {
    use deployer::PrerequisiteStatus;

    let status = provider
        .check_prerequisites(sandbox.clone())
        .await
        .map_err(|e| miette!("{}", e))?;

    match status {
        PrerequisiteStatus::Ready => Ok(provider),
        PrerequisiteStatus::CliMissing { tool, install_hint } => {
            if is_ci {
                Err(miette!(
                    "Required CLI tool '{}' is not installed.\nInstall it: {}",
                    tool,
                    install_hint
                ))
            } else {
                let _ = ui::status::warning(&format!(
                    "'{}' is not installed. Install it: {}",
                    tool, install_hint
                ));
                Err(miette!(
                    "Required CLI tool '{}' is not installed.",
                    tool
                ))
            }
        }
        PrerequisiteStatus::AuthMissing {
            env_var,
            login_hint,
        } => {
            if is_ci {
                Err(miette!(
                    "Authentication required. Set {}=<token> in your CI environment.",
                    env_var
                ))
            } else {
                let _ = ui::status::warning(&format!(
                    "Authentication required. Run: {}",
                    login_hint
                ));
                // In interactive mode, let the provider's deploy handle login
                Ok(provider)
            }
        }
        PrerequisiteStatus::Multiple(issues) => {
            for issue in issues {
                if let PrerequisiteStatus::CliMissing { tool, install_hint } = issue {
                    let _ = ui::status::warning(&format!("Missing: {} ({})", tool, install_hint));
                }
            }
            Err(miette!("Multiple prerequisites are missing."))
        }
    }
}

/// Run the build step before deploying.
/// When project_dir override is provided (e.g. from --project-path), use it as working_dir for the build.
async fn run_build_step(
    session: AppzSession,
    project_dir: std::path::PathBuf,
    json_output: bool,
) -> Result<()> {
    if !json_output {
        let _ = ui::status::info("Building project before deployment...");
    }

    let mut build_session = session.clone();
    build_session.working_dir = project_dir;
    crate::commands::build(build_session).await?;

    if !json_output {
        let _ = ui::status::success("Build completed.");
    }

    Ok(())
}

/// Run a deploy lifecycle hook via the sandbox.
async fn run_hook(
    name: String,
    command: String,
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        let _ = ui::status::info(&format!("Running {} hook: {}", name, command));
    }

    let status = sandbox
        .exec_interactive(&command)
        .await
        .map_err(|e| miette!("Failed to run {} hook: {}", name, e))?;

    if !status.success() {
        return Err(miette!(
            "Deploy hook '{}' failed with exit code: {:?}",
            name,
            status.code()
        ));
    }

    Ok(())
}

/// Deploy to all configured targets in parallel.
async fn deploy_to_all_targets(
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    output_dir: String,
    deploy_config: DeployConfig,
    preview: bool,
    _no_build: bool,
    dry_run: bool,
    json_output: bool,
    _is_ci: bool,
) -> AppResult {
    let target_slugs = deploy_config.target_slugs();

    if target_slugs.is_empty() {
        return Err(miette!(
            "No deploy targets configured in appz.json.\n\
             Add targets with 'appz deploy --init' first."
        ));
    }

    if !json_output {
        let _ = ui::layout::section_title(&format!(
            "Deploying to {} targets: {}",
            target_slugs.len(),
            target_slugs.join(", ")
        ));
    }

    let mut handles = Vec::new();
    let mut results: Vec<Result<DeployOutput, String>> = Vec::new();

    for slug in &target_slugs {
        let slug_owned = slug.clone();
        let provider = match get_provider(slug) {
            Ok(p) => p,
            Err(e) => {
                results.push(Err(format!("{}: {}", slug, e)));
                continue;
            }
        };

        let env_vars = deploy_config.env_for(if preview { "preview" } else { "production" });
        let ctx = DeployContext::new(sandbox.clone(), output_dir.clone())
            .with_preview(preview)
            .with_config(deploy_config.clone())
            .with_env(env_vars)
            .with_dry_run(dry_run)
            .with_json_output(json_output);

        handles.push(async move {
            let result = if preview {
                provider.deploy_preview(ctx).await
            } else {
                provider.deploy(ctx).await
            };
            (slug_owned, result)
        });
    }

    // Run all deploys concurrently
    let deploy_results = futures::future::join_all(handles).await;

    let mut all_outputs = Vec::new();
    let mut had_errors = false;

    for (slug, result) in deploy_results {
        match result {
            Ok(output) => {
                all_outputs.push(output.clone());
                if !json_output {
                    let _ = ui::status::success(&format!(
                        "{}: {} ({})",
                        slug, output.url, output.status
                    ));
                }
            }
            Err(e) => {
                had_errors = true;
                if !json_output {
                    let _ = ui::status::error(&format!("{}: {}", slug, e));
                }
            }
        }
    }

    if json_output {
        let json = serde_json::to_string_pretty(&all_outputs)
            .map_err(|e| miette!("JSON serialization failed: {}", e))?;
        println!("{}", json);
    }

    if had_errors {
        Err(miette!("Some deployments failed. See errors above."))
    } else {
        Ok(None)
    }
}

/// Display the deployment result.
/// When `phased_output_done` is true (Appz phased UI), skip success message and URL since already printed.
fn display_deploy_result(
    output: &DeployOutput,
    json_output: bool,
    phased_output_done: bool,
) -> Result<()> {
    if json_output {
        let json = serde_json::to_string_pretty(output)
            .map_err(|e| miette!("JSON serialization failed: {}", e))?;
        println!("{}", json);
    } else {
        // Vercel: use stderr when phased so output appears incrementally with spinner
        let out: &mut dyn Write = if phased_output_done {
            &mut std::io::stderr()
        } else {
            &mut std::io::stdout()
        };

        let _ = writeln!(out);
        let _ = out.flush();

        if !phased_output_done {
            let deploy_type = if output.is_preview {
                "Preview"
            } else {
                "Production"
            };

            let _ = ui::status::success(&format!(
                "{} deployment to {} is {}!",
                deploy_type, output.provider, output.status
            ));

            let _ = writeln!(out);
            let _ = writeln!(out, "  URL: {}", output.url);

            for url in &output.additional_urls {
                let _ = writeln!(out, "  Alias: {}", url);
            }
        }

        if let Some(id) = &output.deployment_id {
            let _ = writeln!(out, "  Deployment ID: {}", id);
        }

        if let Some(duration) = output.duration_ms {
            let secs = duration as f64 / 1000.0;
            let _ = writeln!(out, "  Duration: {:.1}s", secs);
        }

        let _ = writeln!(out);
        let _ = out.flush();
    }

    Ok(())
}

/// Write deployment output to CI systems (GitHub Actions, etc.).
fn write_ci_output(output: &DeployOutput) {
    if let Some(ci_platform) = deployer::config::detect_ci_platform() {
        if ci_platform == deployer::CiPlatform::GitHubActions {
            // Write to GITHUB_OUTPUT
            if let Ok(output_file) = std::env::var("GITHUB_OUTPUT") {
                let content = format!(
                    "deploy_url={}\ndeploy_status={}\ndeploy_provider={}\n",
                    output.url, output.status, output.provider
                );
                let _ = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&output_file)
                    .and_then(|mut f| {
                        use std::io::Write;
                        f.write_all(content.as_bytes())
                    });
            }

            // Write to GITHUB_STEP_SUMMARY
            if let Ok(summary_file) = std::env::var("GITHUB_STEP_SUMMARY") {
                let deploy_type = if output.is_preview { "Preview" } else { "Production" };
                let content = format!(
                    "### {} Deployment\n\n\
                     | Field | Value |\n\
                     |-------|-------|\n\
                     | Provider | {} |\n\
                     | URL | [{}]({}) |\n\
                     | Status | {} |\n\n",
                    deploy_type,
                    output.provider,
                    output.url,
                    output.url,
                    output.status,
                );
                let _ = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&summary_file)
                    .and_then(|mut f| {
                        use std::io::Write;
                        f.write_all(content.as_bytes())
                    });
            }
        }
    }
}

/// Resolve the build output directory from project path and config.
///
/// Prefers `.appz/output` (Build Output v3) when `config.json` exists,
/// else uses `appz.json` outputDirectory or common fallbacks.
pub fn resolve_output_dir(project_dir: &std::path::Path) -> String {
    // Prefer standardized build output when present
    let appz_output = project_dir.join(".appz/output");
    if appz_output.join("config.json").exists() {
        return ".appz/output".to_string();
    }

    // Check for static export output (from appz wp-export)
    let static_output = project_dir.join(".appz/output/static");
    if static_output.is_dir() && std::fs::read_dir(&static_output).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return ".appz/output/static".to_string();
    }

    // Try to read outputDirectory from appz.json
    if let Ok(root) =
        starbase_utils::json::read_file::<serde_json::Value>(project_dir.join("appz.json"))
    {
        if let Some(dir) = root.get("outputDirectory").and_then(|v| v.as_str()) {
            return dir.to_string();
        }
    }

    // Default to common output directories
    for dir in &["dist", "build", "out", "public", "_site"] {
        if project_dir.join(dir).exists() {
            return dir.to_string();
        }
    }

    "dist".to_string()
}

/// Prompt the user to select a provider from the full list.
async fn prompt_provider_selection() -> Result<String> {
    let registry = create_provider_registry();
    let options: Vec<String> = registry
        .iter()
        .map(|p| format!("{} ({})", p.name(), p.slug()))
        .collect();

    let selection = ui::select_interactive("Select a deployment provider:", &options)
        .map_err(|e| miette!("Selection failed: {}", e))?
        .ok_or_else(|| Report::from(UserCancellation::selection()))?;

    // Extract slug from "Name (slug)" format
    let slug = selection
        .split('(')
        .nth(1)
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(&selection)
        .to_string();

    Ok(slug)
}

/// Prompt the user to select from detected platforms.
async fn prompt_detected_selection(detected: Vec<DetectedPlatform>) -> Result<String> {
    let options: Vec<String> = detected
        .iter()
        .map(|p| format!("{} (detected: {})", p.name, p.config_files.join(", ")))
        .collect();

    let selection = ui::select_interactive("Multiple platforms detected. Select one:", &options)
        .map_err(|e| miette!("Selection failed: {}", e))?
        .ok_or_else(|| Report::from(UserCancellation::selection()))?;

    // Match back to slug
    for (i, option) in detected.iter().enumerate() {
        let expected = format!(
            "{} (detected: {})",
            option.name,
            option.config_files.join(", ")
        );
        if selection == expected {
            return Ok(detected[i].slug.clone());
        }
    }

    Err(miette!("Invalid selection"))
}
