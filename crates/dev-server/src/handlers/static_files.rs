use crate::error::{DevServerError, Result};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderValue, CACHE_CONTROL, CONTENT_TYPE, LOCATION};
use hyper::http::StatusCode;
use hyper::Response;
use mime_guess::from_path;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error};

/// Handle static file requests
pub async fn handle_static_file(
    root_dir: &Path,
    request_path: &str,
    spa_fallback: bool,
    directory_listing: bool,
) -> Result<Response<Full<Bytes>>> {
    // Check for redirects first (SEO-friendly URLs)
    if let Some(redirect_path) = check_redirect(request_path) {
        debug!("Redirecting {} to {}", request_path, redirect_path);
        return Ok(create_redirect(&redirect_path));
    }

    // Normalize the path
    let path = normalize_path(root_dir, request_path)?;

    debug!("Serving file: {}", path.display());

    // Check if path exists
    let metadata = match fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Try fallback logic: demo.html or demo/index.html
            if let Some(fallback_path) = try_html_fallback(root_dir, request_path).await? {
                return serve_file(&fallback_path).await;
            }
            // Try SPA fallback (only for route-like paths, not missing assets)
            if spa_fallback && is_spa_navigation_path(request_path) {
                return handle_spa_fallback(root_dir).await;
            }
            return Ok(not_found());
        }
        Err(e) => {
            error!("Error reading file metadata: {}", e);
            return Ok(internal_error());
        }
    };

    // Handle directory
    if metadata.is_dir() {
        // Redirect trailing slash to non-trailing slash (e.g., /demo/ -> /demo)
        if request_path.ends_with('/') && request_path != "/" {
            let redirect_path = request_path.trim_end_matches('/');
            return Ok(create_redirect(redirect_path));
        }

        // For directories without trailing slash, try HTML fallback first
        // Try path.html first (e.g., demo.html), then path/index.html (e.g., demo/index.html)
        if let Some(fallback_path) = try_html_fallback(root_dir, request_path).await? {
            return serve_file(&fallback_path).await;
        }

        if directory_listing {
            return handle_directory_listing(&path, request_path).await;
        }
        // Try index.html in directory
        let index_path = path.join("index.html");
        if fs::metadata(&index_path).await.is_ok() {
            return serve_file(&index_path).await;
        }
        // SPA fallback: dir exists but no index.html (e.g. /dashboard with client-side routing)
        if spa_fallback && is_spa_navigation_path(request_path) {
            return handle_spa_fallback(root_dir).await;
        }
        return Ok(not_found());
    }

    // Serve the file
    serve_file(&path).await
}

/// Serve a single file
async fn serve_file(path: &Path) -> Result<Response<Full<Bytes>>> {
    let content = fs::read(path).await?;
    let content_len = content.len();
    let mime_type = from_path(path).first_or_octet_stream().to_string();

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_str(&mime_type)?)
        .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .body(Full::new(Bytes::from(content)))?;

    debug!(
        "Served file: {} ({} bytes, {})",
        path.display(),
        content_len,
        mime_type
    );

    Ok(response)
}

/// Handle SPA fallback (serve index.html for 404s)
async fn handle_spa_fallback(root_dir: &Path) -> Result<Response<Full<Bytes>>> {
    let index_path = root_dir.join("index.html");
    match fs::read(&index_path).await {
        Ok(content) => {
            let mime_type = "text/html";
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, HeaderValue::from_static(mime_type))
                .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
                .body(Full::new(Bytes::from(content)))?)
        }
        Err(_) => Ok(not_found()),
    }
}

