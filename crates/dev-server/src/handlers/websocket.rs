use crate::error::Result;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderValue, UPGRADE};
use hyper::http::StatusCode;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::sync::broadcast;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info};

use crate::watcher::FileChangeEvent;

/// Handle WebSocket upgrade request
pub async fn handle_websocket_upgrade(
    mut request: Request<hyper::body::Incoming>,
    change_receiver: broadcast::Receiver<FileChangeEvent>,
) -> Result<Response<Full<Bytes>>> {
    // Check if this is a WebSocket upgrade request
    if !request.headers().contains_key(UPGRADE) {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Bad Request")))?);
    }

    // Create the upgrade response
    let response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(UPGRADE, HeaderValue::from_static("websocket"))
        .header("Connection", HeaderValue::from_static("Upgrade"))
        .body(Full::new(Bytes::new()))?;

    // Set up the upgrade future
    let upgrade = hyper::upgrade::on(&mut request);

    // Spawn task to handle the WebSocket connection after upgrade
    tokio::spawn(async move {
        match upgrade.await {
            Ok(upgraded) => {
                info!("WebSocket connection established");
                handle_websocket_connection(upgraded, change_receiver).await;
            }
            Err(e) => {
                error!("WebSocket upgrade failed: {}", e);
            }
        }
    });

    Ok(response)
}

/// Handle an active WebSocket connection
async fn handle_websocket_connection(
    upgraded: Upgraded,
    change_receiver: broadcast::Receiver<FileChangeEvent>,
) {
    let ws_stream = WebSocketStream::from_raw_socket(
        TokioIo::new(upgraded),
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Send initial connection message
    let _ = ws_sender
        .send(Message::Text(
            r#"{"type":"connected","message":"Hot reload enabled"}"#.to_string().into(),
        ))
        .await;

    // Use a channel to forward file change events to WebSocket
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let mut change_receiver_clone = change_receiver;
    tokio::spawn(async move {
        while let Ok(event) = change_receiver_clone.recv().await {
            let _ = tx.send(event).await;
        }
    });

    // Handle both WebSocket messages and file change events
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages (ping/pong)
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sender.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket connection closed by client");
                        break;
                    }
                    Some(Ok(_)) => {
                        // Ignore other messages
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        info!("WebSocket stream ended");
                        break;
                    }
                }
            }
            // Handle file change events
            event = rx.recv() => {
                match event {
                    Some(evt) => {
                        let message = match evt {
                            FileChangeEvent::Created(path) => {
                                format!(r#"{{"type":"reload","event":"created","path":"{}"}}"#, path.display())
                            }
                            FileChangeEvent::Modified(path) => {
                                format!(r#"{{"type":"reload","event":"modified","path":"{}"}}"#, path.display())
                            }
                            FileChangeEvent::Deleted(path) => {
                                format!(r#"{{"type":"reload","event":"deleted","path":"{}"}}"#, path.display())
                            }
                        };

                        if ws_sender.send(Message::Text(message.into())).await.is_err() {
                            debug!("WebSocket client disconnected");
                            break;
                        }
                    }
                    None => {
                        // Channel closed
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket connection closed");
}
