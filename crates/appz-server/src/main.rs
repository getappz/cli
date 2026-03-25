use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use tokio::sync::broadcast;

use appz_server::control::start_control_plane;
use appz_server::events::EventBus;
use appz_server::proxy::start_proxy;
use appz_server::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "appz-server", version, about = "Appz development server daemon")]
struct Args {
    /// Port for the HTTP reverse proxy (default 47831)
    #[arg(long, default_value = "47831")]
    port: u16,

    /// Path to the Unix control socket
    #[arg(long)]
    socket: Option<String>,

    /// Directory for state database and PID file
    #[arg(long)]
    data_dir: Option<String>,
}

fn default_socket_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".appz")
        .join("server.sock")
}

fn default_data_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".appz")
}

#[tokio::main]
async fn main() {
    // Initialise tracing with RUST_LOG / APPZ_LOG env filter.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("APPZ_LOG")
                .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let data_dir = args
        .data_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);

    let socket_path = args
        .socket
        .unwrap_or_else(|| default_socket_path().to_string_lossy().into_owned());

    // Write PID file so other tools can find us.
    let pid_file = data_dir.join("server.pid");
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::warn!("could not create data dir: {e}");
    }
    if let Err(e) = std::fs::write(&pid_file, std::process::id().to_string()) {
        tracing::warn!("could not write PID file: {e}");
    }

    let start_time = Instant::now();

    let state = match AppState::new(&data_dir) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!("failed to initialise AppState: {e}");
            std::process::exit(1);
        }
    };

    let events = Arc::new(EventBus::new(256));

    // Broadcast channel used to signal shutdown to all subsystems.
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    // Spawn the HTTP reverse proxy.
    let proxy_state = state.clone();
    let proxy_events = events.clone();
    let proxy_port = args.port;
    tokio::spawn(async move {
        start_proxy(proxy_port, proxy_state, proxy_events).await;
    });

    // Spawn the Unix socket control plane.
    let ctrl_state = state.clone();
    let ctrl_events = events.clone();
    let ctrl_socket = socket_path.clone();
    let ctrl_shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        start_control_plane(
            ctrl_socket,
            ctrl_state,
            ctrl_events,
            start_time,
            proxy_port,
            ctrl_shutdown,
        )
        .await;
    });

    tracing::info!(
        "appz-server started (proxy :{}, socket {})",
        args.port,
        socket_path
    );

    // Wait for Ctrl-C or a shutdown signal from the control plane.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received Ctrl-C, shutting down");
        }
        _ = shutdown_rx.recv() => {
            tracing::info!("shutdown requested via control plane");
        }
    }

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&pid_file);

    tracing::info!("appz-server stopped");
}
