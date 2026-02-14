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

use std::sync::Arc;

use deployer::{
    self, detect_all, get_provider, is_ci_environment, read_deploy_config, write_deploy_config,
    create_provider_registry, DeployConfig, DeployContext, DeployOutput,
    DeployProvider, DetectedPlatform, SetupContext,
};
use miette::{miette, Result};
use starbase::AppResult;

use crate::session::AppzSession;

// ---------------------------------------------------------------------------
// Sandbox creation helper
// ---------------------------------------------------------------------------

/// Create and initialise a sandbox for the project directory.
///
/// The sandbox provides command execution (via mise), scoped filesystem
/// access, and tool management. All provider operations go through it
/// instead of raw `tokio::process::Command`.
async fn create_deploy_sandbox(
    project_dir: &std::path::Path,
) -> Result<Arc<dyn sandbox::SandboxProvider>> {
    let config = sandbox::SandboxConfig::new(project_dir)
        .with_settings(sandbox::SandboxSettings::default().quiet());

    let sb = sandbox::create_sandbox(config)
        .await
        .map_err(|e| miette!("Failed to create deploy sandbox: {}", e))?;

    Ok(Arc::from(sb))
}

/// Main deploy command entry point.
pub async fn deploy(
    session: AppzSession,
    provider_arg: Option<String>,
    preview: bool,
    no_build: bool,
    dry_run: bool,
    json_output: bool,
    deploy_all: bool,
    yes: bool,
) -> AppResult {
    let project_dir = session.working_dir.clone();
    let is_ci = is_ci_environment() || yes;

    // 1. Load existing deploy config
    let deploy_config = read_deploy_config(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    // 2. Determine output directory
    let output_dir = resolve_output_dir(&session);

    // 3. Create sandbox for command execution
    let sandbox = create_deploy_sandbox(&project_dir).await?;

    // 4. Handle --all flag
    if deploy_all {
        return deploy_to_all_targets(
            sandbox.clone(),
            &output_dir,
            &deploy_config,
            preview,
            no_build,
            dry_run,
            json_output,
            is_ci,
        )
        .await;
    }

    // 5. Resolve the provider
    let provider = resolve_provider(
        provider_arg.as_deref(),
        &project_dir,
        &deploy_config,
        is_ci,
    )
    .await?;

    if !json_output {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title(&format!("Deploying to {}", provider.name()));
    }

    // 6. Check prerequisites via sandbox
    handle_prerequisites(&*provider, &*sandbox, is_ci).await?;

    // 7. Build before deploy (unless --no-build)
    if !no_build && !dry_run {
        run_build_step(&session, json_output).await?;
    }

    // 8. Run before_deploy hook via sandbox
    if let Some(ref hooks) = deploy_config.hooks {
        if let Some(ref before) = hooks.before_deploy {
            run_hook("before_deploy", before, &sandbox, json_output).await?;
        }
    }

    // 9. Build deploy context with sandbox
    let env_vars = deploy_config.env_for(if preview { "preview" } else { "production" });
    let ctx = DeployContext::new(sandbox.clone(), output_dir)
        .with_preview(preview)
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
    let deploy_label = if preview {
        format!("{} (preview)", provider.name())
    } else {
        format!("{} (production)", provider.name())
    };

    let result = if !json_output && !dry_run {
        let sp = ui::progress::spinner(&format!("Deploying to {}...", deploy_label));
        let r = if preview {
            provider.deploy_preview(&ctx).await
        } else {
            provider.deploy(&ctx).await
        };
        let msg = if r.is_ok() { "Deployed!" } else { "Failed" };
        sp.finish_with_message(msg);
        r
    } else {
        if preview {
            provider.deploy_preview(&ctx).await
        } else {
            provider.deploy(&ctx).await
        }
    };

    let output = result.map_err(|e| miette!("{}", e))?;

    // 12. Run after_deploy hook via sandbox
    if let Some(ref hooks) = deploy_config.hooks {
        if let Some(ref after) = hooks.after_deploy {
            run_hook("after_deploy", after, &sandbox, json_output).await?;
        }
    }

    // 13. Display results
    display_deploy_result(&output, json_output)?;

    // 14. Write GitHub Actions output if applicable
    write_ci_output(&output);

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

    let output_dir = resolve_output_dir(&session);

    // Create sandbox for setup operations
    let sandbox = create_deploy_sandbox(&project_dir).await?;

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

    let output_dir = resolve_output_dir(&session);

    let provider_slug = match provider_arg {
        Some(slug) => slug,
        None => deploy_config
            .default
            .clone()
            .ok_or_else(|| miette!("No default provider configured. Specify a provider: appz deploy-list <provider>"))?,
    };

    let provider = get_provider(&provider_slug).map_err(|e| miette!("{}", e))?;

    // Create sandbox for the list operation
    let sandbox = create_deploy_sandbox(&project_dir).await?;

    let ctx = DeployContext::new(sandbox, output_dir)
        .with_config(deploy_config);

    let deployments = provider
        .list_deployments(&ctx)
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
    provider_arg: Option<&str>,
    project_dir: &std::path::Path,
    deploy_config: &DeployConfig,
    is_ci: bool,
) -> Result<Box<dyn DeployProvider>> {
    // 1. Explicit provider arg
    if let Some(slug) = provider_arg {
        return get_provider(slug).map_err(|e| miette!("{}", e));
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
                let slug = prompt_detected_selection(&detected).await?;
                get_provider(&slug).map_err(|e| miette!("{}", e))
            }
        }
    }
}

