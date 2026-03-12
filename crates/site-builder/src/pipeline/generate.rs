//! Phase 4: Content transformation and page assembly.

use std::time::Duration;

use ai::{call_llm, extract_json_block, ChatMessage, LlmConfig};
use serde::{Deserialize, Serialize};

use sandbox::SandboxProvider;

use crate::config::{SiteBuilderConfig, SiteMode};
use crate::error::{SiteBuilderError, SiteBuilderResult};
use crate::firecrawl::types::CrawlData;
use crate::pipeline::analyze::{AnalysisResult, RouteSpec};
use crate::pipeline::assets;
use crate::prompts;

/// A section in a LayoutSpec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionSpec {
    pub component: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub props: serde_json::Value,
}

/// A LayoutSpec for a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSpec {
    pub page_type: String,
    pub sections: Vec<SectionSpec>,
}

/// Result of a generate run, indicating what was done and what remains.
pub struct GenerateResult {
    /// Pages that were generated in this run.
    pub generated: Vec<String>,
    /// Pages that still need to be generated.
    pub remaining: Vec<String>,
}

/// Run the generate phase.
///
/// Page selection logic:
/// - `config.pages = None` → generate only the home page ("/"), then stop.
/// - `config.pages = Some(["*"])` → generate all remaining pages.
/// - `config.pages = Some(["/about", "/contact"])` → generate only those pages.
///
/// Already-generated pages (tracked in `already_generated`) are always skipped.
pub async fn run(
    config: &SiteBuilderConfig,
    crawl_data: Option<&CrawlData>,
    analysis: &AnalysisResult,
    already_generated: &[String],
    sandbox: &dyn SandboxProvider,
) -> SiteBuilderResult<GenerateResult> {
    let _ = ui::status::info("Phase 4: Generating content and pages...");

    let llm_config = LlmConfig {
        provider: config.ai.provider.clone(),
        model: config.ai.generation_model.clone(),
        api_key: config
            .ai
            .api_key
            .clone()
            .or_else(|| resolve_api_key(&config.ai.provider)),
        base_url: config.ai.base_url.clone(),
        temperature: 0.3,
    };

    let classification_json =
        serde_json::to_string_pretty(&analysis.classification).unwrap_or_default();

    // Determine which routes to process this run.
    let routes_to_generate = select_routes(&analysis.ia.routes, &config.pages, already_generated);

    // Determine inter-call pacing delay.
    let pacing_delay = llm_pacing_delay(&config.ai.provider);

    let mut generated_pages: Vec<String> = Vec::new();

    // Process selected routes
    for (idx, route) in routes_to_generate.iter().enumerate() {
        // Pace LLM calls: sleep between pages (skip before the first).
        if idx > 0 && !pacing_delay.is_zero() {
            countdown_wait(pacing_delay).await;
        }

        let _ = ui::status::info(&format!("  Generating page: {} ({})", route.path, route.page_type));

        // Find matching crawl content
        let page_content = crawl_data.and_then(|c| {
            c.pages.iter().find(|p| {
                route
                    .source_urls
                    .iter()
                    .any(|u| p.url.contains(u.as_str()) || u.contains(p.url.as_str()))
                    || p.url.ends_with(&route.path)
            })
        });

        // Extract content: use raw crawled markdown by default.
        // Only run AI content transformation if explicitly opted in.
        let markdown_content = if let Some(page) = page_content {
            if config.transform_content {
                eprintln!("    ✍ Transforming content with AI...");
                transform_content(&llm_config, page.title.as_deref().unwrap_or(""), &page.html)
                    .await?
            } else {
                page.markdown.clone()
            }
        } else {
            String::new()
        };

        // Write markdown content file
        if !markdown_content.is_empty() {
            write_content_file(sandbox, route, &markdown_content)?;
        }

        // Download images from page content
        if let Some(page) = page_content {
            let _ = assets::download_page_images(&page.html, sandbox).await;
        }

        // Generate LayoutSpec and assemble page
        let layout = generate_layout_spec(
            &llm_config,
            config,
            route,
            &markdown_content,
            &classification_json,
        )
        .await?;

        // Render Astro page file from LayoutSpec
        let astro_content = render_astro_page(route, &layout, &analysis.ia.navigation);
        let page_path = if route.path == "/" {
            "src/pages/index.astro".to_string()
        } else {
            format!(
                "src/pages{}.astro",
                route.path.trim_end_matches('/')
            )
        };
        sandbox.fs().write_string(&page_path, &astro_content)?;

        generated_pages.push(route.path.clone());
    }

    // Compute remaining pages that haven't been generated yet.
    let all_generated: Vec<String> = already_generated
        .iter()
        .chain(generated_pages.iter())
        .cloned()
        .collect();
    let remaining: Vec<String> = analysis
        .ia
        .routes
        .iter()
        .filter(|r| !all_generated.contains(&r.path))
        .map(|r| r.path.clone())
        .collect();

    let _ = ui::status::success(&format!(
        "Generated {} page(s) this run.",
        generated_pages.len()
    ));

    Ok(GenerateResult {
        generated: generated_pages,
        remaining,
    })
}

