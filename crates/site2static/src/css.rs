use std::sync::LazyLock;

use regex::Regex;
use url::Url;

static CSS_URL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r#"url\s*\(\s*['"]?([^'"]*?)['"]?\s*\)"#).unwrap(),
        Regex::new(r#"@import\s+url\s*\(\s*['"]?([^'"]*?)['"]?\s*\)"#).unwrap(),
        Regex::new(r#"@import\s+'([^']+)'"#).unwrap(),
        Regex::new(r#"@import\s+"([^"]+)""#).unwrap(),
        Regex::new(r#"@import\s+([^'"\s;][^\s;]*)"#).unwrap(),
    ]
});

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
    pub fn find_urls_as_strings(&mut self, base_url: &Url) -> Vec<String> {
        let mut urls = Vec::new();
        let base_domain = base_url.domain().unwrap_or_default().to_string();

        for regex in CSS_URL_PATTERNS.iter() {
            // Collect URLs first
            for cap in regex.captures_iter(&self.css) {
                if let Some(m) = cap.get(1) {
                    let url = m.as_str().trim();
                    if !url.is_empty() && !url.starts_with("data:") {
                        urls.push(url.to_string());
                    }
                }
            }

            // Rewrite same-domain URLs
            self.css = regex
                .replace_all(&self.css, |caps: &regex::Captures| {
                    if let Some(m) = caps.get(1) {
                        let original = m.as_str().trim();
                        if !original.is_empty() && !original.starts_with("data:") {
                            let resolved = Url::parse(original)
                                .or_else(|_| base_url.join(original));
                            if let Ok(resolved) = resolved {
                                if resolved.domain() == Some(&*base_domain) {
                                    if let Some(path) = resolved.path().strip_prefix('/') {
                                        let rel = format!("/{}", path);
                                        let full = caps.get(0).unwrap().as_str();
                                        return full.replace(original, &rel);
                                    }
                                }
                            }
                        }
                    }
                    caps.get(0).unwrap().as_str().to_string()
                })
                .to_string();
        }
        urls
    }
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
