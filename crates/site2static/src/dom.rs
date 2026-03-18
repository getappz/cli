use dashmap::DashSet;
use lol_html::{element, html_content::ContentType, rewrite_str, text, RewriteStrSettings};
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use url::Url;

static CSS_URL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"url\((['"]?)([^'")]+)(['"]?)\)"#).unwrap());

// Cache for hostname regex patterns to avoid per-page compilation
static REGEX_CACHE: LazyLock<Mutex<HashMap<String, Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get cached regex for host pattern to avoid per-page compilation
fn get_cached_regex(host: &str) -> Result<Regex, Box<dyn std::error::Error>> {
    let mut cache = REGEX_CACHE.lock().unwrap();

    if let Some(regex) = cache.get(host) {
        Ok(regex.clone())
    } else {
        let pattern = format!(
            r#"(?P<esc>\\*)(?:https?:\\?/\\?/|//){}(:\d+)?(?P<path>(?:\\?/[^\\s>"']*)?)"#,
            regex::escape(host)
        );

        let re = Regex::new(&pattern).map_err(|e| format!("Failed to compile regex: {}", e))?;

        cache.insert(host.to_string(), re.clone());
        Ok(re)
    }
}

/// Struct containing a DOM-like handler for a web page
pub struct Dom {
    pub html: String,
}

impl Dom {
    /// Create a new DOM from a string
    pub fn new(html: &str) -> Dom {
        Dom {
            html: html.to_string(),
        }
    }

    /// Serialize the DOM (returns original HTML as string)
    pub fn serialize(&self) -> String {
        self.html.clone()
    }

    /// Optimized version for rewriting URLs in HTML with minimal allocations.
    /// - Normal URLs -> normal path (/wp-includes/...).
    /// - Escaped URLs -> escaped path (\/wp-includes/...).
    pub fn rewrite_html_with_base_host(
        &mut self,
        base_url: &Url,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let host = base_url.host_str().ok_or("Invalid URL: missing host")?;

        // Use cached regex to avoid per-page compilation
        let re = get_cached_regex(host)?;

        // Use `String::with_capacity` to avoid multiple reallocations
        let mut result = String::with_capacity(self.html.len() + self.html.len() / 4);
        let mut last_end = 0;

        for caps in re.captures_iter(&self.html) {
            let m = caps.get(0).unwrap();
            result.push_str(&self.html[last_end..m.start()]);

            let path = caps.name("path").map_or("/", |m| m.as_str());
            let esc = caps.name("esc").map_or("", |m| m.as_str());

            if !esc.is_empty() || path.contains(r"\/") {
                // Escaped URL -> keep escaped format
                let path_unescaped = path.replace(r"\/", "/");
                let escaped_path = path_unescaped.replace('/', r"\/");
                result.push_str(&escaped_path);
            } else {
                // Normal URL -> normal path
                if !path.starts_with('/') {
                    result.push('/');
                }
                result.push_str(path);
            }

            last_end = m.end();
        }

        // Append remaining HTML after last match
        result.push_str(&self.html[last_end..]);

        // Update the HTML content
        self.html = result;

        Ok(())
    }

    /// Returns all URLs in the DOM tree and updates same-domain ones to relative paths.
    pub fn find_urls_as_strings(&mut self, base_url: &Url) -> Vec<String> {
        let captured_urls: Arc<DashSet<String>> = Arc::new(DashSet::new());
        let captured_for_closure = captured_urls.clone();
        let captured_for_styles = captured_urls.clone();
        let captured_for_inline_styles = captured_urls.clone();

        let base_domain = base_url.domain().unwrap_or_default().to_string();
        let base_domain_for_styles = base_domain.clone();

        let selectors = "img[src], script[src], source[src], video[src], audio[src], iframe[src], embed[src], object[data], picture[src], link[href], a[href]";
        // Buffer to accumulate text chunks (text can come in multiple chunks)
        let style_buffer = Rc::new(RefCell::new(String::new()));

        self.html = rewrite_str(
            &self.html,
            RewriteStrSettings {
                element_content_handlers: vec![
                    element!(selectors, move |el| {
                        // Pick the right attribute
                        let attr = match el.tag_name().as_str() {
                            "a" | "link" => "href",
                            "object" => "data",
                            _ => "src",
                        };

                        if let Some(mut local_url) = el.get_attribute(attr) {
                            // ignore wp-json urls
                            if local_url.contains("/wp-json/") {
                                return Ok(());
                            }

                            // Strip query strings and fragments
                            if let Some(pos) = local_url.find(['?', '#']) {
                                local_url.truncate(pos);
                            }

                            if !local_url.is_empty() {
                                captured_for_closure.insert(local_url.clone());
                            }

                            // Use Url for URL parsing and domain comparison
                            if let Ok(url) = Url::parse(&local_url) {
                                if url.domain() == Some(&base_domain) {
                                    if let Some(path) = url.path().strip_prefix('/') {
                                        let rel_path = format!("/{}", path);
                                        let _ = el.set_attribute(attr, &rel_path);
                                    }
                                }
                            }
                        }

                        Ok(())
                    }),
                    element!("*[style]", move |el| {
                        if let Some(style) = el.get_attribute("style") {
                            let new_style =
                                CSS_URL_REGEX.replace_all(&style, |caps: &regex::Captures| {
                                    let quote_start = &caps[1];
                                    let mut url_str = caps[2].to_string();
                                    let quote_end = &caps[3];

                                    captured_for_styles.insert(url_str.clone());

                                    // Use Url for URL parsing and domain comparison
                                    if let Ok(url) = Url::parse(&url_str) {
                                        if url.domain() == Some(&base_domain_for_styles) {
                                            if let Some(path) = url.path().strip_prefix('/') {
                                                url_str = format!("/{}", path);
                                            }
                                        }
                                    }
                                    format!("url({}{}{})", quote_start, url_str, quote_end)
                                });
                            el.set_attribute("style", &new_style).unwrap();
                        }
                        Ok(())
                    }),
                    text!("style", move |el_text| {
                        let mut style_buffer_b = style_buffer.borrow_mut();
                        style_buffer_b.push_str(el_text.as_str());
                        el_text.remove();
                        if el_text.last_in_text_node() {
                            *style_buffer_b = CSS_URL_REGEX
                                .replace_all(&style_buffer_b, |caps: &regex::Captures| {
                                    let quote_start = &caps[1];
                                    let url_str = caps[2].trim().to_string();
                                    let quote_end = &caps[3];
                                    captured_for_inline_styles.insert(url_str.clone());
                                    format!("url({}{}{})", quote_start, url_str, quote_end)
                                })
                                .to_string();
                            el_text.replace(&style_buffer_b, ContentType::Html);
                            style_buffer_b.clear();
                        }
                        Ok(())
                    }),
                ],
                ..RewriteStrSettings::default()
            },
        )
        .unwrap()
        .to_string();

        // After lol_html processing, rewrite any remaining hostname references
        if let Err(e) = self.rewrite_html_with_base_host(base_url) {
            eprintln!("Warning: Failed to rewrite hostname references: {}", e);
        }

        captured_urls.iter().map(|s| s.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_dom_urls() {
        let html = r#"<html><head><link href="style.css"><script src="script.js"></script></head><body><img src="image.png"><a href="https://example.com/page.html">Link</a></body></html>"#;
        let mut dom = Dom::new(html);
        let base_url = Url::parse("https://example.com/").unwrap();
        let urls = dom.find_urls_as_strings(&base_url);
        assert!(urls.contains(&"style.css".to_string()));
        assert!(urls.contains(&"script.js".to_string()));
        assert!(urls.contains(&"image.png".to_string()));
        assert!(urls.contains(&"https://example.com/page.html".to_string()));
    }

    #[test]
    fn test_css_url_extraction() {
        let html = r#"<html><head><style>.bg { background-image: url('image.png'); } .bg2 { background-image: url("image2.jpg"); } .bg3 { background-image: url(image3.gif); }</style></head><body></body></html>"#;
        let mut dom = Dom::new(html);
        let base_url = Url::parse("https://example.com/").unwrap();
        let urls = dom.find_urls_as_strings(&base_url);
        assert!(urls.contains(&"image.png".to_string()));
        assert!(urls.contains(&"image2.jpg".to_string()));
        assert!(urls.contains(&"image3.gif".to_string()));
    }

    #[test]
    fn test_css_url_replacement() {
        let html = r#"<html><head><style>.bg { background-image: url(https://example.com/image.png); } .bg2 { background-image: url(https://other.com/image2.jpg); }</style></head><body></body></html>"#;
        let mut dom = Dom::new(html);
        let base_url = Url::parse("https://example.com/").unwrap();
        let _urls = dom.find_urls_as_strings(&base_url);
        let serialized = dom.serialize();
        assert!(serialized.contains("url(/image.png)"));
        assert!(serialized.contains("url(https://other.com/image2.jpg)"));
    }

    #[test]
    fn test_hostname_rewriting() {
        let html = r#"<html><head><script>var apiUrl = "https://example.com/api/data";</script></head><body><div data-url="https://example.com/page">Content</div></body></html>"#;
        let mut dom = Dom::new(html);
        let base_url = Url::parse("https://example.com/").unwrap();
        let _urls = dom.find_urls_as_strings(&base_url);
        let serialized = dom.serialize();
        assert!(serialized.contains("/api/data"));
        assert!(serialized.contains("/page"));
        assert!(!serialized.contains("https://example.com"));
    }

    #[test]
    fn test_serialize() {
        let html = "<html><body></body></html>";
        let dom = Dom::new(html);
        assert_eq!(dom.serialize(), html);
    }
}