/// Select which routes to generate based on the pages filter.
fn select_routes<'a>(
    all_routes: &'a [RouteSpec],
    pages_filter: &Option<Vec<String>>,
    already_generated: &[String],
) -> Vec<&'a RouteSpec> {
    match pages_filter {
        // No filter: generate only the home page.
        None => all_routes
            .iter()
            .filter(|r| r.path == "/" && !already_generated.contains(&r.path))
            .collect(),
        // Explicit list — "*" means all remaining.
        Some(pages) if pages.len() == 1 && pages[0] == "*" => all_routes
            .iter()
            .filter(|r| !already_generated.contains(&r.path))
            .collect(),
        // Explicit list of page paths.
        Some(pages) => all_routes
            .iter()
            .filter(|r| {
                pages.contains(&r.path) && !already_generated.contains(&r.path)
            })
            .collect(),
    }
}

/// Transform HTML content to clean Markdown using AI.
async fn transform_content(
    llm_config: &LlmConfig,
    title: &str,
    html: &str,
) -> SiteBuilderResult<String> {
    // Truncate HTML if too long (keep under ~10K chars for the prompt)
    let truncated_html = if html.len() > 10_000 {
        &html[..10_000]
    } else {
        html
    };

    let messages = vec![
        ChatMessage::system(prompts::transform::SYSTEM),
        ChatMessage::user(prompts::transform::build_user_prompt(title, truncated_html)),
    ];

    let response = call_llm(llm_config, &messages).await?;
    Ok(response.content)
}

/// Generate a LayoutSpec for a page using AI.
async fn generate_layout_spec(
    llm_config: &LlmConfig,
    config: &SiteBuilderConfig,
    route: &RouteSpec,
    markdown_content: &str,
    classification_json: &str,
) -> SiteBuilderResult<LayoutSpec> {
    let user_prompt = if config.mode == SiteMode::Create {
        prompts::assemble::build_create_prompt(
            &route.path,
            &route.page_type,
            &route.title,
            config.prompt.as_deref().unwrap_or(""),
            classification_json,
        )
    } else {
        prompts::assemble::build_user_prompt(
            &route.path,
            &route.page_type,
            markdown_content,
            classification_json,
        )
    };

    let messages = vec![
        ChatMessage::system(prompts::assemble::SYSTEM),
        ChatMessage::user(user_prompt),
    ];

    let response = call_llm(llm_config, &messages).await?;

    let json_str =
        extract_json_block(&response.content).ok_or_else(|| SiteBuilderError::GenerateFailed {
            reason: "AI did not return valid JSON for page layout".to_string(),
        })?;

    let layout: LayoutSpec = serde_json::from_str(&json_str).map_err(|e| {
        SiteBuilderError::GenerateFailed {
            reason: format!("Failed to parse LayoutSpec JSON: {}", e),
        }
    })?;

    // Validate the layout spec (inspired by lovable's truncation detection)
    validate_layout_spec(&layout, route)?;

    Ok(layout)
}

