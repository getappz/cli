use std::fs;
use std::path::PathBuf;
use url::Url;
use crate::url_utils;
use crate::WebRoot;

/// Convert a URL to a relative filesystem path for the output directory.
pub fn url_to_path(url: &Url) -> String {
    let url_path = url_utils::without_leading_slash(url.path());
    if url_path.is_empty() {
        return "index.html".into();
    }
    let trimmed = url_utils::without_trailing_slash(url_path);
    if trimmed.is_empty() {
        return "index.html".into();
    }
    let lower = trimmed.to_lowercase();
    let is_html_like = !lower.contains('.') || lower.ends_with(".html") || lower.ends_with(".htm");
    if is_html_like {
        if trimmed.ends_with(".html") || trimmed.ends_with(".htm") {
            trimmed
        } else {
            format!("{}/index.html", trimmed)
        }
    } else {
        trimmed
    }
}

/// Resolve a relative path against the WebRoot. Returns `Some(path)` if the file exists locally.
pub fn resolve_local_path(webroot: &WebRoot, relative_path: &str) -> Option<PathBuf> {
    match webroot {
        WebRoot::Direct(root) => {
            let full = root.join(relative_path);
            if full.exists() { Some(full) } else { None }
        }
        WebRoot::Search(roots) => {
            for root in roots {
                let full = root.join(relative_path);
                if full.exists() { return Some(full); }
            }
            None
        }
    }
}

/// Read a file from the local filesystem based on URL and WebRoot.
pub fn read_local_file(webroot: &WebRoot, url: &Url) -> Result<Vec<u8>, std::io::Error> {
    let relative = url_utils::without_leading_slash(url.path());
    match resolve_local_path(webroot, relative) {
        Some(path) => fs::read(&path),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found locally for URL: {}", url),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_url_to_path_root() {
        let url = Url::parse("http://localhost:8080/").unwrap();
        assert_eq!(url_to_path(&url), "index.html");
    }

    #[test]
    fn test_url_to_path_directory() {
        let url = Url::parse("http://localhost:8080/about/").unwrap();
        assert_eq!(url_to_path(&url), "about/index.html");
    }

    #[test]
    fn test_url_to_path_no_extension() {
        let url = Url::parse("http://localhost:8080/about").unwrap();
        assert_eq!(url_to_path(&url), "about/index.html");
    }

    #[test]
    fn test_url_to_path_asset() {
        let url = Url::parse("http://localhost:8080/assets/app.js").unwrap();
        assert_eq!(url_to_path(&url), "assets/app.js");
    }

    #[test]
    fn test_url_to_path_html_file() {
        let url = Url::parse("http://localhost:8080/page.html").unwrap();
        assert_eq!(url_to_path(&url), "page.html");
    }

    #[test]
    fn test_url_to_path_bare_root() {
        let url = Url::parse("http://localhost:8080").unwrap();
        assert_eq!(url_to_path(&url), "index.html");
    }

    #[test]
    fn test_resolve_direct_webroot() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets/style.css"), "body{}").unwrap();
        let webroot = crate::WebRoot::Direct(dir.path().to_path_buf());
        let result = resolve_local_path(&webroot, "assets/style.css");
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_resolve_direct_webroot_missing() {
        let dir = TempDir::new().unwrap();
        let webroot = crate::WebRoot::Direct(dir.path().to_path_buf());
        let result = resolve_local_path(&webroot, "missing.css");
        assert!(result.is_none());
    }
}
