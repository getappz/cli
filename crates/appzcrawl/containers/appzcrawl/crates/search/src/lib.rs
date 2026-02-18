/**
 * Search implementations: DuckDuckGo and SearXNG.
 * Adapted from firecrawl/apps/api/src/search/v2/ddgsearch.ts and searxng.ts.
 */

use rand::Rng;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchOptions {
    pub tbs: Option<String>,
    pub filter: Option<String>,
    pub lang: Option<String>,
    pub country: Option<String>,
    pub location: Option<String>,
    pub num_results: usize,
    #[serde(default)]
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebSearchResult {
    pub url: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<Vec<WebSearchResult>>,
}

// ---------------------------------------------------------------------------
// DuckDuckGo Search
// Adapted from firecrawl/apps/api/src/search/v2/ddgsearch.ts
// ---------------------------------------------------------------------------

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:120.0) Gecko/20100101 Firefox/120.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
];

fn random_user_agent() -> &'static str {
    let mut rng = rand::thread_rng();
    USER_AGENTS[rng.gen_range(0..USER_AGENTS.len())]
}

fn clean_ddg_url(href: &str) -> String {
    if href.contains("uddg=") {
        if let Ok(url) = url::Url::parse(href) {
            if let Some(uddg) = url.query_pairs().find(|(k, _)| k == "uddg") {
                return urlencoding::decode(&uddg.1).unwrap_or(href.into()).into_owned();
            }
        }
    }
    href.to_string()
}

