use crate::error::Result;
use crate::watcher::FileChangeEvent;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderValue, UPGRADE};
use hyper::http::StatusCode;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::sync::broadcast;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

pub async fn handle_websocket_upgrade(
    mut request: Request<hyper::body::Incoming>,
    change_receiver: broadcast::Receiver<FileChangeEvent>,
) -> Result<Response<Full<Bytes>>> {
    if !request.headers().contains_key(UPGRADE) {
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Bad Request")))?);
    }

    let response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(UPGRADE, HeaderValue::from_static("websocket"))
        .header("Connection", HeaderValue::from_static("Upgrade"))
        .body(Full::new(Bytes::new()))?;

    let upgrade = hyper::upgrade::on(&mut request);
    tokio::spawn(async move {
        if let Ok(upgraded) = upgrade.await {
            handle_websocket_connection(upgraded, change_receiver).await;
        }
    });

    Ok(response)
}

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

    let _ = ws_sender
        .send(Message::Text(
            r#"{"type":"connected","message":"Hot reload enabled"}"#.into(),
        ))
        .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let mut change_receiver_clone = change_receiver;
    tokio::spawn(async move {
        while let Ok(event) = change_receiver_clone.recv().await {
            let _ = tx.send(event).await;
        }
    });

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sender.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = rx.recv() => {
                let Some(evt) = event else { break };
                let message = match evt {
                    FileChangeEvent::Created(path)
                    | FileChangeEvent::Modified(path)
                    | FileChangeEvent::Deleted(path) => {
                        format!(r#"{{"type":"reload","path":"{}"}}"#, path.display())
                    }
                };
                if ws_sender.send(Message::Text(message.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}
