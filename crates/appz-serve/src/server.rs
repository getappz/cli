use crate::config::ServerConfig;
use crate::error::{Result, ServeError};
use crate::reload::handle_websocket_upgrade;
use crate::static_files::{handle_static_file, internal_error};
use crate::watcher::FileWatcher;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub struct DevServer {
    config: Arc<ServerConfig>,
    change_receiver: Option<broadcast::Receiver<crate::watcher::FileChangeEvent>>,
}

impl DevServer {
    pub fn new(config: ServerConfig) -> Result<Self> {
        Ok(Self {
            config: Arc::new(config),
            change_receiver: None,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr = self.config.bind_addr();
        if self.config.hot_reload {
            let (_watcher, receiver) = FileWatcher::new(self.config.root_dir.clone())?;
            self.change_receiver = Some(receiver);
        }

        let listener = TcpListener::bind(&addr).await?;
        let config = Arc::clone(&self.config);
        let reload_rx = self.change_receiver.as_ref().map(|r| r.resubscribe());

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                accept = listener.accept() => {
                    let (stream, _) = accept?;
                    let io = hyper_util::rt::TokioIo::new(stream);
                    let config_clone = Arc::clone(&config);
                    let change_receiver = reload_rx.as_ref().map(|r| r.resubscribe());
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            let cfg = Arc::clone(&config_clone);
                            let rx = change_receiver.as_ref().map(|r| r.resubscribe());
                            async move { Ok::<_, std::convert::Infallible>(dispatch(req, cfg, rx).await) }
                        });
                        let _ = http1::Builder::new().serve_connection(io, service).await;
                    });
                }
            }
        }
        Ok(())
    }
}

async fn dispatch(
    request: Request<hyper::body::Incoming>,
    config: Arc<ServerConfig>,
    change_receiver: Option<broadcast::Receiver<crate::watcher::FileChangeEvent>>,
) -> Response<Full<Bytes>> {
    let path = request.uri().path();
    let method = request.method();

    if path == "/__hot_reload"
        && config.hot_reload
        && let Some(receiver) = change_receiver
    {
        return handle_websocket_upgrade(request, receiver)
            .await
            .unwrap_or_else(|_| internal_error());
    }

    if *method != Method::GET && *method != Method::HEAD {
        return Response::builder()
            .status(hyper::StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method Not Allowed")))
            .unwrap_or_else(|_| internal_error());
    }

    handle_static_file(&config.root_dir, path, method, config.spa_fallback)
        .await
        .unwrap_or_else(|_| internal_error())
}

#[allow(dead_code)]
fn map_err(e: ServeError) -> Response<Full<Bytes>> {
    let _ = e;
    internal_error()
}
