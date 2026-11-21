use crate::error::{Result, ServeError};
use crate::file_cache;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue, LOCATION};
use hyper::http::StatusCode;
use hyper::{Method, Response};
use mime_guess::from_path;
use std::path::{Component, Path, PathBuf};
use tokio::fs;

pub async fn handle_static_file(
    root_dir: &Path,
    request_path: &str,
    method: &Method,
    spa_fallback: bool,
) -> Result<Response<Full<Bytes>>> {
    let canonical_root = fs::canonicalize(root_dir).await.map_err(ServeError::Io)?;
    let candidate = resolve_under_root(&canonical_root, request_path)?;

    if let Some(redirect_path) = check_redirect(request_path) {
        return Ok(create_redirect(&redirect_path));
    }

    let path = match verify_existing_under_root(&canonical_root, &candidate).await? {
        Some(p) => p,
        None => {
            if let Some(fallback_path) =
                try_html_fallback(&canonical_root, &candidate, request_path).await?
            {
                return respond_path(&fallback_path, method).await;
            }
            if spa_fallback && is_spa_navigation_path(request_path) {
                return handle_spa_fallback(&canonical_root, method).await;
            }
            return Ok(not_found());
        }
    };

    let metadata = match fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(fallback_path) =
                try_html_fallback(&canonical_root, &candidate, request_path).await?
            {
                return respond_path(&fallback_path, method).await;
            }
            if spa_fallback && is_spa_navigation_path(request_path) {
                return handle_spa_fallback(&canonical_root, method).await;
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

        if let Some(fallback_path) =
            try_html_fallback(&canonical_root, &candidate, request_path).await?
        {
            return respond_path(&fallback_path, method).await;
        }

        let index_candidate = path.join("index.html");
        if let Some(index_path) =
            verify_existing_under_root(&canonical_root, &index_candidate).await?
        {
            return respond_path(&index_path, method).await;
        }

        if spa_fallback && is_spa_navigation_path(request_path) {
            return handle_spa_fallback(&canonical_root, method).await;
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
    let path = path.to_path_buf();
    let content = tokio::task::spawn_blocking(move || file_cache::read_bytes(&path))
        .await
        .map_err(|_| ServeError::Io(std::io::Error::other("read task cancelled")))?
        .map_err(ServeError::Io)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_str(mime_type.as_ref())?)
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

async fn handle_spa_fallback(
    canonical_root: &Path,
    method: &Method,
) -> Result<Response<Full<Bytes>>> {
    let candidate = canonical_root.join("index.html");
    let Some(index_path) = verify_existing_under_root(canonical_root, &candidate).await? else {
        return Ok(not_found());
    };
    respond_path(&index_path, method).await.or(Ok(not_found()))
}

fn resolve_under_root(canonical_root: &Path, request_path: &str) -> Result<PathBuf> {
    let path = request_path.trim_start_matches('/');
    let decoded = urlencoding::decode(path)
        .map_err(|e| ServeError::Config(format!("Invalid URL encoding: {e}")))?;
    if decoded.contains('\0') || decoded.contains('\\') {
        return Err(ServeError::InvalidPath(format!(
            "Invalid path characters: {request_path}"
        )));
    }
    let mut resolved = canonical_root.to_path_buf();
    for component in Path::new(decoded.as_ref()).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ServeError::InvalidPath(format!(
                    "Path outside root directory: {request_path}"
                )));
            }
        }
    }
    Ok(resolved)
}

