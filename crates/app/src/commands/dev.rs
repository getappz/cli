use crate::detectors::{
    detect_framework_record, detect_hugo_info, DetectFrameworkRecordOptions, StdFilesystem,
};
use crate::session::AppzSession;
use crate::shell::{command_exists, run_local_with, RunOptions, ToolVersionInfo};
use crate::tunnel::{CloudflaredTunnel, TunnelService};
use frameworks::frameworks;
use starbase::AppResult;
use std::path::Path;
use std::sync::Arc;
use task::Context;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn dev(session: AppzSession) -> AppResult {
    // Extract CLI flags
    let share = if let crate::app::Commands::Dev { share, .. } = &session.cli.command {
        *share
    } else {
        false
    };

    let port = if let crate::app::Commands::Dev { port, .. } = &session.cli.command {
        port.unwrap_or(3000)
    } else {
        3000
    };

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
    let fs = Arc::new(StdFilesystem::new(Some(project_path.clone())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    match detect_framework_record(options).await {
        Ok(Some((framework, _version, package_manager))) => {
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
            } else {
                println!("✓ Using framework dev command");
            }

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

            // Create filesystem detector for Hugo info detection
            let fs_for_hugo = Arc::new(StdFilesystem::new(Some(project_path.clone())));

            // Detect Hugo-specific info if this is a Hugo project
            let tool_info = if framework.slug == Some("hugo") {
                let fs_dyn: Arc<dyn crate::detectors::filesystem::DetectorFilesystem> = fs_for_hugo.clone();
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

            // Create a minimal context for running the command
            let mut ctx = Context::new();
            ctx.set_working_path(project_path.clone());
            let opts = RunOptions {
                cwd: Some(project_path),
                env: None,
                show_output: true,
                package_manager: package_manager.clone(),
                tool_info,
            };

            // If sharing, wait a bit for dev server to start
            if share {
                println!("⏳ Waiting for dev server to start on port {}...", port);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }

            // Try to run the dev command
            // If it fails and it's a shell script on Windows, fallback to framework command
            // Note: tunnel will be cleaned up by Drop when it goes out of scope
            let result = run_local_with(&ctx, &dev_cmd, opts.clone()).await;

            // Clean up tunnel after command completes (success or failure)
            if let Some(ref mut t) = tunnel {
                let _ = t.stop().await;
            }

            // Handle shell script fallback on Windows
            if result.is_err() {
                #[cfg(target_os = "windows")]
                {
                    // Check if this was a user script that failed due to shell script on Windows
                    let used_user_script = package_manager
                        .as_ref()
                        .and_then(|pm| pm.dev_script.as_ref())
                        .map(|s| s == &dev_cmd)
                        .unwrap_or(false);

                    if used_user_script && is_shell_script(&dev_cmd) {
                        // Use framework dev command as fallback
                        eprintln!("⚠️  Warning: Shell script detected. Falling back to framework dev command.");
                        println!("✓ Using framework dev command");
                        let mut fallback_opts = opts;
                        fallback_opts.show_output = true;
                        let fallback_result =
                            run_local_with(&ctx, &framework_dev_cmd, fallback_opts).await;

                        // Tunnel already cleaned up above, just return result
                        fallback_result?;
                        return Ok(None);
                    } else {
                        result?; // Not a user script shell script issue, return error
                        return Ok(None);
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    result?; // On Unix, just return the error
                    return Ok(None);
                }
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
