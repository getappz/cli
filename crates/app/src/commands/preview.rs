//! Preview command - serve static files from build output directory

use detectors::{detect_framework_record, DetectFrameworkRecordOptions, StdFilesystem};
use crate::session::AppzSession;
use crate::tunnel::{CloudflaredTunnel, TunnelService};
use dev_server::{DevServer, ServerConfig};
use frameworks::frameworks;
use starbase::AppResult;
use std::sync::Arc;
use tokio::signal;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn preview(session: AppzSession) -> AppResult {
    // Extract CLI flags
    let port = if let crate::app::Commands::Preview { port, .. } = &session.cli.command {
        *port
    } else {
        3000
    };

    let dir = if let crate::app::Commands::Preview { dir, .. } = &session.cli.command {
        dir.clone()
    } else {
        None
    };

    let share = if let crate::app::Commands::Preview { share, .. } = &session.cli.command {
        *share
    } else {
        false
    };

    let spa_fallback =
        if let crate::app::Commands::Preview { spa_fallback, .. } = &session.cli.command {
            *spa_fallback
        } else {
            false
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

    // Create filesystem detector
    let fs = Arc::new(StdFilesystem::new(Some(project_path.clone())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    let output_dir = match detect_framework_record(options).await {
        Ok(Some((fw, _version, _package_manager))) => {
            // If explicit directory provided, use it
            if let Some(ref d) = dir {
                d.clone()
            } else {
                // Try to get from framework settings
                let mut found = None;
                if let Some(settings) = &fw.settings {
                    if let Some(output_dir) = &settings.output_directory {
                        if let Some(value) = output_dir.value {
                            let dir = project_path.join(value);
                            if dir.exists() {
                                found = Some(dir);
                            }
                        }
                    }
                }

                // Fallback to common output directories if not found in settings
                found.unwrap_or_else(|| {
                    let common_dirs = ["dist", "build", ".output/public", "out", ".next"];
                    for dir_name in &common_dirs {
                        let dir = project_path.join(dir_name);
                        if dir.exists() {
                            return dir;
                        }
                    }
                    // Default to dist if nothing found (will error later if doesn't exist)
                    project_path.join("dist")
                })
            }
        }
        Ok(None) => {
            // No framework detected, try to use explicit dir or common defaults
            if let Some(ref d) = dir {
                d.clone()
            } else {
                // Try common directories
                let common_dirs = ["dist", "build", ".output/public", "out"];
                let mut found = None;
                for dir_name in &common_dirs {
                    let dir_path = project_path.join(dir_name);
                    if dir_path.exists() {
                        found = Some(dir_path);
                        break;
                    }
                }
                found.unwrap_or_else(|| project_path.join("dist"))
            }
        }
        Err(e) => {
            return Err(miette::miette!("Error detecting framework: {}", e));
        }
    };

    // Check if output directory exists
    if !output_dir.exists() {
        return Err(miette::miette!(
            "Build output directory not found: {}\n\nPlease run 'appz build' first to build your project.",
            output_dir.display()
        ));
    }

    if !output_dir.is_dir() {
        return Err(miette::miette!(
            "Output path is not a directory: {}",
            output_dir.display()
        ));
    }

    println!("✓ Serving files from: {}", output_dir.display());

    // Create server configuration
    let config = ServerConfig {
        address: "127.0.0.1".to_string(),
        port,
        root_dir: output_dir.clone(),
        hot_reload: false,
        enable_forms: false,
        upload_dir: None,
        cors: true,
        directory_listing: false,
        spa_fallback,
    };

    println!("Starting preview server on http://127.0.0.1:{}", port);

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

    // Create and start the server
    let mut server =
        DevServer::new(config).map_err(|e| miette::miette!("Failed to create server: {}", e))?;

    // Handle Ctrl+C to clean up tunnel
    if share {
        let tunnel_handle = tunnel.take();
        tokio::select! {
            result = server.start() => {
                // Clean up tunnel when server exits
                if let Some(mut t) = tunnel_handle {
                    let _ = t.stop().await;
                }
                result.map_err(|e| miette::miette!("Server error: {}", e))?;
                Ok(None)
            }
            _ = signal::ctrl_c() => {
                // Clean up tunnel on Ctrl+C
                if let Some(mut t) = tunnel_handle {
                    let _ = t.stop().await;
                }
                println!("\n✓ Tunnel stopped");
                Ok(None)
            }
        }
    } else {
        server
            .start()
            .await
            .map_err(|e| miette::miette!("Server error: {}", e))?;
        Ok(None)
    }
}
