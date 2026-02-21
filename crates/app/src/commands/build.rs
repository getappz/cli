use appz_build::{detect_framework, produce_standardized_output, resolve_build_output_dir, validate_output_dir};
use crate::commands::install_helpers::{
    get_default_install_command, handle_shell_script_fallback, run_recipe_task,
};
use crate::sandbox_helpers::mise_tools_for_execution;
use crate::session::AppzSession;
use crate::shell::{command_exists, is_shell_script, run_local_with, RunOptions};
use frameworks::frameworks;
use sandbox::{create_sandbox, SandboxConfig};
use starbase::AppResult;
use task::Context;
use tracing::instrument;

/// Build the project.
///
/// Detects the framework, installs dependencies, and runs the build command.
/// Produces standardized output at `.appz/output/` when successful.
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

    // Detect framework via appz-build
    let detected = match detect_framework(&project_path).await {
        Ok(Some(d)) => d,
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
            return Ok(None);
        }
        Err(e) => return Err(miette::miette!("Error detecting framework: {}", e)),
    };

    println!("✓ Detected framework: {}", detected.name);

    // Handle ddev setup for Jigsaw
    if detected.slug.as_deref() == Some("jigsaw") {
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

    // Create a minimal context for fallback (run_local_with)
    let mut ctx = Context::new();
    ctx.set_working_path(project_path.clone());
    let opts = RunOptions {
        cwd: Some(project_path.clone()),
        env: None,
        show_output: true,
        package_manager: detected.package_manager.clone(),
        tool_info: None,
    };

    // Sandbox for install/build (mise-managed, node_modules/.bin in PATH)
    let config = SandboxConfig::new(project_path.clone())
        .with_settings(mise_tools_for_execution(&detected.package_manager, None));
    let sandbox = create_sandbox(config).await.ok();
    if sandbox.is_none() {
        tracing::debug!(
            "Sandbox setup failed, will use run_local_with fallback for install/build"
        );
    }

    let has_user_install = detected
        .package_manager
        .as_ref()
        .and_then(|pm| pm.install_script.as_ref())
        .is_some();
    let has_user_build = detected
        .package_manager
        .as_ref()
        .and_then(|pm| pm.build_script.as_ref())
        .is_some();

    // Display which commands are being used
    if has_user_install {
        println!("✓ Using user-defined install script from package.json");
    } else {
        println!("✓ Using framework or package manager install command");
    }
    if has_user_build {
        println!("✓ Using user-defined build script from package.json");
    } else {
        println!("✓ Using framework build command");
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
            s.exec_interactive(&detected.install_command)
                .await
                .map(|_| ())
                .map_err(|e| miette::miette!("{}", e))
        } else {
            run_local_with(&ctx, &detected.install_command, opts.clone()).await
        };

        handle_shell_script_fallback(
            install_result,
            is_shell_script(&detected.install_command),
            has_user_install,
            None,
            Some(get_default_install_command(&detected.package_manager)),
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
            s.exec_interactive(&detected.build_command)
                .await
                .map(|_| ())
                .map_err(|e| miette::miette!("{}", e))
        } else {
            run_local_with(&ctx, &detected.build_command, opts.clone()).await
        };

        handle_shell_script_fallback(
            build_result,
            is_shell_script(&detected.build_command),
            has_user_build,
            Some(&detected.build_command),
            None,
            &ctx,
            &opts,
        )
        .await?;
    }

    println!("\n✓ Build completed successfully!");

    // Produce standardized output (.appz/output) for deployment
    let build_output_path =
        resolve_build_output_dir(&project_path, &detected.output_directory);
    if let Err(e) = validate_output_dir(&build_output_path) {
        tracing::debug!(
            "Skipping .appz/output: build output validation failed: {}",
            e
        );
    } else if let Err(e) =
        produce_standardized_output(&project_path, &build_output_path)
    {
        tracing::debug!("Skipping .appz/output: {}", e);
    } else {
        println!("✓ Produced standardized output at .appz/output/");
    }

    Ok(None)
}
