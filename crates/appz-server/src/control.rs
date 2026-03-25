use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::events::EventBus;
use crate::process;
use crate::protocol::{Request, Response, ServerEvent};
use crate::state::{AppState, AppStatus, RuntimeApp};

/// Remove stale socket, bind a Unix listener, and accept/dispatch connections.
pub async fn start_control_plane(
    socket_path: String,
    state: Arc<AppState>,
    events: Arc<EventBus>,
    start_time: Instant,
    proxy_port: u16,
    shutdown: broadcast::Sender<()>,
) {
    // Remove any stale socket file from a previous run.
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => {
            tracing::info!("control plane listening on {socket_path}");
            l
        }
        Err(e) => {
            tracing::error!("failed to bind control socket at {socket_path}: {e}");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = state.clone();
                let events = events.clone();
                let shutdown = shutdown.clone();
                tokio::spawn(handle_client(
                    stream,
                    state,
                    events,
                    start_time,
                    proxy_port,
                    shutdown,
                ));
            }
            Err(e) => {
                tracing::warn!("control accept error: {e}");
            }
        }
    }
}

/// Read JSONL lines from the Unix stream and dispatch each request.
async fn handle_client(
    stream: UnixStream,
    state: Arc<AppState>,
    events: Arc<EventBus>,
    start_time: Instant,
    proxy_port: u16,
    shutdown: broadcast::Sender<()>,
) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                tracing::debug!("control read error: {e}");
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::Error {
                    message: format!("invalid request: {e}"),
                };
                let _ = send_response(&mut write_half, &resp).await;
                continue;
            }
        };

        // SubscribeEvents enters a streaming loop — handle separately.
        if matches!(request, Request::SubscribeEvents) {
            let resp = Response::Subscribed;
            if send_response(&mut write_half, &resp).await.is_err() {
                break;
            }

            let mut rx = events.subscribe();
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let resp = Response::Event { event };
                        if send_response(&mut write_half, &resp).await.is_err() {
                            return;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            return;
        }

        let resp = process_request(
            request,
            state.clone(),
            events.clone(),
            start_time,
            proxy_port,
            shutdown.clone(),
        )
        .await;

        if send_response(&mut write_half, &resp).await.is_err() {
            break;
        }
    }
}

/// Dispatch a single request and return a response.
async fn process_request(
    request: Request,
    state: Arc<AppState>,
    events: Arc<EventBus>,
    _start_time: Instant,
    _proxy_port: u16,
    shutdown: broadcast::Sender<()>,
) -> Response {
    match request {
        Request::Ping => Response::Pong,

        Request::Info => Response::ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_count: state.app_count(),
        },

        Request::RegisterApp {
            config_path,
            project_dir,
            app_name,
            upstream_port,
            command,
            env,
            hosts,
            static_dir,
            hot_reload,
        } => {
            // Build URL from first host, or fall back to port.
            let _url = if let Some(host) = hosts.first() {
                format!("http://{host}")
            } else {
                format!("http://localhost:{upstream_port}")
            };

            let app = RuntimeApp {
                config_path: config_path.clone(),
                project_dir,
                app_name: app_name.clone(),
                upstream_port,
                command,
                env,
                hosts,
                static_dir,
                hot_reload,
                status: AppStatus::Idle,
                pid: None,
            };

            match state.register(app) {
                Ok(()) => {
                    events.publish(ServerEvent::AppStatusChanged {
                        config_path: config_path.clone(),
                        app_name,
                        status: "idle".to_string(),
                    });
                    Response::AppRegistered { config_path }
                }
                Err(e) => Response::Error {
                    message: format!("register failed: {e}"),
                },
            }
        }

        Request::UnregisterApp { config_path } => {
            // Kill if running
            if let Some(app) = state.get(&config_path) {
                if let Some(pid) = app.pid {
                    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
                }
            }
            match state.unregister(&config_path) {
                Ok(_) => Response::AppUnregistered { config_path },
                Err(e) => Response::Error {
                    message: format!("unregister failed: {e}"),
                },
            }
        }

        Request::SetAppStatus { config_path, status } => {
            let parsed_status = AppStatus::parse(&status);
            if state.set_status(&config_path, parsed_status) {
                let app_name = state
                    .get(&config_path)
                    .map(|a| a.app_name)
                    .unwrap_or_else(|| config_path.clone());
                events.publish(ServerEvent::AppStatusChanged {
                    config_path: config_path.clone(),
                    app_name,
                    status: status.clone(),
                });
                Response::AppStatusUpdated { config_path, status }
            } else {
                Response::Error {
                    message: format!("app not found: {config_path}"),
                }
            }
        }

        Request::HandoffApp { config_path } => {
            // The client is telling us it already started the process at the registered port.
            // We just need to monitor it.
            match state.get(&config_path) {
                None => Response::Error {
                    message: format!("app not found: {config_path}"),
                },
                Some(app) => {
                    if let Some(pid) = app.pid {
                        state.set_status(&config_path, AppStatus::Running);
                        tokio::spawn(process::monitor_external_pid(
                            pid,
                            config_path.clone(),
                            state.clone(),
                            events.clone(),
                        ));
                        Response::AppHandedOff { config_path }
                    } else {
                        Response::Error {
                            message: format!("no PID set for app: {config_path}"),
                        }
                    }
                }
            }
        }

        Request::RestartApp { config_path } => {
            // Kill existing process if any
            if let Some(app) = state.get(&config_path) {
                if let Some(pid) = app.pid {
                    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
                }
                events.publish(ServerEvent::RestartRequested {
                    config_path: config_path.clone(),
                    app_name: app.app_name.clone(),
                });
            } else {
                return Response::Error {
                    message: format!("app not found: {config_path}"),
                };
            }

            match process::spawn_app(state, events, config_path.clone()).await {
                Ok(_pid) => Response::AppRestarted { config_path },
                Err(e) => Response::Error {
                    message: format!("spawn failed: {e}"),
                },
            }
        }

        Request::ListApps => Response::Apps {
            apps: state.list(),
        },

        // SubscribeEvents is handled in handle_client before this function is called.
        Request::SubscribeEvents => Response::Subscribed,

        Request::StopServer => {
            let _ = shutdown.send(());
            Response::Ok
        }
    }
}

/// Serialize `response` as a JSON line and write it to `writer`.
async fn send_response<W>(writer: &mut W, response: &Response) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let mut json = serde_json::to_string(response)
        .unwrap_or_else(|_| r#"{"type":"Error","message":"serialization error"}"#.to_string());
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
