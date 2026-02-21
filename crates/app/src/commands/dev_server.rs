use crate::session::AppzSession;
use dev_server::{DevServer, ServerConfig};
use starbase::AppResult;
use tracing::instrument;

/// Start the development server
#[instrument(skip_all)]
pub async fn dev_server(session: AppzSession, args: crate::args::DevServerArgs) -> AppResult {
    let project_path = session.working_dir.clone();

    let port = args.port;
    let dir = args.dir.clone().unwrap_or_else(|| project_path.clone());
    let hot_reload = !args.no_reload;
    let enable_forms = args.enable_forms;
    let spa_fallback = args.spa_fallback;

    // Check if path exists
    if !dir.exists() {
        return Err(miette::miette!("Path does not exist: {}", dir.display()));
    }

    if !dir.is_dir() {
        return Err(miette::miette!(
            "Path is not a directory: {}",
            dir.display()
        ));
    }

    // Create server configuration
    let config = ServerConfig {
        address: "127.0.0.1".to_string(),
        port,
        root_dir: dir.clone(),
        hot_reload,
        enable_forms,
        upload_dir: None,
        cors: true,
        directory_listing: false,
        spa_fallback,
    };

    println!("Starting dev server on http://127.0.0.1:{}", port);
    println!("Serving files from: {}", dir.display());
    if hot_reload {
        println!(
            "Hot reload enabled - connect to ws://127.0.0.1:{}/__hot_reload",
            port
        );
    }
    if enable_forms {
        println!("Form data processing enabled");
    }
    if spa_fallback {
        println!("SPA fallback enabled");
    }

    // Create and start the server
    let mut server =
        DevServer::new(config).map_err(|e| miette::miette!("Failed to create server: {}", e))?;
    server
        .start()
        .await
        .map_err(|e| miette::miette!("Server error: {}", e))?;

    Ok(None)
}