/// Handle directory listing
async fn handle_directory_listing(
    dir_path: &Path,
    request_path: &str,
) -> Result<Response<Full<Bytes>>> {
    let mut entries = Vec::new();
    let mut read_dir = fs::read_dir(dir_path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let metadata = entry.metadata().await?;

        let entry_path = if request_path.ends_with('/') {
            format!("{}{}", request_path, file_name_str)
        } else {
            format!("{}/{}", request_path, file_name_str)
        };

        entries.push(format!(
            "<li><a href=\"{}\">{}</a> {}</li>",
            entry_path,
            file_name_str,
            if metadata.is_dir() { "[DIR]" } else { "" }
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Directory Listing: {}</title>
    <style>
        body {{ font-family: monospace; margin: 40px; }}
        ul {{ list-style: none; padding: 0; }}
        li {{ margin: 5px 0; }}
        a {{ text-decoration: none; color: #0066cc; }}
        a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <h1>Directory Listing: {}</h1>
    <ul>
        {}
    </ul>
</body>
</html>"#,
        request_path,
        request_path,
        entries.join("\n        ")
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/html"))
        .body(Full::new(Bytes::from(html)))?)
}

/// Normalize and validate the request path
fn normalize_path(root_dir: &Path, request_path: &str) -> Result<PathBuf> {
    // Remove leading slash and decode URL
    let path = request_path.trim_start_matches('/');
    let decoded = urlencoding::decode(path)
        .map_err(|e| DevServerError::Config(format!("Invalid URL encoding: {}", e)))?;

    // Build the full path
    let full_path = root_dir.join(decoded.as_ref());

    // Ensure the path is within root_dir (prevent directory traversal)
    if !full_path.starts_with(root_dir) {
        return Err(DevServerError::InvalidPath(format!(
            "Path outside root directory: {}",
            request_path
        )));
    }

    Ok(full_path)
}

/// Create a 404 Not Found response
fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Full::new(Bytes::from("404 Not Found")))
        .unwrap()
}

/// Create a 500 Internal Server Error response
fn internal_error() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Full::new(Bytes::from("500 Internal Server Error")))
        .unwrap()
}

/// Check if the request path should be redirected to a SEO-friendly URL
/// Returns Some(redirect_path) if redirect is needed, None otherwise
fn check_redirect(request_path: &str) -> Option<String> {
    // /index.html -> /
    if request_path == "/index.html" {
        return Some("/".to_string());
    }

    // /demo.html -> /demo
    if request_path.ends_with(".html") && request_path != "/index.html" {
        let without_ext = request_path.trim_end_matches(".html");
        return Some(without_ext.to_string());
    }

    // /demo/index.html -> /demo
    if request_path.ends_with("/index.html") {
        let without_index = request_path.trim_end_matches("/index.html");
        return Some(without_index.to_string());
    }

    None
}

/// Create a 301 permanent redirect response
fn create_redirect(location: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            LOCATION,
            HeaderValue::from_str(location).unwrap_or_else(|_| HeaderValue::from_static("/")),
        )
        .body(Full::new(Bytes::new()))
        .unwrap()
}

/// Check if path looks like a SPA route (no file extension) vs a static asset request
fn is_spa_navigation_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    !path.is_empty() && !path.contains('.')
}

/// Try HTML fallback logic: try demo.html first, then demo/index.html
async fn try_html_fallback(root_dir: &Path, request_path: &str) -> Result<Option<PathBuf>> {
    // Remove leading slash
    let path = request_path.trim_start_matches('/');

    // Skip if path is empty or already has an extension
    if path.is_empty() || path.contains('.') {
        return Ok(None);
    }

    // Try path.html first (e.g., demo.html)
    let html_path = root_dir.join(format!("{}.html", path));
    if fs::metadata(&html_path).await.is_ok() {
        debug!("Found fallback file: {}", html_path.display());
        return Ok(Some(html_path));
    }

    // Try path/index.html (e.g., demo/index.html)
    let index_path = root_dir.join(path).join("index.html");
    if fs::metadata(&index_path).await.is_ok() {
        debug!("Found fallback file: {}", index_path.display());
        return Ok(Some(index_path));
    }

    Ok(None)
}
