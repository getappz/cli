//! Configuration types for the site builder pipeline.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The mode of operation for the site builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SiteMode {
    /// Redesign an existing website with a fresh, modern look.
    Redesign,
    /// Create a new website from a text prompt.
    Create,
    /// Clone an existing website as faithfully as possible.
    Clone,
}

/// Top-level configuration for a site builder run.
#[derive(Debug, Clone)]
pub struct SiteBuilderConfig {
    /// Mode of operation.
    pub mode: SiteMode,
    /// Source URL (for redesign/clone modes).
    pub url: Option<String>,
    /// Text prompt (for create mode).
    pub prompt: Option<String>,
    /// Output directory for the generated project.
    pub output_dir: PathBuf,
    /// Override theme selection.
    pub theme: Option<String>,
    /// Target framework (default: "astro").
    pub framework: String,
    /// Whether to run the build step after generation.
    pub build: bool,
    /// Whether to resume from a previous checkpoint.
    pub resume: bool,
    /// Dry-run: show plan without executing.
    pub dry_run: bool,
    /// Whether to use AI to rewrite/improve content.
    /// When false (default), the crawled content is used exactly as-is.
    /// When true, AI transforms and improves the copy.
    pub transform_content: bool,
    /// Which pages to generate.
    /// - `None` → generate only the home page, then stop and prompt the user.
    /// - `Some(vec)` → generate only the listed page paths (e.g. `["/about", "/contact"]`).
    /// The special sentinel `["*"]` means "generate all remaining pages".
    pub pages: Option<Vec<String>>,
    /// AI provider configuration.
    pub ai: AiConfig,
    /// Firecrawl API key.
    pub firecrawl_api_key: Option<String>,
}

/// AI provider configuration for the site builder.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// Provider name ("openai", "anthropic", "ollama").
    pub provider: String,
    /// Model name for classification/analysis (cheaper model).
    pub analysis_model: String,
    /// Model name for content generation (higher quality model).
    pub generation_model: String,
    /// API key.
    pub api_key: Option<String>,
    /// Base URL (for Ollama or custom endpoints).
    pub base_url: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            analysis_model: "gpt-4o-mini".to_string(),
            generation_model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
        }
    }
}

impl SiteBuilderConfig {
    /// Create a config for redesign mode.
    pub fn redesign(url: String, output_dir: PathBuf) -> Self {
        Self {
            mode: SiteMode::Redesign,
            url: Some(url),
            prompt: None,
            output_dir,
            theme: None,
            framework: "astro".to_string(),
            build: true,
            resume: false,
            dry_run: false,
            transform_content: false,
            pages: None,
            ai: AiConfig::default(),
            firecrawl_api_key: None,
        }
    }

    /// Create a config for create mode.
    pub fn create(prompt: String, output_dir: PathBuf) -> Self {
        Self {
            mode: SiteMode::Create,
            url: None,
            prompt: Some(prompt),
            output_dir,
            theme: None,
            framework: "astro".to_string(),
            build: true,
            resume: false,
            dry_run: false,
            transform_content: false,
            pages: None,
            ai: AiConfig::default(),
            firecrawl_api_key: None,
        }
    }

    /// Create a config for clone mode.
    pub fn clone_site(url: String, output_dir: PathBuf) -> Self {
        Self {
            mode: SiteMode::Clone,
            url: Some(url),
            prompt: None,
            output_dir,
            theme: None,
            framework: "astro".to_string(),
            build: true,
            resume: false,
            dry_run: false,
            transform_content: false,
            pages: None,
            ai: AiConfig::default(),
            firecrawl_api_key: None,
        }
    }
}