/// Validate a LayoutSpec for quality issues before rendering.
///
/// Catches common AI output problems: empty sections, missing CTA,
/// placeholder content, insufficient section count.
fn validate_layout_spec(
    layout: &LayoutSpec,
    route: &RouteSpec,
) -> SiteBuilderResult<()> {
    let section_count = layout.sections.len();

    // Must have at least 2 sections
    if section_count < 2 {
        let _ = ui::status::warning(&format!(
            "  Layout for {} has only {} section(s) — may look sparse",
            route.path, section_count
        ));
    }

    // Landing pages should start with Hero
    if route.page_type == "landing" {
        if let Some(first) = layout.sections.first() {
            if first.component != "Hero" {
                let _ = ui::status::warning(&format!(
                    "  Landing page {} doesn't start with Hero — starting with {}",
                    route.path, first.component
                ));
            }
        }
    }

    // Every page should end with CTA
    if let Some(last) = layout.sections.last() {
        if last.component != "CTA" {
            let _ = ui::status::warning(&format!(
                "  Page {} doesn't end with CTA — ends with {}",
                route.path, last.component
            ));
        }
    }

    // Check for empty Hero headlines
    for section in &layout.sections {
        if section.component == "Hero" {
            let headline = section
                .props
                .get("headline")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if headline.is_empty() || headline.len() < 5 {
                let _ = ui::status::warning("  Hero section has empty/short headline");
            }
        }

        // Check for empty CardGrid cards
        if section.component == "CardGrid" {
            let cards = section
                .props
                .get("cards")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if cards < 2 {
                let _ = ui::status::warning(&format!(
                    "  CardGrid has only {} card(s) — should have 3+",
                    cards
                ));
            }
        }
    }

    Ok(())
}

/// Write a content markdown file with frontmatter.
fn write_content_file(
    sandbox: &dyn SandboxProvider,
    route: &RouteSpec,
    markdown: &str,
) -> SiteBuilderResult<()> {
    let filename = if route.path == "/" {
        "index".to_string()
    } else {
        route
            .path
            .trim_start_matches('/')
            .trim_end_matches('/')
            .replace('/', "-")
    };

    let content = format!(
        "---\ntitle: \"{}\"\ndescription: \"\"\nlayout: \"default\"\n---\n\n{}",
        route.title.replace('"', "\\\""),
        markdown
    );

    let rel_path = format!("src/content/pages/{}.md", filename);
    sandbox.fs().write_string(&rel_path, &content)?;
    Ok(())
}

