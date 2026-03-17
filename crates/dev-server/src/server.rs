use crate::config::ServerConfig;
use crate::error::Result;
use crate::handlers::{handle_form_data, handle_static_file, handle_websocket_upgrade};
use crate::watcher::FileWatcher;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderValue, ACCESS_CONTROL_ALLOW_ORIGIN};
use hyper::http::StatusCode;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

/// Dev Server instance
pub struct DevServer {
    config: Arc<ServerConfig>,
    watcher: Option<FileWatcher>,
    change_receiver: Option<broadcast::Receiver<crate::watcher::FileChangeEvent>>,
}

impl DevServer {
    /// Create a new dev server instance
    pub fn new(config: ServerConfig) -> Result<Self> {
        let config = Arc::new(config);
        Ok(Self {
            config,
            watcher: None,
            change_receiver: None,
        })
    }

    /// Start the dev server
    pub async fn start(&mut self) -> Result<()> {
        let addr = self.config.bind_addr();
        info!("Starting dev server on http://{}", addr);

        // Initialize file watcher if hot reload is enabled
        if self.config.hot_reload {
            let (watcher, receiver) = FileWatcher::new(self.config.root_dir.clone())?;
            self.watcher = Some(watcher);
            self.change_receiver = Some(receiver);
            info!("Hot reload enabled - watching for file changes");
        }

        // Create TCP listener
        let listener = TcpListener::bind(&addr).await?;
        info!("Server listening on http://{}", addr);
        info!("Serving files from: {}", self.config.root_dir.display());

        // Get a clone of the config and change receiver for the service
        let config = Arc::clone(&self.config);
        let change_receiver = self.change_receiver.as_ref().map(|r| r.resubscribe());

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from: {}", addr);
                    let io = hyper_util::rt::TokioIo::new(stream);

                    let config_clone = Arc::clone(&config);
                    let change_receiver_clone = change_receiver.as_ref().map(|r| r.resubscribe());

                    tokio::task::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(
                                req,
                                Arc::clone(&config_clone),
                                change_receiver_clone.as_ref().map(|r| r.resubscribe()),
                            )
                        });

                        if let Err(err) = http1::Builder::new().serve_connection(io, service).await
                        {
                            error!("Error serving connection: {}", err);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    request: Request<hyper::body::Incoming>,
    config: Arc<ServerConfig>,
    change_receiver: Option<broadcast::Receiver<crate::watcher::FileChangeEvent>>,
) -> Result<Response<Full<Bytes>>> {
    let path = request.uri().path();
    let method = request.method();

    debug!("{} {}", method, path);

    // Handle CORS preflight
    if method == Method::OPTIONS && config.cors {
        return Ok(create_cors_response());
    }

    // Handle WebSocket upgrade for hot reload
    if path == "/__hot_reload" && config.hot_reload {
        if let Some(receiver) = change_receiver {
            return handle_websocket_upgrade(request, receiver).await;
        }
    }

    // Handle form data processing
    if method == Method::POST && config.enable_forms {
        let upload_dir = config.upload_directory();
        return handle_form_data(request, upload_dir).await;
    }

    // Handle CDN image optimization URLs (passthrough — serve original image)
    if method == Method::GET {
        if let Some(image_path) = extract_cdn_image_path(request.uri()) {
            let response = handle_static_file(
                &config.root_dir,
                &image_path,
                false,
                false,
            )
            .await?;

            if config.cors {
                return Ok(add_cors_headers(response));
            }
            return Ok(response);
        }
    }

    // Handle static file serving
    if method == Method::GET || method == Method::HEAD {
        let response = handle_static_file(
            &config.root_dir,
            path,
            config.spa_fallback,
            config.directory_listing,
        )
        .await?;

        // Add CORS headers if enabled
        if config.cors {
            return Ok(add_cors_headers(response));
        }

        return Ok(response);
    }

    // Method not allowed
    Ok(Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Full::new(Bytes::from("Method Not Allowed")))?)
}

/// Create a CORS preflight response
fn create_cors_response() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
        .header(
            "Access-Control-Allow-Methods",
            HeaderValue::from_static("GET, POST, OPTIONS"),
        )
        .header(
            "Access-Control-Allow-Headers",
            HeaderValue::from_static("Content-Type"),
        )
        .body(Full::new(Bytes::new()))
        .unwrap()
}

/// Add CORS headers to a response
fn add_cors_headers(mut response: Response<Full<Bytes>>) -> Response<Full<Bytes>> {
    let headers = response.headers_mut();
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    response
}

use hyper::header::CONTENT_TYPE;

/// Extract the source image path from CDN image optimization URLs.
///
/// Uses the unpic library to detect and extract source URLs from any
/// supported image CDN provider (Vercel, Netlify, Cloudflare, Imgix,
/// Cloudinary, Shopify, and 20+ more).
///
/// Returns the original image path for passthrough serving.
fn extract_cdn_image_path(uri: &hyper::Uri) -> Option<String> {
    let url_str = format!("https://localhost{}", uri);
    unpic::extract_source_url(&url_str).ok().flatten()
}
