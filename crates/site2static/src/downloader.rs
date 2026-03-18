use regex::Regex;
use reqwest::StatusCode;
use std::sync::LazyLock;
use url::Url;

use crate::response::{Response, ResponseData};

static DATA_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^.*(\b[a-z]+/[a-z\-+\.]+).*$").unwrap());
static CHARSET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^.*charset\s*=\s*["']?([^"'\s;]+).*$"#).unwrap());

fn is_html(content_type: &str) -> bool {
    content_type.contains("text/html")
}

fn is_css(content_type: &str) -> bool {
    content_type.contains("text/css")
}

pub struct Downloader {
    client: reqwest::blocking::Client,
    tries: usize,
}

impl Downloader {
    pub fn new(tries: usize) -> Self {
        let client = reqwest::blocking::ClientBuilder::new()
            .cookie_store(true)
            .user_agent("site2static/0.1")
            .danger_accept_invalid_certs(true) // local dev server
            .build()
            .expect("failed to build HTTP client");
        Self { client, tries }
    }

    pub fn get(&self, url: &Url) -> Result<Response, reqwest::Error> {
        self.get_conditional(url, None, None)
    }

    pub fn get_conditional(
        &self,
        url: &Url,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<Response, reqwest::Error> {
        let mut last_err = None;
        for _ in 0..self.tries {
            match self.make_request(url, etag, last_modified) {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap())
    }

    fn make_request(
        &self,
        url: &Url,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<Response, reqwest::Error> {
        let mut req = self.client.get(url.clone());
        if let Some(v) = etag {
            req = req.header("If-None-Match", v);
        } else if let Some(v) = last_modified {
            req = req.header("If-Modified-Since", v);
        }

        let resp = req.send()?;

        if resp.status() == StatusCode::NOT_MODIFIED {
            let resp_etag = resp
                .headers()
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            let resp_lm = resp
                .headers()
                .get("Last-Modified")
                .and_then(|v| v.to_str().ok())
                .map(String::from);
            return Ok(Response::not_modified(
                resp_etag.or_else(|| etag.map(String::from)),
                resp_lm.or_else(|| last_modified.map(String::from)),
            ));
        }

        let headers = resp.headers().clone();
        let etag_val = headers
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let lm_val = headers
            .get("Last-Modified")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let (data_type, charset) = match headers.get("content-type") {
            Some(ct) => {
                let ct_str = ct.to_str().unwrap_or("text/html");
                let dt = DATA_TYPE_RE
                    .captures(ct_str)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_lowercase())
                    .unwrap_or_else(|| "text/html".into());
                let cs = CHARSET_RE
                    .captures(ct_str)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_lowercase());
                (dt, cs)
            }
            None => ("text/html".into(), None),
        };

        let filename = if !is_html(&data_type) {
            headers
                .get("content-disposition")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.find('=').map(|i| s[i + 1..].to_string()))
        } else {
            None
        };

        // Capture final URL after redirects
        let final_url = Some(resp.url().clone());

        let raw = resp.bytes()?.to_vec();

        let response_data = if is_html(&data_type) {
            ResponseData::Html(raw)
        } else if is_css(&data_type) {
            ResponseData::Css(raw)
        } else {
            ResponseData::Other(raw)
        };

        Ok(Response::with_metadata(
            response_data,
            filename,
            charset,
            etag_val,
            lm_val,
            final_url,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_html_content_type() {
        assert!(is_html("text/html; charset=utf-8"));
        assert!(is_html("text/html"));
        assert!(!is_html("text/css"));
        assert!(!is_html("application/javascript"));
    }

    #[test]
    fn test_is_css_content_type() {
        assert!(is_css("text/css"));
        assert!(is_css("text/css; charset=utf-8"));
        assert!(!is_css("text/html"));
    }
}
