//! HTTP client for downloading files with retry logic and progress reporting

use grab::{download_to_path, DownloadOptions, GrabError};
use miette::{IntoDiagnostic, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{ClientBuilder, IntoUrl, Method, Response};
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock as Lazy;
use std::time::Duration;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tracing::debug;
use url::Url;

/// Default HTTP timeout (30 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default number of retries
const DEFAULT_RETRIES: usize = 3;

/// Global HTTP client instance
pub static HTTP: Lazy<Client> = Lazy::new(|| {
    // This should never fail in practice, but if it does, we'll get a panic at runtime
    // which is acceptable for a static initializer
    Client::new(DEFAULT_TIMEOUT).unwrap_or_else(|e| {
        panic!("Failed to create HTTP client: {}", e);
    })
});

#[derive(Debug)]
pub struct Client {
    reqwest: reqwest::Client,
    timeout: Duration,
}

impl Client {
    /// Create a new HTTP client with the specified timeout
    pub fn new(timeout: Duration) -> Result<Self> {
        Ok(Self {
            reqwest: Self::build_client()?
                .read_timeout(timeout)
                .connect_timeout(timeout)
                .build()
                .into_diagnostic()?,
            timeout,
        })
    }

    fn build_client() -> Result<ClientBuilder> {
        let version = env!("CARGO_PKG_VERSION");
        Ok(ClientBuilder::new()
            .user_agent(format!("appz/{}", version))
            .gzip(true)
            .zstd(true))
    }

    /// Download a file from a URL to a local path with progress reporting.
    /// Uses the grab crate (HEAD + single or parallel chunk download).
    pub async fn download_file<U: IntoUrl>(
        &self,
        url: U,
        path: &Path,
        pr: Option<Arc<dyn grab::Progress>>,
    ) -> Result<()> {
        let url = url.into_url().into_diagnostic()?;
        let headers = github_headers(&url);
        self.download_file_with_headers(url, path, &headers, pr)
            .await
    }

    /// Download a file with custom headers (uses grab crate).
    pub async fn download_file_with_headers<U: IntoUrl>(
        &self,
        url: U,
        path: &Path,
        headers: &HeaderMap,
        pr: Option<Arc<dyn grab::Progress>>,
    ) -> Result<()> {
        let url = url.into_url().into_diagnostic()?;
        debug!("Downloading {} to {}", &url, path.display());

        if let Some(parent) = path.parent() {
            starbase_utils::fs::create_dir_all(parent)
                .map_err(|e| miette::miette!("Failed to create directory: {}", e))?;
        }

        let version = env!("CARGO_PKG_VERSION");
        let options = DownloadOptions {
            timeout: self.timeout,
            user_agent: format!("appz/{}", version),
            parallel_threshold_bytes: 5 * 1024 * 1024,
            max_concurrent_chunks: 4,
            chunk_size: 1024 * 1024,
            resume: false,
            headers: Some(headers.clone()),
        };

        let url_str = url.to_string();
        let mut result =
            download_to_path(&url_str, path, options.clone(), pr.clone()).await;
        if let Err(GrabError::Request(ref e)) = result {
            if (e.is_connect() || e.is_timeout()) && url_str.starts_with("http://") {
                let https_url = url_str.replacen("http://", "https://", 1);
                result = download_to_path(&https_url, path, options, pr).await;
            }
        }
        result.map_err(|e| miette::miette!("Download failed: {}", e))?;

        Ok(())
    }

    /// Perform a GET request
    pub async fn get_async<U: IntoUrl>(&self, url: U) -> Result<Response> {
        let url = url.into_url().into_diagnostic()?;
        let headers = github_headers(&url);
        self.get_async_with_headers(url, &headers).await
    }

    /// Perform a GET request with custom headers
    async fn get_async_with_headers<U: IntoUrl>(
        &self,
        url: U,
        headers: &HeaderMap,
    ) -> Result<Response> {
        let url = url.into_url().into_diagnostic()?;
        let resp = self
            .send_with_https_fallback(Method::GET, url, headers, "GET")
            .await?;
        resp.error_for_status_ref().into_diagnostic()?;
        Ok(resp)
    }

    /// Send request with HTTPS fallback and retry logic
    async fn send_with_https_fallback(
        &self,
        method: Method,
        url: Url,
        headers: &HeaderMap,
        verb_label: &str,
    ) -> Result<Response> {
        Retry::spawn(default_backoff_strategy(DEFAULT_RETRIES), || {
            let method = method.clone();
            let url = url.clone();
            let headers = headers.clone();
            async move {
                match self
                    .send_once(method.clone(), url.clone(), &headers, verb_label)
                    .await
                {
                    Ok(resp) => Ok(resp),
                    Err(_err) if url.scheme() == "http" => {
                        // Try HTTPS if HTTP failed
                        let mut url = url;
                        url.set_scheme("https")
                            .map_err(|_| miette::miette!("Failed to set HTTPS scheme"))?;
                        self.send_once(method, url, &headers, verb_label).await
                    }
                    Err(err) => Err(err),
                }
            }
        })
        .await
    }

    /// Send a single HTTP request
    async fn send_once(
        &self,
        method: Method,
        url: Url,
        headers: &HeaderMap,
        verb_label: &str,
    ) -> Result<Response> {
        debug!("{} {}", verb_label, &url);
        let mut req = self.reqwest.request(method, url.clone());
        req = req.headers(headers.clone());

        let resp = match req.send().await {
            Ok(resp) => resp,
            Err(err) => {
                if err.is_timeout() {
                    return Err(miette::miette!(
                        "HTTP timed out after {:?} for {}",
                        self.timeout,
                        url
                    ));
                }
                return Err(miette::miette!("HTTP request failed: {}", err));
            }
        };

        debug!("{} {} {}", verb_label, url, resp.status());
        display_github_rate_limit(&resp);
        resp.error_for_status_ref().into_diagnostic()?;
        Ok(resp)
    }
}

/// Create GitHub API headers if needed
fn github_headers(url: &Url) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if url.host_str() == Some("api.github.com") {
        // Add GitHub API version header
        // HeaderValue::from_static returns HeaderValue directly, not Result
        let header_value = HeaderValue::from_static("2022-11-28");
        headers.insert("x-github-api-version", header_value);
        // Note: We don't add auth token here as we don't have GITHUB_TOKEN env var handling
        // This can be added later if needed
    }
    headers
}

/// Display GitHub rate limit warnings if applicable
fn display_github_rate_limit(resp: &Response) {
    let status = resp.status().as_u16();
    if status == 403 || status == 429 {
        let remaining = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|r| r.to_str().ok());
        if remaining.is_some_and(|r| r == "0") {
            if let Some(reset_time) = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            {
                tracing::warn!(
                    "GitHub rate limit exceeded. Resets at {}",
                    reset_time.with_timezone(&chrono::Local)
                );
            }
            return;
        }
        // Check retry-after header
        if let Some(retry_after) = resp
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        {
            tracing::warn!(
                "GitHub rate limit exceeded. Retry after {} seconds",
                retry_after
            );
        }
    }
}

/// Default exponential backoff strategy for retries
fn default_backoff_strategy(retries: usize) -> impl Iterator<Item = Duration> {
    ExponentialBackoff::from_millis(100)
        .map(jitter)
        .take(retries)
}
