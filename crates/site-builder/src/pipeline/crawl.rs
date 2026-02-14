//! Phase 1: Crawl the target website using Firecrawl API.

use crate::config::SiteBuilderConfig;
use crate::error::{SiteBuilderError, SiteBuilderResult};
use crate::firecrawl::types::CrawlData;
use crate::firecrawl::FirecrawlClient;

/// Run the crawl phase.
pub async fn run(url: &str, config: &SiteBuilderConfig) -> SiteBuilderResult<CrawlData> {
    let api_key = config
        .firecrawl_api_key
        .clone()
        .or_else(|| std::env::var("FIRECRAWL_API_KEY").ok())
        .ok_or_else(|| SiteBuilderError::ConfigError {
            reason: "FIRECRAWL_API_KEY not set. Set it via environment variable or appz.json."
                .to_string(),
        })?;

    let client = FirecrawlClient::new(api_key);
    let _ = ui::status::info("Phase 1: Crawling website...");
    let data = client.crawl_site(url).await?;

    let _ = ui::status::success(&format!(
        "Crawl complete: {} pages, branding {}",
        data.pages.len(),
        if data.branding.is_some() {
            "extracted"
        } else {
            "not available"
        }
    ));

    Ok(data)
}
