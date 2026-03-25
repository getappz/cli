use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};

use crate::events::EventBus;
use crate::state::AppState;

/// Bind a TCP listener on `port` and serve reverse-proxy requests.
pub async fn start_proxy(port: u16, state: Arc<AppState>, events: Arc<EventBus>) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            tracing::info!("proxy listening on {addr}");
            l
        }
        Err(e) => {
            tracing::error!("failed to bind proxy on {addr}: {e}");
            return;
        }
    };

    loop {
        let (stream, _peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("proxy accept error: {e}");
                continue;
            }
        };

        let state = state.clone();
        let events = events.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| {
                handle_proxy_request(req, state.clone(), events.clone())
            });
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!("proxy connection error: {e}");
            }
        });
    }
}

/// Extract the `Host` header, find the matching app, and proxy the request.
async fn handle_proxy_request(
    req: Request<Incoming>,
    state: Arc<AppState>,
    events: Arc<EventBus>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let host = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        // Strip port if present
        .map(|h| h.split(':').next().unwrap_or(h).to_string())
        .unwrap_or_default();

    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    match state.find_by_host(&host) {
        None => {
            tracing::debug!("no app for host '{host}'");
            let body = Full::new(Bytes::from(format!("404 Not Found: no app for host '{host}'")));
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(body)
                .unwrap())
        }
        Some(app) => {
            events.publish(crate::protocol::ServerEvent::RequestStarted {
                host: host.clone(),
                method: method.clone(),
                path: path.clone(),
            });

            let upstream_port = app.upstream_port;
            let resp: Response<Full<Bytes>> = proxy_to_upstream(req, upstream_port).await?;
            let status = resp.status().as_u16();

            events.publish(crate::protocol::ServerEvent::RequestFinished {
                host,
                method,
                path,
                status,
            });

            Ok(resp)
        }
    }
}

/// Connect to `127.0.0.1:{upstream_port}`, forward the request, and collect the response.
async fn proxy_to_upstream(
    req: Request<Incoming>,
    upstream_port: u16,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let addr = format!("127.0.0.1:{upstream_port}");

    let stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("upstream connect error ({addr}): {e}");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("502 Bad Gateway: {e}"))))
                .unwrap());
        }
    };

    let io = TokioIo::new(stream);
    let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("upstream handshake error: {e}");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("502 Bad Gateway: {e}"))))
                .unwrap());
        }
    };

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::debug!("upstream connection error: {e}");
        }
    });

    // Convert the request body to Full<Bytes> so we can forward it.
    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!("error reading request body: {e}");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("502 Bad Gateway: body read error")))
                .unwrap());
        }
    };
    let forwarded_req = Request::from_parts(parts, Full::new(body_bytes));

    let upstream_resp = match sender.send_request(forwarded_req).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("upstream send error: {e}");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("502 Bad Gateway: {e}"))))
                .unwrap());
        }
    };

    // Collect the response body.
    let (resp_parts, resp_body) = upstream_resp.into_parts();
    let resp_bytes = match resp_body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!("upstream response body error: {e}");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("502 Bad Gateway: upstream body error")))
                .unwrap());
        }
    };

    Ok(Response::from_parts(resp_parts, Full::new(resp_bytes)))
}
