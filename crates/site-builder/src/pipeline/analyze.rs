//! Phase 2: Business classification and IA rebuild using AI.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use ai::{call_llm, extract_json_block, ChatMessage, LlmConfig};

use crate::config::{SiteBuilderConfig, SiteMode};
use crate::error::{SiteBuilderError, SiteBuilderResult};
use crate::firecrawl::types::CrawlData;
use crate::prompts;

/// Combined output of the analysis phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub classification: Classification,
    pub ia: InformationArchitecture,
}

/// Business classification output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub primary_category: String,
    #[serde(default)]
    pub secondary_category: String,
    #[serde(default)]
    pub audiences: Vec<String>,
    #[serde(default)]
    pub brand_tone: String,
    #[serde(default)]
    pub content_density: String,
    #[serde(default)]
    pub suggested_theme: String,
    #[serde(default)]
    pub layout_type: String,
    #[serde(default)]
    pub trust_elements: Vec<String>,
    #[serde(default)]
    pub conversion_goals: Vec<String>,
}

/// Information Architecture output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InformationArchitecture {
    pub routes: Vec<RouteSpec>,
    pub navigation: NavigationSpec,
}

/// A single route in the rebuilt IA.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSpec {
    pub path: String,
    #[serde(rename = "type")]
    pub page_type: String,
    pub title: String,
    #[serde(default)]
    pub source_urls: Vec<String>,
}

/// Navigation structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationSpec {
    pub primary: Vec<NavLink>,
    pub footer: Vec<NavLink>,
}

/// A navigation link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavLink {
    pub label: String,
    pub href: String,
}

/// Run the analysis phase.
pub async fn run(
    config: &SiteBuilderConfig,
    crawl_data: Option<&CrawlData>,
) -> SiteBuilderResult<AnalysisResult> {
    let _ = ui::status::info("Phase 2: Analyzing...");

    let llm_config = LlmConfig {
        provider: config.ai.provider.clone(),
        model: config.ai.analysis_model.clone(),
        api_key: config.ai.api_key.clone().or_else(|| resolve_api_key(&config.ai.provider)),
        base_url: config.ai.base_url.clone(),
        temperature: 0.2,
    };

    // Step 1: Classify
    let _ = ui::status::info("  Classifying business...");
    let classification = classify(&llm_config, config, crawl_data).await?;
    let _ = ui::status::info(&format!(
        "  Category: {} | Theme: {} | Tone: {}",
        classification.primary_category, classification.suggested_theme, classification.brand_tone
    ));

    // Pace between classify and IA rebuild for rate-limited providers.
    let pacing = llm_pacing_delay(&config.ai.provider);
    if !pacing.is_zero() {
        countdown_wait(pacing).await;
    }

    // Step 2: IA rebuild
    let _ = ui::status::info("  Rebuilding information architecture...");
    let ia = rebuild_ia(&llm_config, config, crawl_data, &classification).await?;
    let _ = ui::status::info(&format!(
        "  Routes: {} | Nav links: {}",
        ia.routes.len(),
        ia.navigation.primary.len()
    ));

    let _ = ui::status::success("Analysis complete.");

    Ok(AnalysisResult {
        classification,
        ia,
    })
}

async fn classify(
    llm_config: &LlmConfig,
    config: &SiteBuilderConfig,
    crawl_data: Option<&CrawlData>,
) -> SiteBuilderResult<Classification> {
    let user_prompt = if config.mode == SiteMode::Create {
        let prompt_text = config.prompt.as_deref().unwrap_or("A professional website");
        prompts::classify::build_create_prompt(prompt_text)
    } else {
        let crawl = crawl_data.ok_or_else(|| SiteBuilderError::AnalyzeFailed {
            reason: "No crawl data available for classification".to_string(),
        })?;
        let summary = build_crawl_summary(crawl);
        let branding_json = crawl
            .branding
            .as_ref()
            .and_then(|b| serde_json::to_string_pretty(b).ok());
        prompts::classify::build_user_prompt(&summary, branding_json.as_deref())
    };

    let messages = vec![
        ChatMessage::system(prompts::classify::SYSTEM),
        ChatMessage::user(user_prompt),
    ];

    let response = call_llm(llm_config, &messages).await?;

    let json_str = extract_json_block(&response.content).ok_or_else(|| {
        SiteBuilderError::AnalyzeFailed {
            reason: "AI did not return valid JSON for classification".to_string(),
        }
    })?;

    serde_json::from_str(&json_str).map_err(|e| SiteBuilderError::AnalyzeFailed {
        reason: format!("Failed to parse classification JSON: {}", e),
    })
}