fn extract_ddg_results(html: &str, seen_urls: &mut HashSet<String>) -> Result<Vec<WebSearchResult>, String> {
    let document = Html::parse_document(html);
    
    // Check for anti-bot modal
    let anomaly_selector = Selector::parse(".anomaly-modal__modal").unwrap();
    if document.select(&anomaly_selector).next().is_some() {
        return Err("DDG_ANTIBOT".to_string());
    }

    let mut results = Vec::new();
    let result_selector = Selector::parse(".result.web-result").unwrap();
    let title_selector = Selector::parse(".result__a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();

    for block in document.select(&result_selector) {
        let title_link = block.select(&title_selector).next();
        let snippet = block.select(&snippet_selector).next();

        if let (Some(link), Some(desc)) = (title_link, snippet) {
            if let Some(href) = link.value().attr("href") {
                let raw_url = href.trim();
                let title = link.text().collect::<String>().trim().to_string();
                let description = desc.text().collect::<String>().trim().to_string();

                if !raw_url.is_empty() && !title.is_empty() && !description.is_empty() {
                    let url = clean_ddg_url(raw_url);
                    if !seen_urls.contains(&url) {
                        seen_urls.insert(url.clone());
                        results.push(WebSearchResult {
                            url,
                            title,
                            description,
                        });
                    }
                }
            }
        }
    }

    Ok(results)
}

fn get_next_page_params(html: &str) -> Option<Vec<(String, String)>> {
    let document = Html::parse_document(html);
    
    // Find all forms and check if they contain the Next button
    let form_selector = Selector::parse("form").unwrap();
    let next_selector = Selector::parse(r#"input[type="submit"][value="Next"]"#).unwrap();
    let input_selector = Selector::parse("input").unwrap();
    
    for form in document.select(&form_selector) {
        // Check if this form contains a Next button
        let has_next = form.select(&next_selector).next().is_some();
        
        if has_next {
            // Extract all form inputs
            let mut params = Vec::new();
            
            for input_ref in form.select(&input_selector) {
                let element = input_ref.value();
                if let (Some(name), Some(value)) = (element.attr("name"), element.attr("value")) {
                    params.push((name.to_string(), value.to_string()));
                }
            }
            
            if !params.is_empty() {
                return Some(params);
            }
        }
    }

    None
}

pub async fn ddg_search(
    query: &str,
    num_results: usize,
    options: &SearchOptions,
) -> Result<SearchResponse, String> {
    let lang = options.lang.as_deref().unwrap_or("en");
    let country = options.country.as_deref().unwrap_or("us");
    let timeout = Duration::from_millis(options.timeout_ms);

    let client = Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let user_agent = random_user_agent();
    let mut params: Vec<(String, String)> = vec![
        ("q".to_string(), query.to_string()),
        ("kp".to_string(), "1".to_string()),
    ];

    if let Some(location) = &options.location {
        params.push(("kl".to_string(), location.clone()));
    } else {
        params.push(("kl".to_string(), format!("{}-{}", country.to_lowercase(), lang.to_lowercase())));
    }

    // Handle time-based search (tbs)
    if let Some(tbs) = &options.tbs {
        if tbs == "d" || tbs == "w" || tbs == "m" || tbs == "y" || tbs.contains("..") {
            params.push(("df".to_string(), tbs.clone()));
        }
    }

    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let mut is_first_page = true;
    let mut antibot_retries = 0;
    let max_antibot_retries = 3;

    while results.len() < num_results && antibot_retries <= max_antibot_retries {
        let response = if is_first_page {
            client
                .get("https://html.duckduckgo.com/html")
                .query(&params)
                .header("User-Agent", user_agent)
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "en-US,en;q=0.5")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Upgrade-Insecure-Requests", "1")
                .send()
                .await
        } else {
            client
                .post("https://html.duckduckgo.com/html")
                .form(&params)
                .header("User-Agent", user_agent)
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "en-US,en;q=0.5")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Upgrade-Insecure-Requests", "1")
                .send()
                .await
        };

        let response = match response {
            Ok(r) => r,
            Err(e) => return Err(format!("HTTP request failed: {}", e)),
        };

        let html = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        match extract_ddg_results(&html, &mut seen_urls) {
            Ok(new_results) => {
                is_first_page = false;
                antibot_retries = 0;

                if new_results.is_empty() {
                    break;
                }

                results.extend(new_results);

                if results.len() >= num_results {
                    break;
                }

                // Try to get next page params
                if let Some(next_params) = get_next_page_params(&html) {
                    params = next_params;
                } else {
                    break;
                }

                // Delay between pages
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
            Err(e) if e == "DDG_ANTIBOT" => {
                antibot_retries += 1;
                if antibot_retries > max_antibot_retries {
                    return Err("DuckDuckGo: Blocked by anti-bot measures".to_string());
                }
                eprintln!("DuckDuckGo: Anti-bot detected, retrying... (attempt {})", antibot_retries);
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
            Err(e) => return Err(e),
        }
    }

    if results.is_empty() {
        return Ok(SearchResponse { web: None });
    }

    Ok(SearchResponse {
        web: Some(results.into_iter().take(num_results).collect()),
    })
}

// ---------------------------------------------------------------------------
// SearXNG Search
// Adapted from firecrawl/apps/api/src/search/v2/searxng.ts
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    results: Vec<SearxngResult>,
}

#[derive(Debug, Deserialize)]
struct SearxngResult {
    url: String,
    title: String,
    content: String,
}

pub async fn searxng_search(
    query: &str,
    searxng_endpoint: &str,
    options: &SearchOptions,
) -> Result<SearchResponse, String> {
    let num_results = options.num_results;
    let results_per_page = 20;
    let pages_to_fetch = ((num_results + results_per_page - 1) / results_per_page).max(1);

    let timeout = Duration::from_millis(options.timeout_ms);
    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let base_url = if searxng_endpoint.ends_with('/') {
        &searxng_endpoint[..searxng_endpoint.len() - 1]
    } else {
        searxng_endpoint
    };
    let search_url = format!("{}/search", base_url);

    let mut all_results = Vec::new();

    for page in 1..=pages_to_fetch {
        let mut params = vec![
            ("q", query.to_string()),
            ("pageno", page.to_string()),
            ("format", "json".to_string()),
        ];

        if let Some(lang) = &options.lang {
            params.push(("language", lang.clone()));
        }

        // SearXNG-specific config (would come from env)
        // For now, use defaults
        // params.push(("engines", "google,duckduckgo".to_string()));

        let response = client
            .get(&search_url)
            .query(&params)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| format!("SearXNG request failed: {}", e))?;

        let searxng_resp: SearxngResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse SearXNG response: {}", e))?;

        if searxng_resp.results.is_empty() {
            break;
        }

        for result in searxng_resp.results {
            all_results.push(WebSearchResult {
                url: result.url,
                title: result.title,
                description: result.content,
            });

            if all_results.len() >= num_results {
                break;
            }
        }

        if all_results.len() >= num_results {
            break;
        }
    }

    if all_results.is_empty() {
        return Ok(SearchResponse { web: None });
    }

    Ok(SearchResponse {
        web: Some(all_results.into_iter().take(num_results).collect()),
    })
}

// ---------------------------------------------------------------------------
// Combined search with fallback
// ---------------------------------------------------------------------------

pub async fn search_with_fallback(
    query: &str,
    searxng_endpoint: Option<&str>,
    options: &SearchOptions,
) -> Result<SearchResponse, String> {
    // Try SearXNG first if configured
    if let Some(endpoint) = searxng_endpoint {
        match searxng_search(query, endpoint, options).await {
            Ok(resp) if resp.web.as_ref().map(|w| !w.is_empty()).unwrap_or(false) => {
                return Ok(resp);
            }
            Ok(_) => {
                eprintln!("SearXNG returned no results, falling back to DuckDuckGo");
            }
            Err(e) => {
                eprintln!("SearXNG search failed: {}, falling back to DuckDuckGo", e);
            }
        }
    }

    // Fallback to DuckDuckGo
    ddg_search(query, options.num_results, options).await
}