/// Handle prerequisite checks — install CLI tools or show auth instructions.
async fn handle_prerequisites(
    provider: &dyn DeployProvider,
    sandbox: &dyn sandbox::SandboxProvider,
    is_ci: bool,
) -> Result<()> {
    use deployer::PrerequisiteStatus;

    let status = provider
        .check_prerequisites(sandbox)
        .await
        .map_err(|e| miette!("{}", e))?;

    match status {
        PrerequisiteStatus::Ready => Ok(()),
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
                Ok(())
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
async fn run_build_step(session: &AppzSession, json_output: bool) -> Result<()> {
    if !json_output {
        let _ = ui::status::info("Building project before deployment...");
    }

    // Build takes AppzSession by value, so clone it
    let build_session = session.clone();
    crate::commands::build(build_session).await?;

    if !json_output {
        let _ = ui::status::success("Build completed.");
    }

    Ok(())
}

/// Run a deploy lifecycle hook via the sandbox.
async fn run_hook(
    name: &str,
    command: &str,
    sandbox: &Arc<dyn sandbox::SandboxProvider>,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        let _ = ui::status::info(&format!("Running {} hook: {}", name, command));
    }

    let status = sandbox
        .exec_interactive(command)
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
    output_dir: &str,
    deploy_config: &DeployConfig,
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
             Add targets with 'appz deploy-init <provider>' first."
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
        let provider = match get_provider(slug) {
            Ok(p) => p,
            Err(e) => {
                results.push(Err(format!("{}: {}", slug, e)));
                continue;
            }
        };

        let env_vars = deploy_config.env_for(if preview { "preview" } else { "production" });
        let ctx = DeployContext::new(sandbox.clone(), output_dir.to_string())
            .with_preview(preview)
            .with_config(deploy_config.clone())
            .with_env(env_vars)
            .with_dry_run(dry_run)
            .with_json_output(json_output);

        handles.push(async move {
            let result = if preview {
                provider.deploy_preview(&ctx).await
            } else {
                provider.deploy(&ctx).await
            };
            (slug.clone(), result)
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
fn display_deploy_result(output: &DeployOutput, json_output: bool) -> Result<()> {
    if json_output {
        let json = serde_json::to_string_pretty(output)
            .map_err(|e| miette!("JSON serialization failed: {}", e))?;
        println!("{}", json);
    } else {
        let _ = ui::layout::blank_line();

        let deploy_type = if output.is_preview {
            "Preview"
        } else {
            "Production"
        };

        let _ = ui::status::success(&format!(
            "{} deployment to {} is {}!",
            deploy_type, output.provider, output.status
        ));

        println!();
        println!("  URL: {}", output.url);

        for url in &output.additional_urls {
            println!("  Alias: {}", url);
        }

        if let Some(id) = &output.deployment_id {
            println!("  Deployment ID: {}", id);
        }

        if let Some(duration) = output.duration_ms {
            let secs = duration as f64 / 1000.0;
            println!("  Duration: {:.1}s", secs);
        }

        let _ = ui::layout::blank_line();
    }

    Ok(())
}

/// Write deployment output to CI systems (GitHub Actions, etc.).
fn write_ci_output(output: &DeployOutput) {
    if let Some(ci_platform) = deployer::config::detect_ci_platform() {
        match ci_platform {
            deployer::CiPlatform::GitHubActions => {
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
            _ => {}
        }
    }
}

/// Resolve the build output directory from session and config.
fn resolve_output_dir(session: &AppzSession) -> String {
    // Try to read outputDirectory from appz.json
    if let Ok(content) = std::fs::read_to_string(session.working_dir.join("appz.json")) {
        if let Ok(root) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(dir) = root.get("outputDirectory").and_then(|v| v.as_str()) {
                return dir.to_string();
            }
        }
    }

    // Default to common output directories
    for dir in &["dist", "build", "out", "public", "_site"] {
        if session.working_dir.join(dir).exists() {
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

    let selection = inquire::Select::new("Select a deployment provider:", options)
        .prompt()
        .map_err(|e| miette!("Selection cancelled: {}", e))?;

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
async fn prompt_detected_selection(detected: &[DetectedPlatform]) -> Result<String> {
    let options: Vec<String> = detected
        .iter()
        .map(|p| format!("{} (detected: {})", p.name, p.config_files.join(", ")))
        .collect();

    let selection = inquire::Select::new("Multiple platforms detected. Select one:", options)
        .prompt()
        .map_err(|e| miette!("Selection cancelled: {}", e))?;

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