/// Render an Astro page file from a LayoutSpec.
fn render_astro_page(
    route: &RouteSpec,
    layout: &LayoutSpec,
    _nav: &crate::pipeline::analyze::NavigationSpec,
) -> String {
    let mut imports = vec!["import BaseLayout from '../layouts/BaseLayout.astro';".to_string()];
    let mut body_parts = Vec::new();

    // Collect needed component imports
    let mut needed_components = std::collections::HashSet::new();
    for section in &layout.sections {
        needed_components.insert(section.component.as_str());
    }

    for comp in &needed_components {
        imports.push(format!(
            "import {comp} from '../components/{comp}.astro';"
        ));
    }

    // Render each section
    for section in &layout.sections {
        let props_str = render_props(&section.props);
        if section.component == "Section" {
            // Section might have markdown content
            let markdown = section
                .props
                .get("markdown")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let section_props = render_props_excluding(&section.props, &["markdown"]);
            if markdown.is_empty() {
                body_parts.push(format!("  <{} {} />", section.component, section_props));
            } else {
                body_parts.push(format!(
                    "  <{comp} {props}>\n    <Fragment set:html={{`{md}`}} />\n  </{comp}>",
                    comp = section.component,
                    props = section_props,
                    md = markdown.replace('`', "\\`"),
                ));
            }
        } else if section.component == "CardGrid" || section.component == "Stats" || section.component == "Testimonial" {
            // These components should be wrapped in a Section for spacing/background
            let bg = if body_parts.len() % 2 == 1 { "secondary" } else { "default" };
            let wrapper_title = section.props.get("sectionTitle").and_then(|v| v.as_str()).unwrap_or("");
            let wrapper_subtitle = section.props.get("sectionSubtitle").and_then(|v| v.as_str()).unwrap_or("");
            let inner_props = render_props_excluding(&section.props, &["sectionTitle", "sectionSubtitle"]);
            if !wrapper_title.is_empty() {
                body_parts.push(format!(
                    "  <Section title=\"{title}\" subtitle=\"{subtitle}\" bgColor=\"{bg}\">\n    <{comp} {props} />\n  </Section>",
                    title = wrapper_title.replace('"', "&quot;"),
                    subtitle = wrapper_subtitle.replace('"', "&quot;"),
                    comp = section.component,
                    props = inner_props,
                ));
                needed_components.insert("Section");
            } else {
                body_parts.push(format!(
                    "  <Section bgColor=\"{bg}\">\n    <{comp} {props} />\n  </Section>",
                    comp = section.component,
                    props = inner_props,
                ));
                needed_components.insert("Section");
            }
        } else {
            body_parts.push(format!("  <{} {} />", section.component, props_str));
        }
    }

    // Re-gather imports after wrapping may have added Section
    let mut final_imports = vec!["import BaseLayout from '../layouts/BaseLayout.astro';".to_string()];
    for comp in &needed_components {
        final_imports.push(format!(
            "import {comp} from '../components/{comp}.astro';"
        ));
    }

    let imports_str = final_imports.join("\n");
    let body_str = body_parts.join("\n\n");

    format!(
        r#"---
{imports_str}
---

<BaseLayout title="{title}">
{body_str}
</BaseLayout>
"#,
        title = route.title.replace('"', "&quot;"),
    )
}

/// Render props as Astro JSX attribute string.
fn render_props(props: &serde_json::Value) -> String {
    render_props_excluding(props, &[])
}

/// Render props as Astro JSX attribute string, excluding certain keys.
fn render_props_excluding(props: &serde_json::Value, exclude: &[&str]) -> String {
    let obj = match props.as_object() {
        Some(o) => o,
        None => return String::new(),
    };

    obj.iter()
        .filter(|(k, _)| !exclude.contains(&k.as_str()))
        .map(|(k, v)| match v {
            serde_json::Value::String(s) => format!("{}=\"{}\"", k, s.replace('"', "&quot;")),
            serde_json::Value::Bool(b) => {
                if *b {
                    k.clone()
                } else {
                    format!("{}={{false}}", k)
                }
            }
            serde_json::Value::Number(n) => format!("{}={{{}}}", k, n),
            _ => format!("{}={{{}}}", k, serde_json::to_string(v).unwrap_or_default()),
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    // Clear the countdown line.
    eprint!("\r\x1b[K");
    let _ = std::io::stderr().flush();
}

/// Determine pacing delay between sequential LLM calls based on provider.
///
/// Free-tier rate limits differ significantly across providers:
/// - Groq free: 12K TPM → needs ~30s between large calls
/// - Gemini free: 15 RPM / 1M TPM → needs ~30s between large calls
/// - OpenAI / Anthropic: higher tier limits → no forced pacing
/// - Ollama: local → no pacing needed
///
/// Can be overridden via the `LLM_PACING_SECS` env var.
fn llm_pacing_delay(provider: &str) -> Duration {
    // Allow user override.
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

fn resolve_api_key(provider: &str) -> Option<String> {
    match provider {
        "openai" => std::env::var("OPENAI_API_KEY").ok(),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        "groq" => std::env::var("GROQ_API_KEY").ok(),
        "gemini" => std::env::var("GEMINI_API_KEY").ok(),
        _ => None,
    }
}
