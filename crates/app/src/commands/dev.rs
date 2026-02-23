use crate::args::DevSubcommand;
use crate::commands::install_helpers::{
    get_default_install_command, handle_shell_script_fallback, run_recipe_task,
};
use crate::ddev_helpers::{
    ddev_config_command, ddev_project_type_for_framework, has_ddev_config,
    is_ddev_available, is_ddev_supported_framework,
};
use detectors::{
    detect_framework_record, detect_hugo_info, DetectFrameworkRecordOptions, DetectorFilesystem,
    StdFilesystem,
};
use crate::sandbox_helpers::mise_tools_for_execution;
use crate::session::AppzSession;
use crate::shell::{command_exists, is_shell_script, run_local_with, RunOptions, ToolVersionInfo};
use crate::tunnel::{CloudflaredTunnel, TunnelService};
use sandbox::{create_sandbox, SandboxConfig};
use frameworks::frameworks;
use starbase::AppResult;
use std::path::Path;
use std::sync::Arc;
use task::Context;
use tracing::instrument;

/// Stop DDEV containers for a DDEV project.
async fn dev_stop(project_path: &Path) -> AppResult {
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
    if !has_ddev_config(project_path) {
        return Err(miette::miette!(
            "No DDEV project in {}. Stop is only for DDEV projects (WordPress, Drupal, Laravel, etc.).",
            project_path.display()
        ));
    }
    if !is_ddev_available() {
        return Err(miette::miette!(
            "DDEV is required but was not found. Install it: https://docs.ddev.com/en/stable/users/install/ddev-installation/"
        ));
    }

    let mut ctx = Context::new();
    ctx.set_working_path(project_path.to_path_buf());
    let opts = RunOptions {
        cwd: Some(project_path.to_path_buf()),
        env: None,
        show_output: true,
        package_manager: None,
        tool_info: None,
    };
    run_local_with(&ctx, "ddev stop", opts).await?;
    println!("✓ DDEV stopped");
    Ok(None)
}