async fn verify_existing_under_root(
    canonical_root: &Path,
    candidate: &Path,
) -> Result<Option<PathBuf>> {
    match fs::canonicalize(candidate).await {
        Ok(real) => {
            if real.starts_with(canonical_root) {
                Ok(Some(real))
            } else {
                Err(ServeError::InvalidPath(format!(
                    "Path outside root directory: {}",
                    candidate.display()
                )))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ServeError::Io(e)),
    }
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
    let safe_location = |loc: &str| {
        let collapsed = loc.trim_start_matches('/');
        if collapsed.is_empty() {
            "/".to_string()
        } else {
            format!("/{collapsed}")
        }
    };
    if request_path == "/index.html" {
        return Some("/".to_string());
    }
    if request_path.ends_with("/index.html") {
        return Some(safe_location(request_path.trim_end_matches("/index.html")));
    }
    if request_path.ends_with(".html") {
        return Some(safe_location(request_path.trim_end_matches(".html")));
    }
    None
}

fn create_redirect(location: &str) -> Response<Full<Bytes>> {
    let collapsed = location.trim_start_matches('/');
    let normalized = if collapsed.is_empty() {
        "/".to_string()
    } else {
        format!("/{collapsed}")
    };
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            LOCATION,
            HeaderValue::from_str(&normalized).unwrap_or_else(|_| HeaderValue::from_static("/")),
        )
        .body(Full::new(Bytes::new()))
        .expect("redirect response")
}

fn is_spa_navigation_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    !path.is_empty() && !path.contains('.')
}

async fn try_html_fallback(
    canonical_root: &Path,
    candidate: &Path,
    request_path: &str,
) -> Result<Option<PathBuf>> {
    let path = request_path.trim_start_matches('/');
    if path.is_empty() || path.contains('.') {
        return Ok(None);
    }

    if let Some(name) = candidate.file_name().and_then(|n| n.to_str()) {
        let html_candidate = candidate.with_file_name(format!("{name}.html"));
        if let Some(real) = verify_existing_under_root(canonical_root, &html_candidate).await? {
            return Ok(Some(real));
        }
    }

    let index_candidate = candidate.join("index.html");
    verify_existing_under_root(canonical_root, &index_candidate).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn make_temp_pair() -> (PathBuf, PathBuf) {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("appz-serve-sec-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("site");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("index.html"), "<h1>home</h1>").unwrap();
        std::fs::write(root.join("about.html"), "<h1>about</h1>").unwrap();
        std::fs::write(outside.join("secret.txt"), "top-secret").unwrap();
        (base, root)
    }

    #[tokio::test]
    async fn blocks_lexical_and_encoded_path_traversal() {
        let (base, root) = make_temp_pair();
        for bad in [
            "/../outside/secret.txt",
            "/%2e%2e/outside/secret.txt",
            "/%2e%2e%2foutside%2fsecret.txt",
            "/..%5coutside%5csecret.txt",
            "/sub/../../outside/secret.txt",
        ] {
            let res = handle_static_file(&root, bad, &Method::GET, false).await;
            assert!(
                matches!(res, Err(ServeError::InvalidPath(_))),
                "expected InvalidPath for {bad}, got {res:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn blocks_symlink_escaping_root() {
        let (base, root) = make_temp_pair();
        let outside_secret = base.join("outside").join("secret.txt");
        let link = root.join("leak.txt");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&outside_secret, &link).is_ok();
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_file(&outside_secret, &link).is_ok();

        if linked {
            let res = handle_static_file(&root, "/leak.txt", &Method::GET, false).await;
            assert!(
                matches!(res, Err(ServeError::InvalidPath(_))),
                "symlink outside root must be rejected, got {res:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn redirect_collapses_protocol_relative_paths() {
        let (base, root) = make_temp_pair();
        let resp = handle_static_file(&root, "//evil.com/index.html", &Method::GET, false)
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
        let loc = resp.headers().get(LOCATION).unwrap().to_str().unwrap();
        assert!(!loc.starts_with("//"), "open redirect location: {loc}");
        assert_eq!(loc, "/evil.com");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn serves_valid_files_and_clean_html_fallback() {
        let (base, root) = make_temp_pair();
        let ok_index = handle_static_file(&root, "/", &Method::GET, false)
            .await
            .unwrap();
        assert_eq!(ok_index.status(), StatusCode::OK);

        let ok_about = handle_static_file(&root, "/about", &Method::GET, false)
            .await
            .unwrap();
        assert_eq!(ok_about.status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(&base);
    }
}