async fn rebuild_ia(
    llm_config: &LlmConfig,
    config: &SiteBuilderConfig,
    crawl_data: Option<&CrawlData>,
    classification: &Classification,
) -> SiteBuilderResult<InformationArchitecture> {
    let classification_json =
        serde_json::to_string_pretty(classification).unwrap_or_default();

    let user_prompt = if config.mode == SiteMode::Create {
        let prompt_text = config.prompt.as_deref().unwrap_or("A professional website");
        prompts::ia::build_create_prompt(prompt_text, &classification_json)
    } else if config.mode == SiteMode::Clone {
        // For clone mode, preserve original structure
        if let Some(crawl) = crawl_data {
            return Ok(build_clone_ia(crawl));
        }
        prompts::ia::build_create_prompt("", &classification_json)
    } else {
        let crawl = crawl_data.ok_or_else(|| SiteBuilderError::AnalyzeFailed {
            reason: "No crawl data available for IA rebuild".to_string(),
        })?;
        let summary = build_crawl_summary(crawl);
        prompts::ia::build_user_prompt(&summary, &classification_json)
    };

    let messages = vec![
        ChatMessage::system(prompts::ia::SYSTEM),
        ChatMessage::user(user_prompt),
    ];

    let response = call_llm(llm_config, &messages).await?;

    let json_str =
        extract_json_block(&response.content).ok_or_else(|| SiteBuilderError::AnalyzeFailed {
            reason: "AI did not return valid JSON for IA rebuild".to_string(),
        })?;

    serde_json::from_str(&json_str).map_err(|e| SiteBuilderError::AnalyzeFailed {
        reason: format!("Failed to parse IA JSON: {}", e),
    })
}

/// Build a text summary of crawl data for AI consumption.
fn build_crawl_summary(crawl: &CrawlData) -> String {
    let mut summary = format!("Site: {}\n\nPages:\n", crawl.site_url);
    for page in &crawl.pages {
        summary.push_str(&format!(
            "- URL: {}\n  Title: {}\n  Description: {}\n  Content preview: {}\n\n",
            page.url,
            page.title.as_deref().unwrap_or("(none)"),
            page.meta_description.as_deref().unwrap_or("(none)"),
            &page.markdown[..page.markdown.len().min(500)],
        ));
    }
    summary
}

/// Build an IA from crawl data for clone mode (preserves original structure).
fn build_clone_ia(crawl: &CrawlData) -> InformationArchitecture {
    let base_url = crawl.site_url.trim_end_matches('/');
    let routes: Vec<RouteSpec> = crawl
        .pages
        .iter()
        .map(|page| {
            let path = page
                .url
                .strip_prefix(base_url)
                .unwrap_or(&page.url)
                .to_string();
            let path = if path.is_empty() { "/".to_string() } else { path };
            RouteSpec {
                path: path.clone(),
                page_type: if path == "/" {
                    "landing".to_string()
                } else {
                    "standard".to_string()
                },
                title: page.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                source_urls: vec![page.url.clone()],
            }
        })
        .collect();

    let primary_nav: Vec<NavLink> = routes
        .iter()
        .take(6)
        .map(|r| NavLink {
            label: r.title.clone(),
            href: r.path.clone(),
        })
        .collect();

    InformationArchitecture {
        routes,
        navigation: NavigationSpec {
            primary: primary_nav,
            footer: vec![],
        },
    }
}

/// Display a countdown timer on a single line, overwriting each second.
async fn countdown_wait(duration: Duration) {
    use std::io::Write;
    let total_secs = duration.as_secs();
    for remaining in (1..=total_secs).rev() {
        eprint!("\r\x1b[K  ⏳ Rate-limit pacing: {}s remaining...", remaining);
        let _ = std::io::stderr().flush();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    eprint!("\r\x1b[K");
    let _ = std::io::stderr().flush();
}

/// Determine pacing delay between sequential LLM calls based on provider.
fn llm_pacing_delay(provider: &str) -> Duration {
    if let Ok(secs) = std::env::var("LLM_PACING_SECS") {
        if let Ok(s) = secs.parse::<f64>() {
            return Duration::from_secs_f64(s);
        }
    }
    match provider {
        "groq" => Duration::from_secs(30),
        "gemini" => Duration::from_secs(30),
        _ => Duration::ZERO,
    }
}

/// Resolve API key from environment variables.
fn resolve_api_key(provider: &str) -> Option<String> {
    match provider {
        "openai" => std::env::var("OPENAI_API_KEY").ok(),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        "groq" => std::env::var("GROQ_API_KEY").ok(),
        "gemini" => std::env::var("GEMINI_API_KEY").ok(),
        _ => None,
    }
}
