use crate::error::{DevServerError, Result};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::http::StatusCode;
use hyper::Request;
use multer::Multipart;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, error};

/// Handle form data processing
pub async fn handle_form_data(
    request: Request<Incoming>,
    upload_dir: PathBuf,
) -> Result<hyper::Response<Full<Bytes>>> {
    // Ensure upload directory exists
    fs::create_dir_all(&upload_dir).await?;

    let content_type = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    debug!("Processing form data with Content-Type: {}", content_type);

    if content_type.starts_with("multipart/form-data") {
        handle_multipart(request, upload_dir, &content_type).await
    } else if content_type == "application/x-www-form-urlencoded" {
        handle_urlencoded(request).await
    } else {
        Ok(hyper::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(Full::new(Bytes::from(
                json!({"error": "Unsupported Content-Type"}).to_string(),
            )))?)
    }
}

/// Handle multipart/form-data (file uploads)
async fn handle_multipart(
    request: Request<Incoming>,
    upload_dir: PathBuf,
    content_type: &str,
) -> Result<hyper::Response<Full<Bytes>>> {
    // Collect the body
    let body_bytes = http_body_util::BodyExt::collect(request.into_body())
        .await?
        .to_bytes();
    let body_vec = body_bytes.to_vec();

    // Create multipart parser
    let boundary = extract_boundary(content_type)?;
    let body_stream = futures_util::stream::once(async move { Ok::<_, std::io::Error>(body_vec) });
    let mut multipart = Multipart::new(body_stream, boundary);

    let mut fields = serde_json::Map::new();
    let mut files = Vec::new();

    // Parse multipart data
    while let Some(mut field) = multipart.next_field().await.map_err(|e| {
        error!("Multipart parsing error: {}", e);
        DevServerError::Multipart(e)
    })? {
        let field_name = field.name().unwrap_or("unknown").to_string();
        let filename = field.file_name().map(|s| s.to_string());

        // Read field data
        let mut data = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            error!("Error reading multipart field chunk: {}", e);
            DevServerError::Multipart(e)
        })? {
            data.extend_from_slice(&chunk);
        }

        if let Some(filename) = filename {
            // This is a file upload
            let file_path = upload_dir.join(&filename);
            fs::write(&file_path, &data).await?;
            files.push(json!({
                "name": filename,
                "field": field_name,
                "size": data.len(),
                "path": file_path.to_string_lossy(),
            }));
            debug!("Saved uploaded file: {}", file_path.display());
        } else {
            // This is a regular field
            let value = String::from_utf8_lossy(&data).to_string();
            fields.insert(field_name, json!(value));
        }
    }

    let response = json!({
        "success": true,
        "fields": fields,
        "files": files,
    });

    Ok(hyper::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Full::new(Bytes::from(response.to_string())))?)
}

/// Handle application/x-www-form-urlencoded
async fn handle_urlencoded(request: Request<Incoming>) -> Result<hyper::Response<Full<Bytes>>> {
    let body_bytes = http_body_util::BodyExt::collect(request.into_body())
        .await?
        .to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);

    let mut fields = serde_json::Map::new();

    // Parse URL-encoded data
    for pair in body_str.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let decoded_key = urlencoding::decode(key)
                .map_err(|e| DevServerError::Config(format!("Invalid key encoding: {}", e)))?;
            let decoded_value = urlencoding::decode(value)
                .map_err(|e| DevServerError::Config(format!("Invalid value encoding: {}", e)))?;
            fields.insert(decoded_key.to_string(), json!(decoded_value.to_string()));
        }
    }

    let response = json!({
        "success": true,
        "fields": fields,
        "files": [],
    });

    Ok(hyper::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Full::new(Bytes::from(response.to_string())))?)
}

/// Extract boundary from Content-Type header
fn extract_boundary(content_type: &str) -> Result<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if part.starts_with("boundary=") {
            return Ok(part[9..].trim_matches('"').to_string());
        }
    }
    Err(DevServerError::Config(
        "Missing boundary in Content-Type".to_string(),
    ))
}
