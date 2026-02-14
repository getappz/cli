//! Firecrawl API client for website crawling and branding extraction.

pub mod types;

use crate::error::{SiteBuilderError, SiteBuilderResult};
use types::*;

/// Firecrawl cache maxAge in milliseconds.
///
/// If a cached version of the page is newer than this, Firecrawl returns
/// it instantly without re-scraping. This dramatically speeds up repeated
/// runs and saves credits.
///
/// Default: 30 days. Override via `FIRECRAWL_CACHE_DAYS` env var.
fn cache_max_age_ms() -> u64 {
    let days: u64 = std::env::var("FIRECRAWL_CACHE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    days * 24 * 60 * 60 * 1000
}

/// Firecrawl API client.
pub struct FirecrawlClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl FirecrawlClient {
    /// Create a new client with the given API key.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.firecrawl.dev".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Discover all URLs on a site using the map endpoint.
    pub async fn map(&self, url: &str) -> SiteBuilderResult<Vec<String>> {
        let body = serde_json::json!({ "url": url });
        let response = self
            .client
            .post(format!("{}/v1/map", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SiteBuilderError::CrawlFailed {
                reason: format!("Map request failed: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(SiteBuilderError::CrawlFailed {
                reason: format!("Map API status: {} - {}", status, body_text),
            });
        }

        let map_response: MapResponse =
            response
                .json()
                .await
                .map_err(|e| SiteBuilderError::CrawlFailed {
                    reason: format!("Failed to parse map response: {}", e),
                })?;

        Ok(map_response.links.unwrap_or_default())
    }

    /// Scrape a single page with the given formats.
    ///
    /// `max_age_ms` controls the Firecrawl cache: if a cached version of
    /// the page is newer than this value, Firecrawl returns it instantly
    /// instead of performing a fresh scrape.  `None` uses the Firecrawl
    /// default (2 days).
    pub async fn scrape(
        &self,
        url: &str,
        formats: &[&str],
        only_main_content: bool,
        max_age_ms: Option<u64>,
    ) -> SiteBuilderResult<ScrapeData> {
        let format_values: Vec<serde_json::Value> =
            formats.iter().map(|f| serde_json::json!(f)).collect();

        let mut body = serde_json::json!({
            "url": url,
            "formats": format_values,
            "onlyMainContent": only_main_content,
        });

        if let Some(age) = max_age_ms {
            body["maxAge"] = serde_json::json!(age);
        }

        let response = self
            .client
            .post(format!("{}/v1/scrape", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SiteBuilderError::CrawlFailed {
                reason: format!("Scrape request failed for {}: {}", url, e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(SiteBuilderError::CrawlFailed {
                reason: format!("Scrape API status for {}: {} - {}", url, status, body_text),
            });
        }

        let scrape_response: ScrapeResponse =
            response
                .json()
                .await
                .map_err(|e| SiteBuilderError::CrawlFailed {
                    reason: format!("Failed to parse scrape response for {}: {}", url, e),
                })?;

        scrape_response.data.ok_or_else(|| SiteBuilderError::CrawlFailed {
            reason: format!("Scrape returned no data for {}", url),
        })
    }

    /// Scrape the homepage with branding extraction.
    pub async fn scrape_homepage(&self, url: &str) -> SiteBuilderResult<ScrapeData> {
        self.scrape(url, &["markdown", "html", "links", "branding"], false, Some(cache_max_age_ms()))
            .await
    }

    /// Scrape a subpage for content only.
    pub async fn scrape_page(&self, url: &str) -> SiteBuilderResult<ScrapeData> {
        self.scrape(url, &["markdown", "html", "links"], true, Some(cache_max_age_ms())).await
    }

    /// Full crawl: map the site, then scrape each page.
    pub async fn crawl_site(&self, url: &str) -> SiteBuilderResult<CrawlData> {
        let _ = ui::status::info(&format!("Mapping site: {}", url));

        // Step 1: Discover all URLs
        let site_map = self.map(url).await?;
        let _ = ui::status::info(&format!("Found {} pages", site_map.len()));

        // Step 2: Scrape homepage with branding
        let _ = ui::status::info("Scraping homepage with branding data...");
        let homepage_data = self.scrape_homepage(url).await?;

        let branding = homepage_data.branding.clone();

        // Convert homepage to PageData
        let mut pages = vec![page_data_from_scrape(url, &homepage_data)];

        // Step 3: Scrape subpages (skip homepage)
        let subpages: Vec<&String> = site_map
            .iter()
            .filter(|u| u.as_str() != url && !u.ends_with('/') || u.as_str() != format!("{}/", url))
            .collect();

        if !subpages.is_empty() {
            let _ = ui::status::info(&format!("Scraping {} subpages...", subpages.len()));
        }

        for page_url in &subpages {
            match self.scrape_page(page_url).await {
                Ok(data) => {
                    pages.push(page_data_from_scrape(page_url, &data));
                }
                Err(e) => {
                    let _ = ui::status::warning(&format!("Skipping {}: {}", page_url, e));
                }
            }
        }

        let _ = ui::status::success(&format!(
            "Crawl complete: {} pages scraped",
            pages.len()
        ));

        Ok(CrawlData {
            site_url: url.to_string(),
            branding,
            pages,
            site_map,
        })
    }
}

/// Convert a scrape response to a PageData struct.
fn page_data_from_scrape(url: &str, data: &ScrapeData) -> PageData {
    PageData {
        url: url.to_string(),
        markdown: data.markdown.clone().unwrap_or_default(),
        html: data.html.clone().unwrap_or_default(),
        links: data.links.clone().unwrap_or_default(),
        title: data.metadata.as_ref().and_then(|m| m.title.clone()),
        meta_description: data.metadata.as_ref().and_then(|m| m.description.clone()),
    }
}