#[instrument(skip_all)]
pub async fn dev(session: AppzSession, args: crate::args::DevArgs) -> AppResult {
    let project_path = session.working_dir.clone();

    // Handle `appz dev stop` for DDEV projects
    if matches!(args.command, Some(DevSubcommand::Stop)) {
        return dev_stop(&project_path).await;
    }

    let share = args.share;
    let port = args.port.unwrap_or(3000);

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

    // If sharing, start tunnel first
    let mut tunnel: Option<CloudflaredTunnel> = if share {
        println!("🌐 Starting tunnel...");
        let mut t = CloudflaredTunnel::new();
        match t.start(port).await {
            Ok(url) => {
                println!("✓ Public URL: {}", url);
                Some(t)
            }
            Err(e) => {
                return Err(miette::miette!("Failed to start tunnel: {}", e));
            }
        }
    } else {
        None
    };

    // Create filesystem detector
    let fs: Arc<dyn DetectorFilesystem> =
        Arc::new(StdFilesystem::new(Some(project_path.clone())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    match detect_framework_record(options).await {
        Ok(Some((framework, _version, package_manager))) => {
            // Get framework install command (fallback) - extract before consuming framework.settings
            let framework_install_cmd = framework
                .settings
                .as_ref()
                .and_then(|s| s.install_command.as_ref())
                .and_then(|c| c.value.as_ref())
                .map(|s| s.to_string());

            // Get framework dev command (fallback)
            let framework_dev_cmd = framework
                .settings
                .and_then(|s| s.dev_command)
                .and_then(|d| d.value)
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    miette::miette!(
                        "No dev command configured for framework: {}",
                        framework.name
                    )
                })?;

            // Prioritize user-defined dev script from package.json over framework dev command
            let (dev_cmd, is_user_script) = if let Some(ref pm) = package_manager {
                if let Some(ref user_dev_script) = pm.dev_script {
                    // User has defined scripts.dev in package.json, use it
                    (user_dev_script.clone(), true)
                } else {
                    // No user dev script, fallback to framework dev command
                    (framework_dev_cmd.clone(), false)
                }
            } else {
                // No package manager detected, use framework dev command
                (framework_dev_cmd.clone(), false)
            };

            println!("✓ Detected framework: {}", framework.name);
            if is_user_script {
                println!("✓ Using user-defined dev script from package.json");
            } else if framework
                .slug
                .as_ref()
                .is_some_and(|s| is_ddev_supported_framework(*s))
            {
                // Will show "Using DDEV" at dev execution (after ddev config+start)
            } else {
                println!("✓ Using framework dev command");
            }

            // Handle DDEV setup for supported PHP/CMS frameworks
            // (WordPress, Drupal, Laravel, Jigsaw, Sculpin, Kirby, Statamic, etc.)
            if let Some(ref slug) = framework.slug {
                if is_ddev_supported_framework(slug) {
                    if !is_ddev_available() {
                        return Err(miette::miette!(
                            "DDEV is required for {} projects but was not found. \
                             Install it: https://docs.ddev.com/en/stable/users/install/ddev-installation/",
                            framework.name
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
                            println!("⚙️  Configuring DDEV for {}...", framework.name);
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
                    let mut ctx_start = Context::new();
                    ctx_start.set_working_path(project_path.clone());
                    let start_opts = RunOptions {
                        cwd: Some(project_path.clone()),
                        env: None,
                        show_output: false,
                        package_manager: None,
                        tool_info: None,
                    };
                    let _ = run_local_with(&ctx_start, "ddev start", start_opts).await;
                    println!("✓ DDEV ready");

                    // Verify container can reach internet (common WSL2/Docker DNS issue)
                    let mut ctx_conn = Context::new();
                    ctx_conn.set_working_path(project_path.clone());
                    let conn_opts = RunOptions {
                        cwd: Some(project_path.clone()),
                        env: None,
                        show_output: false,
                        package_manager: None,
                        tool_info: None,
                    };
                    let has_connectivity = run_local_with(
                        &ctx_conn,
                        "ddev exec curl -sSf -o /dev/null --connect-timeout 5 https://github.com",
                        conn_opts,
                    )
                    .await
                    .is_ok();
                    if !has_connectivity {
                        eprintln!();
                        eprintln!("⚠️  DDEV container has no internet access (DNS resolution failed).");
                        eprintln!("   This can cause wp core install, nvm, and other features to fail.");
                        eprintln!();
                        eprintln!("   Fix (WSL2 + Docker): add DNS to Docker daemon:");
                        eprintln!("   1. Docker Desktop → Settings → Docker Engine");
                        eprintln!("   2. Add to JSON: \"dns\": [\"8.8.8.8\", \"1.1.1.1\"]");
                        eprintln!("   3. Apply & Restart");
                        eprintln!();
                        eprintln!("   Or upgrade WSL: wsl --upgrade (WSL 2.2.1+ has DNS tunneling)");
                        eprintln!("   See docs/plans/ddev-troubleshooting.md for more options.");
                        eprintln!();
                    }

                }
            }

            // Create filesystem detector for Hugo info detection
            let fs_for_hugo = Arc::new(StdFilesystem::new(Some(project_path.clone())));

            // Detect Hugo-specific info if this is a Hugo project
            let tool_info = if framework.slug == Some("hugo") {
                let fs_dyn: Arc<dyn DetectorFilesystem> =
                    fs_for_hugo.clone();
                match detect_hugo_info(&fs_dyn).await {
                    Ok(Some(hugo_info)) => {
                        if hugo_info.extended {
                            println!(
                                "✓ Hugo extended required{}",
                                hugo_info
                                    .min_version
                                    .as_ref()
                                    .map(|v| format!(" (min: {})", v))
                                    .unwrap_or_default()
                            );
                        }
                        Some(ToolVersionInfo {
                            tool: "hugo".to_string(),
                            version: hugo_info.min_version,
                            extended: hugo_info.extended,
                        })
                    }
                    _ => Some(ToolVersionInfo {
                        tool: "hugo".to_string(),
                        version: None,
                        extended: false,
                    }),
                }
            } else {
                None
            };

            // Create a minimal context for fallback (run_local_with)
            let mut ctx = Context::new();
            ctx.set_working_path(project_path.clone());
            let opts = RunOptions {
                cwd: Some(project_path.clone()),
                env: None,
                show_output: true,
                package_manager: package_manager.clone(),
                tool_info: tool_info.clone(),
            };

            // Install step (same as build flow): skip for DDEV projects (PHP/CMS served by container)
            let skip_install_for_ddev = framework
                .slug
                .as_ref()
                .is_some_and(|s| is_ddev_supported_framework(s) && has_ddev_config(&project_path));

            let user_install_script = package_manager
                .as_ref()
                .and_then(|pm| pm.install_script.clone());
            let install_cmd = if let Some(ref user_install) = user_install_script {
                user_install.clone()
            } else if let Some(ref framework_install) = framework_install_cmd {
                framework_install.clone()
            } else {
                get_default_install_command(&package_manager)
            };

            // Sandbox for install and dev (mise-managed)
            let config = SandboxConfig::new(project_path.clone())
                .with_settings(mise_tools_for_execution(
                    &package_manager,
                    tool_info.as_ref(),
                ));
            let sandbox = create_sandbox(config).await.ok();
            if sandbox.is_none() {
                tracing::debug!(
                    "Sandbox setup failed, will use run_local_with fallback for install/dev"
                );
            }

            if skip_install_for_ddev {
                // WordPress DDEV: wp core install as the install step
                if framework.slug.as_deref() == Some("wordpress") {
                    println!("✓ Using WordPress install (wp core install)");
                    let mut ctx_wp = Context::new();
                    ctx_wp.set_working_path(project_path.clone());
                    let wp_opts = RunOptions {
                        cwd: Some(project_path.clone()),
                        env: None,
                        show_output: false,
                        package_manager: None,
                        tool_info: None,
                    };
                    let is_installed = run_local_with(
                        &ctx_wp,
                        "ddev exec wp core is-installed",
                        wp_opts.clone(),
                    )
                    .await
                    .is_ok();

                    if !is_installed {
                        println!("\n📦 Running install command...");
                        let project_name = project_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("wordpress");
                        let url = format!("https://{}.ddev.site", project_name);
                        let wp_install_cmd = format!(
                            "ddev exec wp core install --url={} --title=WordPress \
                             --admin_user=admin --admin_password=admin \
                             --admin_email=admin@example.com --skip-email",
                            url
                        );
                        let mut opts_show = wp_opts;
                        opts_show.show_output = true;
                        run_local_with(&ctx_wp, &wp_install_cmd, opts_show).await?;
                        println!("✓ WordPress installed (admin / admin)");
                    } else {
                        println!("✓ WordPress already installed");
                    }
                } else {
                    println!("✓ Skipping install for DDEV project");
                }
            } else {
                if user_install_script.is_some() {
                    println!("✓ Using user-defined install script from package.json");
                } else if framework_install_cmd.is_some() {
                    println!("✓ Using framework install command");
                } else {
                    println!("✓ Using package manager default install command");
                }

                let registry = session.get_task_registry();
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
                    println!("\n📦 Running install command...");
                    let install_result = if let Some(ref s) = &sandbox {
                        s.exec_interactive(&install_cmd)
                            .await
                            .map(|_| ())
                            .map_err(|e| miette::miette!("{}", e))
                    } else {
                        run_local_with(&ctx, &install_cmd, opts.clone()).await
                    };

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
            }

            // If sharing, wait a bit for dev server to start
            if share {
                println!("⏳ Waiting for dev server to start on port {}...", port);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }

            // For DDEV frameworks: DDEV serves the site — use ddev launch + ddev logs -f
            // instead of the framework's dev command (e.g. php -S)
            let use_ddev_dev = framework
                .slug
                .as_ref()
                .is_some_and(|s| is_ddev_supported_framework(s) && has_ddev_config(&project_path));

            let result = if use_ddev_dev {
                println!("✓ Using DDEV (site served by DDEV)");
                println!("\n🌐 Opening browser and streaming logs (Ctrl+C to stop)...");
                let _ = run_local_with(&ctx, "ddev launch", opts.clone()).await;
                run_local_with(&ctx, "ddev logs -f", opts.clone()).await
            } else {
                match &sandbox {
                    Some(s) => s
                        .exec_interactive(&dev_cmd)
                        .await
                        .map(|_| ())
                        .map_err(|e| miette::miette!("{}", e)),
                    None => run_local_with(&ctx, &dev_cmd, opts.clone()).await,
                }
            };

            // Clean up tunnel after command completes (success or failure)
            if let Some(ref mut t) = tunnel {
                let _ = t.stop().await;
            }

            // Propagate DDEV errors or handle Windows shell script fallback
            if let Err(e) = result {
                if use_ddev_dev {
                    return Err(e.into());
                }
                #[cfg(target_os = "windows")]
                {
                    let used_user_script = package_manager
                        .as_ref()
                        .and_then(|pm| pm.dev_script.as_ref())
                        .map(|s| s == &dev_cmd)
                        .unwrap_or(false);

                    if used_user_script && is_shell_script(&dev_cmd) {
                        eprintln!("⚠️  Warning: Shell script detected. Falling back to framework dev command.");
                        println!("✓ Using framework dev command");
                        let mut fallback_opts = opts;
                        fallback_opts.show_output = true;
                        let fallback_result =
                            run_local_with(&ctx, &framework_dev_cmd, fallback_opts).await;
                        fallback_result?;
                        return Ok(None);
                    }
                }
                return Err(e.into());
            }
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
