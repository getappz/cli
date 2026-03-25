use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use appz_server::protocol::{AppInfo, Request, Response};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("server error: {0}")]
    Server(String),
    #[error("connection error: {0}")]
    Connection(String),
    #[error("timed out waiting for server")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, ClientError>;

/// Returns the default Unix socket path: ~/.appz/server.sock
fn default_socket_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".appz")
        .join("server.sock")
}

/// Connect to the control socket, send a request, and read one response line.
async fn send_request(request: &Request) -> Result<Response> {
    let socket_path = default_socket_path();
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        ClientError::Connection(format!(
            "cannot connect to {}: {e}",
            socket_path.display()
        ))
    })?;

    let (read_half, mut write_half) = stream.into_split();

    let mut json = serde_json::to_string(request)?;
    json.push('\n');
    write_half.write_all(json.as_bytes()).await?;
    write_half.flush().await?;

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let response: Response = serde_json::from_str(line.trim())?;
    Ok(response)
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Send a Ping and expect Pong.
pub async fn ping() -> Result<()> {
    match send_request(&Request::Ping).await? {
        Response::Pong => Ok(()),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// Query server info. Returns (listen_port, app_count, uptime_secs).
///
/// Note: the current protocol's ServerInfo response carries `version` and
/// `app_count` only. `listen_port` and `uptime_secs` are not transmitted,
/// so both are returned as 0.
pub async fn info() -> Result<(u16, usize, u64)> {
    match send_request(&Request::Info).await? {
        Response::ServerInfo { app_count, .. } => Ok((0, app_count, 0)),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// Register an app with the server. Returns the URL for the app.
#[allow(clippy::too_many_arguments)]
pub async fn register_app(
    config_path: String,
    project_dir: String,
    app_name: String,
    upstream_port: u16,
    command: Vec<String>,
    env: HashMap<String, String>,
    hosts: Vec<String>,
    static_dir: Option<String>,
    hot_reload: bool,
) -> Result<String> {
    let url = if let Some(host) = hosts.first() {
        format!("http://{host}")
    } else {
        format!("http://localhost:{upstream_port}")
    };

    let req = Request::RegisterApp {
        config_path,
        project_dir,
        app_name,
        upstream_port,
        command,
        env,
        hosts,
        static_dir,
        hot_reload,
    };

    match send_request(&req).await? {
        Response::AppRegistered { .. } => Ok(url),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// Unregister an app from the server.
pub async fn unregister_app(config_path: String) -> Result<()> {
    match send_request(&Request::UnregisterApp { config_path }).await? {
        Response::AppUnregistered { .. } => Ok(()),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// Hand off a running process (by PID) to the server for monitoring.
///
/// Note: the protocol's HandoffApp request carries only `config_path`; the
/// `pid` is set via a preceding SetAppStatus / state mutation on the server.
/// This function first records the PID via SetAppStatus then sends HandoffApp.
pub async fn handoff_app(config_path: String, pid: u32) -> Result<()> {
    // Record the PID on the server side before handing off.
    let set_req = Request::SetAppStatus {
        config_path: config_path.clone(),
        status: format!("running:{pid}"),
    };
    // Best-effort: ignore errors from the status update so we can still hand off.
    let _ = send_request(&set_req).await;

    match send_request(&Request::HandoffApp { config_path }).await? {
        Response::AppHandedOff { .. } => Ok(()),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// List all registered apps.
pub async fn list_apps() -> Result<Vec<AppInfo>> {
    match send_request(&Request::ListApps).await? {
        Response::Apps { apps } => Ok(apps),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

/// Tell the server to shut itself down.
pub async fn stop_server() -> Result<()> {
    match send_request(&Request::StopServer).await? {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(ClientError::Server(message)),
        other => Err(ClientError::Server(format!("unexpected response: {other:?}"))),
    }
}

// ─── Binary finder ───────────────────────────────────────────────────────────

/// Locate the `appz-server` binary using a prioritised search.
///
/// Search order:
/// 1. `target/debug/appz-server` and `target/release/appz-server` (local dev)
/// 2. Same directory as the current executable (sibling binary)
/// 3. PATH via `which::which`
/// 4. `~/.appz/bin/appz-server`
pub fn find_server_binary() -> Result<PathBuf> {
    // 1. Local dev builds relative to the current working directory.
    for rel in &["target/debug/appz-server", "target/release/appz-server"] {
        let p = PathBuf::from(rel);
        if p.is_file() {
            tracing::debug!("found appz-server at local dev path: {}", p.display());
            return Ok(p);
        }
    }

    // 2. Sibling of the currently running executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("appz-server");
            if sibling.is_file() {
                tracing::debug!("found appz-server as sibling: {}", sibling.display());
                return Ok(sibling);
            }
        }
    }

    // 3. PATH lookup.
    if let Ok(p) = which::which("appz-server") {
        tracing::debug!("found appz-server in PATH: {}", p.display());
        return Ok(p);
    }

    // 4. ~/.appz/bin/appz-server
    if let Some(home) = dirs_next::home_dir() {
        let p = home.join(".appz").join("bin").join("appz-server");
        if p.is_file() {
            tracing::debug!("found appz-server in ~/.appz/bin: {}", p.display());
            return Ok(p);
        }
    }

    Err(ClientError::Connection(
        "appz-server binary not found. \
         Try installing it with `cargo install --path crates/appz-server` \
         or ensure it is on your PATH."
            .to_string(),
    ))
}

// ─── Daemon lifecycle ─────────────────────────────────────────────────────────

/// Ensure the server daemon is running on `port`.
///
/// If a ping succeeds the server is already up and this function returns
/// immediately. Otherwise it locates the binary, spawns it detached in its
/// own session (setsid), and waits up to 10 seconds for the ping to succeed.
pub async fn ensure_running(port: u16) -> Result<()> {
    // Fast path — server already up.
    if ping().await.is_ok() {
        tracing::debug!("appz-server already running");
        return Ok(());
    }

    let binary = find_server_binary()?;
    tracing::info!("starting appz-server from {}", binary.display());

    // Ensure log directory exists.
    let log_dir = dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".appz")
        .join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("server.log");

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let stdout_fd = {
        use std::os::unix::io::IntoRawFd;
        log_file.into_raw_fd()
    };

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.arg("--port").arg(port.to_string());

    // Redirect stdout and stderr to the log file.
    // SAFETY: dup2 is async-signal-safe and we only do fd manipulation.
    unsafe {
        use std::os::unix::io::FromRawFd;
        let stderr_file = std::fs::File::from_raw_fd(libc::dup(stdout_fd));
        cmd.stdout(stderr_file);

        let stdout_file = std::fs::File::from_raw_fd(stdout_fd);
        cmd.stdout(stdout_file);

        cmd.pre_exec(|| {
            // Create a new session so the daemon is detached from the terminal.
            libc::setsid();
            Ok(())
        });
    }

    cmd.stdin(std::process::Stdio::null())
        .kill_on_drop(false)
        .spawn()
        .map_err(|e| ClientError::Connection(format!("failed to spawn appz-server: {e}")))?;

    // Wait up to 10 seconds for the daemon to become reachable.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if ping().await.is_ok() {
            tracing::info!("appz-server is up");
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(ClientError::Timeout);
        }
    }
}
