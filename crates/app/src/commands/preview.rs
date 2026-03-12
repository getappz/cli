//! Preview command - serve static files from build output directory

use crate::session::AppzSession;
use crate::tunnel::{CloudflaredTunnel, TunnelService};
use crate::utils::build::detect_build_output_dir;
use dev_server::{DevServer, ServerConfig};
use starbase::AppResult;
use tokio::signal;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn preview(session: AppzSession, args: crate::args::PreviewArgs) -> AppResult {
    let port = args.port;
    let dir = args.dir.clone();
    let share = args.share;
    let spa_fallback = args.spa_fallback;

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

    // Detect build output directory using shared utility
    let output_dir = detect_build_output_dir(&project_path, dir.clone()).await?;

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
