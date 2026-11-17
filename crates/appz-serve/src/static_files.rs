use crate::error::{Result, ServeError};
use crate::file_cache;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue, LOCATION};
use hyper::http::StatusCode;
use hyper::{Method, Response};
use mime_guess::from_path;
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn handle_static_file(
    root_dir: &Path,
    request_path: &str,
    method: &Method,
    spa_fallback: bool,
) -> Result<Response<Full<Bytes>>> {
    if let Some(redirect_path) = check_redirect(request_path) {
        return Ok(create_redirect(&redirect_path));
    }

    let path = normalize_path(root_dir, request_path)?;

    let metadata = match fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(fallback_path) = try_html_fallback(root_dir, request_path).await? {
                return respond_path(&fallback_path, method).await;
            }
            if spa_fallback && is_spa_navigation_path(request_path) {
                return handle_spa_fallback(root_dir, method).await;
            }
            return Ok(not_found());
        }
        Err(_) => return Ok(internal_error()),
    };

    if metadata.is_dir() {
        if request_path.ends_with('/') && request_path != "/" {
            let redirect_path = request_path.trim_end_matches('/');
            return Ok(create_redirect(redirect_path));
        }

        if let Some(fallback_path) = try_html_fallback(root_dir, request_path).await? {
            return respond_path(&fallback_path, method).await;
        }

        let index_path = path.join("index.html");
        if fs::metadata(&index_path).await.is_ok() {
            return respond_path(&index_path, method).await;
        }

        if spa_fallback && is_spa_navigation_path(request_path) {
            return handle_spa_fallback(root_dir, method).await;
        }
        return Ok(not_found());
    }

    respond_path(&path, method).await
}

async fn respond_path(path: &Path, method: &Method) -> Result<Response<Full<Bytes>>> {
    if *method == Method::HEAD {
        return head_file(path).await;
    }
    serve_file(path).await
}

async fn serve_file(path: &Path) -> Result<Response<Full<Bytes>>> {
    let mime_type = from_path(path).first_or_octet_stream();
    let content_type = mime_type.as_ref().to_string();
    let path = path.to_path_buf();
    let content = tokio::task::spawn_blocking(move || file_cache::read_bytes(&path))
        .await
        .map_err(|_| ServeError::Io(std::io::Error::other("read task cancelled")))?
        .map_err(ServeError::Io)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_str(&content_type)?)
        .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .body(Full::new(content))?)
}

async fn head_file(path: &Path) -> Result<Response<Full<Bytes>>> {
    let metadata = fs::metadata(path).await?;
    let mime_type = from_path(path).first_or_octet_stream();
    let len = metadata.len();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_str(mime_type.as_ref())?)
        .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .header(
            hyper::header::CONTENT_LENGTH,
            HeaderValue::from_str(&len.to_string())?,
        )
        .body(Full::new(Bytes::new()))?)
}

async fn handle_spa_fallback(root_dir: &Path, method: &Method) -> Result<Response<Full<Bytes>>> {
    let index_path = root_dir.join("index.html");
    respond_path(&index_path, method).await.or(Ok(not_found()))
}

fn normalize_path(root_dir: &Path, request_path: &str) -> Result<PathBuf> {
    let path = request_path.trim_start_matches('/');
    let decoded = urlencoding::decode(path)
        .map_err(|e| ServeError::Config(format!("Invalid URL encoding: {e}")))?;
    let full_path = root_dir.join(decoded.as_ref());
    if !full_path.starts_with(root_dir) {
        return Err(ServeError::InvalidPath(format!(
            "Path outside root directory: {request_path}"
        )));
    }
    Ok(full_path)
}

pub(crate) fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Full::new(Bytes::from("404 Not Found")))
        .expect("static 404 response")
}

pub(crate) fn internal_error() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Full::new(Bytes::from("500 Internal Server Error")))
        .expect("static 500 response")
}

fn check_redirect(request_path: &str) -> Option<String> {
    if request_path == "/index.html" {
        return Some("/".to_string());
    }
    if request_path.ends_with(".html") && request_path != "/index.html" {
        return Some(request_path.trim_end_matches(".html").to_string());
    }
    if request_path.ends_with("/index.html") {
        return Some(request_path.trim_end_matches("/index.html").to_string());
    }
    None
}

fn create_redirect(location: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            LOCATION,
            HeaderValue::from_str(location).unwrap_or_else(|_| HeaderValue::from_static("/")),
        )
        .body(Full::new(Bytes::new()))
        .expect("redirect response")
}

fn is_spa_navigation_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    !path.is_empty() && !path.contains('.')
}

async fn try_html_fallback(root_dir: &Path, request_path: &str) -> Result<Option<PathBuf>> {
    let path = request_path.trim_start_matches('/');
    if path.is_empty() || path.contains('.') {
        return Ok(None);
    }

    let html_path = root_dir.join(format!("{path}.html"));
    if fs::metadata(&html_path).await.is_ok() {
        return Ok(Some(html_path));
    }

    let index_path = root_dir.join(path).join("index.html");
    if fs::metadata(&index_path).await.is_ok() {
        return Ok(Some(index_path));
    }

    Ok(None)
}
