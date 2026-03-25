use std::error::Error;
use std::sync::Arc;

use tokio::process::Command;

use crate::events::EventBus;
use crate::protocol::ServerEvent;
use crate::state::{AppState, AppStatus};

/// Sanitize an app name to only alphanumeric + hyphens, lowercase.
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

/// Spawn the app process, redirect stdout/stderr to a log file, set PID and status,
/// and monitor the process in the background.
pub async fn spawn_app(
    state: Arc<AppState>,
    events: Arc<EventBus>,
    config_path: String,
) -> Result<u32, Box<dyn Error + Send + Sync>> {
    let app = state
        .get(&config_path)
        .ok_or_else(|| format!("app not found: {config_path}"))?;

    // Resolve log file path: ~/.appz/logs/{sanitized_name}.log
    let home = dirs_next::home_dir().ok_or("cannot determine home directory")?;
    let log_dir = home.join(".appz").join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!("{}.log", sanitize_name(&app.app_name)));
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_file2 = log_file.try_clone()?;

    // Build command
    let (program, args) = app
        .command
        .split_first()
        .ok_or("command is empty")?;

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(&app.project_dir)
        .env("PORT", app.upstream_port.to_string());

    // Merge in app-specific env vars
    for (k, v) in &app.env {
        cmd.env(k, v);
    }

    cmd.stdout(log_file)
       .stderr(log_file2);

    let child = cmd.spawn()?;
    let pid = child.id().ok_or("process has no PID")?;

    state.set_pid(&config_path, Some(pid));
    state.set_status(&config_path, AppStatus::Starting);

    events.publish(ServerEvent::AppStatusChanged {
        config_path: config_path.clone(),
        app_name: app.app_name.clone(),
        status: "starting".to_string(),
    });

    // Monitor in background
    tokio::spawn(monitor_process(child, config_path, state, events));

    Ok(pid)
}

/// Wait for the child to exit and mark the app as idle.
pub async fn monitor_process(
    mut child: tokio::process::Child,
    config_path: String,
    state: Arc<AppState>,
    events: Arc<EventBus>,
) {
    let _ = child.wait().await;

    state.set_status(&config_path, AppStatus::Idle);
    state.set_pid(&config_path, None);

    let app_name = state
        .get(&config_path)
        .map(|a| a.app_name)
        .unwrap_or_else(|| config_path.clone());

    events.publish(ServerEvent::AppStatusChanged {
        config_path,
        app_name,
        status: "idle".to_string(),
    });
}

/// Poll an external PID every 2 seconds using `kill(pid, 0)`.
/// When the process disappears, mark the app as idle.
pub async fn monitor_external_pid(
    pid: u32,
    config_path: String,
    state: Arc<AppState>,
    events: Arc<EventBus>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        if !alive {
            break;
        }
    }

    state.set_status(&config_path, AppStatus::Idle);
    state.set_pid(&config_path, None);

    let app_name = state
        .get(&config_path)
        .map(|a| a.app_name)
        .unwrap_or_else(|| config_path.clone());

    events.publish(ServerEvent::AppStatusChanged {
        config_path,
        app_name,
        status: "idle".to_string(),
    });
}
