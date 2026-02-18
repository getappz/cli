//! DuckDuckGo web search using Workers fetch API.
//!
//! Port of `crates/search/src/lib.rs` — replaces `reqwest` with Workers
//! global `fetch()` so it runs inside the Worker WASM sandbox.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use worker::wasm_bindgen::JsValue;
use worker::{Fetch, Headers, Method, Request, RequestInit};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchOptions {
    pub tbs: Option<String>,
    pub filter: Option<String>,
    pub lang: Option<String>,
    pub country: Option<String>,
    pub location: Option<String>,
    pub num_results: usize,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
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
// User agents
// ---------------------------------------------------------------------------

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
];

fn random_user_agent() -> &'static str {
    let mut buf = [0u8; 1];
    let _ = getrandom::fill(&mut buf);
    USER_AGENTS[buf[0] as usize % USER_AGENTS.len()]
}

// ---------------------------------------------------------------------------
// URL cleaning
// ---------------------------------------------------------------------------

fn clean_ddg_url(href: &str) -> String {
    if href.contains("uddg=") {
        if let Ok(url) = url::Url::parse(href) {
            if let Some(uddg) = url.query_pairs().find(|(k, _)| k == "uddg") {
                return urlencoding::decode(&uddg.1)
                    .unwrap_or(std::borrow::Cow::Borrowed(href))
                    .into_owned();
            }
        }
    }
    href.to_string()
}

// ---------------------------------------------------------------------------
// HTML parsing
// ---------------------------------------------------------------------------

fn extract_ddg_results(
    html: &str,
    seen_urls: &mut HashSet<String>,
) -> Result<Vec<WebSearchResult>, String> {
    let document = Html::parse_document(html);

    let anomaly_selector =
        Selector::parse(".anomaly-modal__modal").map_err(|e| format!("selector: {e:?}"))?;
    if document.select(&anomaly_selector).next().is_some() {
        return Err("DDG_ANTIBOT".to_string());
    }

    let result_selector =
        Selector::parse(".result.web-result").map_err(|e| format!("selector: {e:?}"))?;
    let title_selector =
        Selector::parse(".result__a").map_err(|e| format!("selector: {e:?}"))?;
    let snippet_selector =
        Selector::parse(".result__snippet").map_err(|e| format!("selector: {e:?}"))?;

    let mut results = Vec::new();

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

    let form_selector = Selector::parse("form").ok()?;
    let next_selector = Selector::parse(r#"input[type="submit"][value="Next"]"#).ok()?;
    let input_selector = Selector::parse("input").ok()?;

    for form in document.select(&form_selector) {
        if form.select(&next_selector).next().is_some() {
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

// ---------------------------------------------------------------------------
// Workers fetch helpers
// ---------------------------------------------------------------------------

fn common_headers(user_agent: &str) -> Result<Headers, String> {
    let mut headers = Headers::new();
    headers.set("User-Agent", user_agent).map_err(|e| e.to_string())?;
    headers
        .set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .map_err(|e| e.to_string())?;
    headers
        .set("Accept-Language", "en-US,en;q=0.5")
        .map_err(|e| e.to_string())?;
    Ok(headers)
}

async fn workers_get(url_str: &str, user_agent: &str) -> Result<String, String> {
    let headers = common_headers(user_agent)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let req = Request::new_with_init(url_str, &init).map_err(|e| e.to_string())?;
    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

async fn workers_post_form(
    url_str: &str,
    user_agent: &str,
    params: &[(String, String)],
) -> Result<String, String> {
    let mut headers = common_headers(user_agent)?;
    headers
        .set("Content-Type", "application/x-www-form-urlencoded")
        .map_err(|e| e.to_string())?;

    let body: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let req = Request::new_with_init(url_str, &init).map_err(|e| e.to_string())?;
    let mut resp = Fetch::Request(req).send().await.map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Public search API
// ---------------------------------------------------------------------------

pub async fn ddg_search(query: &str, options: &SearchOptions) -> Result<SearchResponse, String> {
    let lang = options.lang.as_deref().unwrap_or("en");
    let country = options.country.as_deref().unwrap_or("us");
    let num_results = options.num_results;
    let user_agent = random_user_agent();

    let mut params: Vec<(String, String)> = vec![
        ("q".to_string(), query.to_string()),
        ("kp".to_string(), "1".to_string()),
    ];

    if let Some(location) = &options.location {
        params.push(("kl".to_string(), location.clone()));
    } else {
        params.push((
            "kl".to_string(),
            format!("{}-{}", country.to_lowercase(), lang.to_lowercase()),
        ));
    }

    if let Some(tbs) = &options.tbs {
        if tbs == "d" || tbs == "w" || tbs == "m" || tbs == "y" || tbs.contains("..") {
            params.push(("df".to_string(), tbs.clone()));
        }
    }

    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let mut is_first_page = true;
    let mut antibot_retries = 0u32;
    let max_antibot_retries = 3u32;

    while results.len() < num_results && antibot_retries <= max_antibot_retries {
        let html = if is_first_page {
            let base = url::Url::parse_with_params("https://html.duckduckgo.com/html", &params)
                .map_err(|e| format!("url parse: {e}"))?;
            workers_get(base.as_str(), user_agent).await?
        } else {
            workers_post_form("https://html.duckduckgo.com/html", user_agent, &params).await?
        };

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

                if let Some(next_params) = get_next_page_params(&html) {
                    params = next_params;
                } else {
                    break;
                }
            }
            Err(e) if e == "DDG_ANTIBOT" => {
                antibot_retries += 1;
                if antibot_retries > max_antibot_retries {
                    return Err("DuckDuckGo: Blocked by anti-bot measures".to_string());
                }
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
