//! Site builder error types with rich diagnostic messages.

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for site builder operations.
pub type SiteBuilderResult<T> = Result<T, SiteBuilderError>;

/// Errors that can occur during site building operations.
#[derive(Error, Debug, Diagnostic)]
pub enum SiteBuilderError {
    #[diagnostic(
        code(site_builder::crawl_failed),
        help("Check your FIRECRAWL_API_KEY and network connection.\nVerify the target URL is accessible.")
    )]
    #[error("Crawl failed: {reason}")]
    CrawlFailed { reason: String },

    #[diagnostic(
        code(site_builder::analyze_failed),
        help("The AI analysis step failed. Check your AI provider configuration.")
    )]
    #[error("Analysis failed: {reason}")]
    AnalyzeFailed { reason: String },

    #[diagnostic(
        code(site_builder::scaffold_failed),
        help("Failed to bootstrap the Astro project. Ensure Node.js is installed.")
    )]
    #[error("Scaffold failed: {reason}")]
    ScaffoldFailed { reason: String },

    #[diagnostic(
        code(site_builder::generate_failed),
        help("Content generation failed. Check your AI provider configuration.")
    )]
    #[error("Generation failed: {reason}")]
    GenerateFailed { reason: String },

    #[diagnostic(
        code(site_builder::build_failed),
        help("The Astro build failed. Check the generated project for errors.")
    )]
    #[error("Build failed: {reason}")]
    BuildFailed { reason: String },

    #[diagnostic(
        code(site_builder::asset_failed),
        help("Failed to download an asset. The URL may be inaccessible.")
    )]
    #[error("Asset download failed: {reason}")]
    AssetFailed { reason: String },

    #[diagnostic(
        code(site_builder::cache_error),
        help("The pipeline cache may be corrupted. Delete .appz/cache/site-builder/ and retry.")
    )]
    #[error("Cache error: {reason}")]
    CacheError { reason: String },

    #[diagnostic(
        code(site_builder::config_error),
        help("Check your site builder configuration in appz.json.")
    )]
    #[error("Configuration error: {reason}")]
    ConfigError { reason: String },

    #[diagnostic(code(site_builder::io_error))]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[diagnostic(code(site_builder::ai_error))]
    #[error(transparent)]
    Ai(#[from] ai::AiError),

    #[diagnostic(code(site_builder::sandbox_error))]
    #[error(transparent)]
    Sandbox(#[from] sandbox::SandboxError),
}
