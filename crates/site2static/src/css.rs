use url::Url;

pub struct Css {
    pub css: String,
}

impl Css {
    pub fn new(css: &str) -> Self {
        Self {
            css: css.to_string(),
        }
    }

    pub fn serialize(&self) -> String {
        self.css.clone()
    }

    /// Extract URLs from CSS and rewrite same-domain ones to absolute paths.
    /// Uses `css_refs` for URL extraction, then rewrites in place.
    pub fn find_urls_as_strings(&mut self, base_url: &Url) -> Vec<String> {
        let base_domain = base_url.domain().unwrap_or_default().to_string();

        // Extract all URLs using css-refs (zero-copy extraction)
        let mut urls: Vec<String> = css_refs::extract_urls(&self.css)
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        // Also extract @import URLs (css_refs separates these)
        for import_url in css_refs::extract_imports(&self.css) {
            let s = import_url.to_string();
            if !urls.contains(&s) {
                urls.push(s);
            }
        }

        // Rewrite same-domain absolute URLs to root-relative paths.
        // Uses the raw regex from css-refs for in-place replacement.
        let re = css_refs::url_regex();
        self.css = re
            .replace_all(&self.css, |caps: &regex::Captures| {
                rewrite_if_same_domain(caps, base_url, &base_domain)
            })
            .to_string();

        let re_import = css_refs::import_regex();
        self.css = re_import
            .replace_all(&self.css, |caps: &regex::Captures| {
                rewrite_if_same_domain(caps, base_url, &base_domain)
            })
            .to_string();

        urls
    }
}

/// Rewrite a captured URL to a root-relative path if it's on the same domain.
fn rewrite_if_same_domain(
    caps: &regex::Captures,
    base_url: &Url,
    base_domain: &str,
) -> String {
    // Try capture group 1 first, then 2 (import patterns have two groups)
    let m = caps.get(1).or_else(|| caps.get(2));
    if let Some(m) = m {
        let original = m.as_str().trim();
        if !original.is_empty() && !original.starts_with("data:") {
            let resolved = Url::parse(original).or_else(|_| base_url.join(original));
            if let Ok(resolved) = resolved {
                if resolved.domain() == Some(base_domain) {
                    if let Some(path) = resolved.path().strip_prefix('/') {
                        let rel = format!("/{path}");
                        let full = caps.get(0).unwrap().as_str();
                        return full.replace(original, &rel);
                    }
                }
            }
        }
    }
    caps.get(0).unwrap().as_str().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_css_urls() {
        let css = r#"
            body {
                background-image: url('background.jpg');
                background: url("header.png");
            }
            .icon {
                background: url(data:image/png;base64,iVBORw0KGgo);
            }
            @import url('https://fonts.googleapis.com/css?family=Roboto');
            @import 'styles.css';
            .logo {
                content: url(logo.svg);
            }
        "#;
        let mut css_handler = Css::new(css);
        let base_url = Url::parse("https://example.com/").unwrap();
        let urls = css_handler.find_urls_as_strings(&base_url);
        assert!(urls.contains(&"background.jpg".to_string()));
        assert!(urls.contains(&"header.png".to_string()));
        assert!(urls.contains(&"https://fonts.googleapis.com/css?family=Roboto".to_string()));
        assert!(urls.contains(&"styles.css".to_string()));
        assert!(urls.contains(&"logo.svg".to_string()));
        assert!(!urls.iter().any(|url: &String| url.starts_with("data:")));
    }

    #[test]
    fn test_css_relative_path_conversion() {
        let css = r#"
            body {
                background: url('https://example.com/images/bg.jpg');
                background: url("https://example.com/css/style.css");
            }
            @import 'https://example.com/fonts/font.css';
        "#;
        let mut css_handler = Css::new(css);
        let base_url = Url::parse("https://example.com/").unwrap();
        let _urls = css_handler.find_urls_as_strings(&base_url);
        let result = css_handler.serialize();
        assert!(result.contains("url('/images/bg.jpg')"));
        assert!(result.contains("url(\"/css/style.css\")"));
        assert!(result.contains("@import '/fonts/font.css'"));
    }

    #[test]
    fn test_serialize() {
        let css = "body { color: red; }";
        let css_handler = Css::new(css);
        assert_eq!(css_handler.serialize(), css);
    }
}
