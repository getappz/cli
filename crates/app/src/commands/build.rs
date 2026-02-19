use crate::commands::install_helpers::{
    get_default_install_command, handle_shell_script_fallback, run_recipe_task,
};
use detectors::{
    detect_framework_record, DetectFrameworkRecordOptions, DetectorFilesystem, StdFilesystem,
};
use crate::sandbox_helpers::mise_tools_for_execution;
use crate::session::AppzSession;
use crate::shell::{command_exists, is_shell_script, run_local_with, RunOptions};
use sandbox::{create_sandbox, SandboxConfig};
use frameworks::frameworks;
use starbase::AppResult;
use std::sync::Arc;
use task::Context;
use tracing::instrument;

/// Build the project.
///
/// Detects the framework, installs dependencies, and runs the build command.
#[instrument(skip_all)]
pub async fn build(session: AppzSession) -> AppResult {
    // Use the working directory from session (already respects --cwd)
    let project_path = session.working_dir.clone();

    // Check if path exists
    if !project_path.exists() {
        return Err(miette::miette!(
            "Path does not exist: {}",
            project_path.display()
        ));
    }

    if !project_path.is_dir() {
        return Err(miette::miette!(
            "Path is not a directory: {}",
            project_path.display()
        ));
    }

    // Get task registry for checking recipe tasks
    let registry = session.get_task_registry();

    // Create filesystem detector
    let fs: Arc<dyn DetectorFilesystem> =
        Arc::new(StdFilesystem::new(Some(project_path.clone())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    match detect_framework_record(options).await {
        Ok(Some((framework, _version, package_manager))) => {
            // Get scripts from package_manager (already extracted by detect_framework_record)
            let user_install_script = package_manager
                .as_ref()
                .and_then(|pm| pm.install_script.clone());
            let user_build_script = package_manager
                .as_ref()
                .and_then(|pm| pm.build_script.clone());

            // Get framework install command (fallback)
            let framework_install_cmd = framework
                .settings
                .as_ref()
                .and_then(|s| s.install_command.as_ref())
                .and_then(|c| c.value)
                .map(|s| s.to_string());

            // Get framework build command (fallback)
            let framework_build_cmd = framework
                .settings
                .as_ref()
                .and_then(|s| s.build_command.as_ref())
                .and_then(|c| c.value)
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    miette::miette!(
                        "No build command configured for framework: {}",
                        framework.name
                    )
                })?;

            // Determine install command (priority: user script > framework command > package manager default)
            let install_cmd = if let Some(ref user_install) = user_install_script {
                user_install.clone()
            } else if let Some(ref framework_install) = framework_install_cmd {
                framework_install.clone()
            } else {
                get_default_install_command(&package_manager)
            };

            // Determine build command (priority: user script > framework command)
            let build_cmd = if let Some(ref user_build) = user_build_script {
                user_build.clone()
            } else {
                framework_build_cmd.clone()
            };

            println!("✓ Detected framework: {}", framework.name);

            // Handle ddev setup for Jigsaw
            if framework.slug == Some("jigsaw") {
                if !command_exists("ddev") {
                    return Err(miette::miette!(
                        "ddev is required for Jigsaw projects but was not found. Please install ddev first."
                    ));
                }

                let ddev_config_path = project_path.join(".ddev").join("config.yaml");
                if !ddev_config_path.exists() {
                    println!("⚙️  Configuring ddev for Jigsaw project...");
                    let mut ctx_config = Context::new();
                    ctx_config.set_working_path(project_path.clone());
                    let config_opts = RunOptions {
                        cwd: Some(project_path.clone()),
                        env: None,
                        show_output: true,
                        package_manager: None,
                        tool_info: None,
                    };
                    run_local_with(
                        &ctx_config,
                        "ddev config --project-type=php --php-version=8.2",
                        config_opts,
                    )
                    .await?;
                    println!("✓ ddev configured");
                }

                // Ensure ddev is started
                println!("🚀 Starting ddev...");
                let mut ctx_start = Context::new();
                ctx_start.set_working_path(project_path.clone());
                let start_opts = RunOptions {
                    cwd: Some(project_path.clone()),
                    env: None,
                    show_output: false, // ddev start can be verbose
                    package_manager: None,
                    tool_info: None,
                };
                // ddev start is idempotent, so it's safe to run even if already started
                let _ = run_local_with(&ctx_start, "ddev start", start_opts).await;
                println!("✓ ddev ready");
            }

            // Display which install command is being used
            if user_install_script.is_some() {
                println!("✓ Using user-defined install script from package.json");
            } else if framework_install_cmd.is_some() {
                println!("✓ Using framework install command");
            } else {
                println!("✓ Using package manager default install command");
            }

            // Display which build command is being used
            if user_build_script.is_some() {
                println!("✓ Using user-defined build script from package.json");
            } else {
                println!("✓ Using framework build command");
            }

            // Create a minimal context for fallback (run_local_with)
            let mut ctx = Context::new();
            ctx.set_working_path(project_path.clone());
            let opts = RunOptions {
                cwd: Some(project_path.clone()),
                env: None,
                show_output: true,
                package_manager: package_manager.clone(),
                tool_info: None,
            };

            // Sandbox for install/build (mise-managed, node_modules/.bin in PATH)
            let config = SandboxConfig::new(project_path.clone())
                .with_settings(mise_tools_for_execution(&package_manager, None));
            let sandbox = create_sandbox(config).await.ok();
            if sandbox.is_none() {
                tracing::debug!(
                    "Sandbox setup failed, will use run_local_with fallback for install/build"
                );
            }

            // Execute install step: check for appz:install recipe task first
            let using_appz_install = registry.get("appz:install").is_some();

            if using_appz_install {
                println!("✓ Found appz:install recipe task, using recipe install...");
                println!("\n📦 Running install command...");
                run_recipe_task(
                    &registry,
                    "appz:install",
                    project_path.clone(),
                    session.cli.verbose,
                )
                .await?;
            } else {
                // Execute framework install command via sandbox (with fallback)
                println!("\n📦 Running install command...");
                let install_result = if let Some(ref s) = sandbox {
                    s.exec_interactive(&install_cmd)
                        .await
                        .map(|_| ())
                        .map_err(|e| miette::miette!("{}", e))
                } else {
                    run_local_with(&ctx, &install_cmd, opts.clone()).await
                };

                // Handle shell script fallback on Windows
                handle_shell_script_fallback(
                    install_result,
                    is_shell_script(&install_cmd),
                    user_install_script.is_some(),
                    framework_install_cmd.as_ref(),
                    Some(get_default_install_command(&package_manager)),
                    &ctx,
                    &opts,
                )
                .await?;
            }

            // Execute build step: check for appz:build recipe task first
            let using_appz_build = registry.get("appz:build").is_some();

            if using_appz_build {
                println!("✓ Found appz:build recipe task, using recipe build...");
                println!("\n🔨 Running build command...");
                run_recipe_task(
                    &registry,
                    "appz:build",
                    project_path.clone(),
                    session.cli.verbose,
                )
                .await?;
            } else {
                // Execute framework build command via sandbox (with fallback)
                println!("\n🔨 Running build command...");
                let build_result = if let Some(ref s) = sandbox {
                    s.exec_interactive(&build_cmd)
                        .await
                        .map(|_| ())
                        .map_err(|e| miette::miette!("{}", e))
                } else {
                    run_local_with(&ctx, &build_cmd, opts.clone()).await
                };

                // Handle shell script fallback on Windows
                handle_shell_script_fallback(
                    build_result,
                    is_shell_script(&build_cmd),
                    user_build_script.is_some(),
                    Some(&framework_build_cmd),
                    None,
                    &ctx,
                    &opts,
                )
                .await?;
            }

            println!("\n✓ Build completed successfully!");
        }
        Ok(None) => {
            println!("✗ No framework detected in {}", project_path.display());
            println!("\nSupported frameworks:");
            for fw in frameworks() {
                if let Some(slug) = fw.slug {
                    println!("  - {} ({})", fw.name, slug);
                } else {
                    println!("  - {}", fw.name);
                }
            }
        }
        Err(e) => {
            return Err(miette::miette!("Error detecting framework: {}", e));
        }
    }

    Ok(None)
}
