use appz_build::{detect_framework, produce_standardized_output, resolve_build_output_dir, validate_output_dir};
use blueprint::WordPressRuntime;
use crate::commands::install_helpers::{
    get_default_install_command, handle_shell_script_fallback, run_recipe_task,
};
use crate::sandbox_helpers::mise_tools_for_execution;
use crate::session::AppzSession;
use crate::ddev_helpers::{
    ddev_config_command, ddev_project_type_for_framework, ddev_web_container_name,
    has_ddev_config, is_ddev_available, is_ddev_supported_framework,
};
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

    // Handle DDEV setup for supported PHP/CMS frameworks
    if let Some(slug) = detected.slug.as_deref() {
        if is_ddev_supported_framework(slug) {
            if !is_ddev_available() {
                return Err(miette::miette!(
                    "DDEV is required for {} projects but was not found. \
                     Install it: https://docs.ddev.com/en/stable/users/install/ddev-installation/",
                    detected.name
                ));
            }

            if !has_ddev_config(&project_path) {
                if let Some((project_type, docroot)) =
                    ddev_project_type_for_framework(slug)
                {
                    let mut config_cmd =
                        ddev_config_command(project_type, docroot);
                    if project_type == "php" {
                        config_cmd.push_str(" --php-version=8.2");
                    }
                    println!("⚙️  Configuring DDEV for {}...", detected.name);
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
                        &config_cmd,
                        config_opts,
                    )
                    .await?;
                    println!("✓ DDEV configured");
                }
            }

            println!("🚀 Starting DDEV...");
            let ddev_runtime = blueprint::DdevRuntime::new();
            ddev_runtime.start(&project_path)
                .map_err(|e| miette::miette!("{}", e))?;
            println!("✓ DDEV ready");
        }
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

    // Execute install step: skip for WordPress DDEV (Like dev command)
    let skip_install_for_wordpress_ddev = detected
        .slug
        .as_deref()
        .is_some_and(|s| s == "wordpress" && has_ddev_config(&project_path));

    let using_appz_install = registry.get("appz:install").is_some();

    if skip_install_for_wordpress_ddev {
        println!("✓ Skipping install for WordPress DDEV project");
    } else if using_appz_install {
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

    // Execute build step
    let is_wordpress = detected.slug.as_deref() == Some("wordpress");
    let using_appz_build = registry.get("appz:build").is_some();

    if is_wordpress {
        // WordPress: use StaticExporter to export to dist/
        let runtime = crate::wp_runtime::resolve(&project_path, false)?;
        let host_output = project_path.join("dist");
        std::fs::create_dir_all(&host_output)
            .map_err(|e| miette::miette!("Failed to create dist/: {}", e))?;

        let spinner = ui::progress::spinner("Discovering sitemap...");
        let spinner_ref = std::sync::Arc::new(spinner);
        let spinner_for_cb = spinner_ref.clone();

        let on_progress: std::sync::Arc<dyn Fn(blueprint::ProgressEvent) + Send + Sync> =
            std::sync::Arc::new(move |event| {
                match event {
                    blueprint::ProgressEvent::DiscoveringSitemap => {
                        spinner_for_cb.set_message("Discovering sitemap...");
                    }
                    blueprint::ProgressEvent::SitemapDone { urls_found } => {
                        if urls_found > 0 {
                            spinner_for_cb.set_message(&format!("Found {} URLs in sitemap, crawling...", urls_found));
                        } else {
                            spinner_for_cb.set_message("Crawling site...");
                        }
                    }
                    blueprint::ProgressEvent::Crawling { pages, assets } => {
                        spinner_for_cb.set_message(&format!(
                            "Exported {} pages, {} assets", pages, assets
                        ));
                    }
                    blueprint::ProgressEvent::Done { pages, assets, duration } => {
                        spinner_for_cb.finish_with_message(&format!(
                            "Exported {} pages, {} assets in {:.1}s",
                            pages, assets, duration.as_secs_f64()
                        ));
                    }
                }
            });

        let exporter = blueprint::StaticExporter::new(project_path.clone(), runtime.clone());
        let output_path = host_output.clone();
        // Run in spawn_blocking because site2static uses reqwest::blocking
        // which creates its own tokio runtime — cannot be used inside an
        // existing async runtime without spawn_blocking.
        let export_result = tokio::task::spawn_blocking(move || {
            exporter.export(Some(output_path.as_path()), Some(on_progress))
        })
        .await
        .map_err(|e| miette::miette!("Static export task panicked: {}", e))?
        .map_err(|e| miette::miette!("Static export failed: {}", e))?;

        // Ensure spinner is finished (in case no Done event was emitted).
        drop(spinner_ref);

        // For DDEV bind-mount: files should already be on host.
        // For mutagen: need docker cp.
        if runtime.slug() == "ddev" {
            let has_files = std::fs::read_dir(&host_output)
                .map(|mut d| d.next().is_some())
                .unwrap_or(false);
            if !has_files {
                if let Some(container) = ddev_web_container_name(&project_path) {
                    let copy_cmd = format!(
                        "docker cp {}:/var/www/html/dist/. {}",
                        container,
                        host_output.display()
                    );
                    let mut ctx_cp = Context::new();
                    ctx_cp.set_working_path(project_path.clone());
                    let copy_opts = RunOptions {
                        cwd: Some(project_path.clone()),
                        env: None,
                        show_output: false,
                        package_manager: None,
                        tool_info: None,
                    };
                    run_local_with(&ctx_cp, &copy_cmd, copy_opts).await?;
                    println!("✓ Synced dist/ from DDEV container");
                }
            }
        }
    } else if using_appz_build {
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
        let use_local_for_ddev = detected.build_command.starts_with("ddev ");
        let build_result = if use_local_for_ddev {
            run_local_with(&ctx, &detected.build_command, opts.clone()).await
        } else if let Some(ref s) = sandbox {
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

    // Produce standardized output (.appz/output) for non-WordPress frameworks
    if !is_wordpress {
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
    }

    Ok(None)
}
