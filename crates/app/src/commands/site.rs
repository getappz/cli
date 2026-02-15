//! AI-powered website creation, redesign, and cloning.

use std::path::PathBuf;

use clap::Subcommand;
use miette::miette;
use starbase::AppResult;
use tracing::instrument;

use crate::session::AppzSession;
use crate::wasm::types::PluginSiteRunInput;
use site_builder::config::{AiConfig, SiteBuilderConfig};
use site_builder::pipeline;

/// Subcommands for the `appz site` command group.
#[derive(Subcommand, Debug, Clone)]
pub enum SiteCommands {
    /// Redesign an existing website with a modern, professional look
    Redesign {
        /// URL of the website to redesign
        url: String,
        /// Output directory for the generated project
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Override theme selection (nonprofit, corporate, startup, minimal)
        #[arg(long)]
        theme: Option<String>,
        /// AI provider (openai, anthropic, ollama)
        #[arg(long)]
        provider: Option<String>,
        /// AI model for generation
        #[arg(long)]
        model: Option<String>,
        /// Use AI to rewrite and improve the site content (default: use original content as-is)
        #[arg(long)]
        transform_content: bool,
        /// Skip the build step after generation
        #[arg(long)]
        no_build: bool,
        /// Resume from last checkpoint
        #[arg(long)]
        resume: bool,
        /// Show plan without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a new website from a natural-language description
    Create {
        /// Description of the website to create
        #[arg(required = true, trailing_var_arg = true)]
        prompt: Vec<String>,
        /// Output directory for the generated project
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Override theme selection (nonprofit, corporate, startup, minimal)
        #[arg(long)]
        theme: Option<String>,
        /// AI provider (openai, anthropic, ollama)
        #[arg(long)]
        provider: Option<String>,
        /// AI model for generation
        #[arg(long)]
        model: Option<String>,
        /// Skip the build step after generation
        #[arg(long)]
        no_build: bool,
    },
    /// Clone an existing website as faithfully as possible
    Clone {
        /// URL of the website to clone
        url: String,
        /// Output directory for the generated project
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// AI provider (openai, anthropic, ollama)
        #[arg(long)]
        provider: Option<String>,
        /// AI model for generation
        #[arg(long)]
        model: Option<String>,
        /// Skip the build step after generation
        #[arg(long)]
        no_build: bool,
    },
    /// Generate specific page(s) for an existing project (after initial redesign/create)
    GeneratePage {
        /// Source URL (required for redesign/clone projects)
        #[arg(long)]
        url: Option<String>,
        /// Output directory of the existing project
        #[arg(short, long, required = true)]
        output: PathBuf,
        /// Page path(s) to generate (e.g. --page /about --page /contact)
        #[arg(long = "page")]
        pages: Vec<String>,
        /// Generate all remaining pages
        #[arg(long)]
        all: bool,
        /// This is a create-mode project (no source URL)
        #[arg(long)]
        create: bool,
        /// AI provider (openai, anthropic, ollama, groq, gemini)
        #[arg(long)]
        provider: Option<String>,
        /// AI model for generation
        #[arg(long)]
        model: Option<String>,
        /// Use AI to rewrite and improve the site content
        #[arg(long)]
        transform_content: bool,
        /// Skip the build step after generation
        #[arg(long)]
        no_build: bool,
    },
}

/// Run the site subcommand.
#[instrument(skip_all)]
pub async fn run(session: AppzSession, command: SiteCommands) -> AppResult {
    match command {
        SiteCommands::Redesign {
            url,
            output,
            theme,
            provider,
            model,
            transform_content,
            no_build,
            resume,
            dry_run,
        } => {
            let output_dir = resolve_output_dir(&session, output, &url)?;
            let mut config = SiteBuilderConfig::redesign(url, output_dir);
            config.theme = theme;
            config.build = !no_build;
            config.resume = resume;
            config.dry_run = dry_run;
            config.transform_content = transform_content;
            config.ai = resolve_ai_config(provider, model);
            config.firecrawl_api_key = std::env::var("FIRECRAWL_API_KEY").ok();

            pipeline::run(&config).await.map_err(|e| miette!("{}", e))?;
        }
        SiteCommands::Create {
            prompt,
            output,
            theme,
            provider,
            model,
            no_build,
        } => {
            let prompt_str = prompt.join(" ").trim().to_string();
            if prompt_str.is_empty() {
                return Err(miette!("Prompt cannot be empty"));
            }

            let output_dir = output.unwrap_or_else(|| session.working_dir.join("site-output"));
            let mut config = SiteBuilderConfig::create(prompt_str, output_dir);
            config.theme = theme;
            config.build = !no_build;
            config.ai = resolve_ai_config(provider, model);

            pipeline::run(&config).await.map_err(|e| miette!("{}", e))?;
        }
        SiteCommands::Clone {
            url,
            output,
            provider,
            model,
            no_build,
        } => {
            let output_dir = resolve_output_dir(&session, output, &url)?;
            let mut config = SiteBuilderConfig::clone_site(url, output_dir);
            config.build = !no_build;
            config.ai = resolve_ai_config(provider, model);
            config.firecrawl_api_key = std::env::var("FIRECRAWL_API_KEY").ok();

            pipeline::run(&config).await.map_err(|e| miette!("{}", e))?;
        }
        SiteCommands::GeneratePage {
            url,
            output,
            pages,
            all,
            create,
            provider,
            model,
            transform_content,
            no_build,
        } => {
            // Determine which pages to generate.
            let page_filter = if all {
                Some(vec!["*".to_string()])
            } else if pages.is_empty() {
                return Err(miette!(
                    "Specify at least one --page <path> or use --all to generate all remaining pages"
                ));
            } else {
                Some(pages)
            };

            // Build the config — resume from the existing project.
            let mut config = if create {
                // create-mode project: no URL needed.
                let mut c = SiteBuilderConfig::create(String::new(), output);
                c.prompt = None; // Not needed for incremental generation.
                c
            } else {
                let url = url.ok_or_else(|| {
                    miette!("--url is required for redesign/clone projects")
                })?;
                SiteBuilderConfig::redesign(url, output)
            };

            config.pages = page_filter;
            config.resume = true;
            config.build = !no_build;
            config.transform_content = transform_content;
            config.ai = resolve_ai_config(provider, model);
            config.firecrawl_api_key = std::env::var("FIRECRAWL_API_KEY").ok();

            pipeline::run(&config).await.map_err(|e| miette!("{}", e))?;
        }
    }

    Ok(None)
}

/// Run the site builder from plugin input. Used by the site plugin host function.
pub async fn run_site_with_config(
    input: &PluginSiteRunInput,
) -> Result<(), miette::Report> {
    let working_dir = PathBuf::from(&input.working_dir);
    let output_dir = resolve_output_from_input(input)?;

    let mut config = match input.subcommand.as_str() {
        "redesign" => {
            let url = input.url.as_ref().ok_or_else(|| {
                miette!("URL is required for redesign. Use --url <url> or pass URL as argument.")
            })?;
            SiteBuilderConfig::redesign(url.clone(), output_dir)
        }
        "create" => {
            let prompt = input
                .prompt
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| miette!("Prompt is required for create. Describe the website you want."))?;
            SiteBuilderConfig::create(prompt, output_dir)
        }
        "clone" => {
            let url = input.url.as_ref().ok_or_else(|| {
                miette!("URL is required for clone. Use --url <url> or pass URL as argument.")
            })?;
            SiteBuilderConfig::clone_site(url.clone(), output_dir)
        }
        "generate-page" => {
            let page_filter = if input.all {
                vec!["*".to_string()]
            } else if let Some(ref p) = input.pages {
                if p.is_empty() {
                    return Err(miette!(
                        "Specify at least one --page <path> or use --all to generate all remaining pages"
                    ));
                }
                p.clone()
            } else {
                return Err(miette!(
                    "Specify at least one --page <path> or use --all to generate all remaining pages"
                ));
            };

            let mut c = if input.create {
                let mut cfg = SiteBuilderConfig::create(String::new(), output_dir);
                cfg.prompt = None;
                cfg
            } else {
                let url = input.url.as_ref().ok_or_else(|| {
                    miette!("--url is required for redesign/clone projects")
                })?;
                SiteBuilderConfig::redesign(url.clone(), output_dir)
            };
            c.pages = Some(page_filter);
            c.resume = true;
            c
        }
        other => return Err(miette!("Unknown site subcommand: {}", other)),
    };

    config.theme = input.theme.clone();
    config.build = !input.no_build;
    config.resume = config.resume || input.resume;
    config.dry_run = input.dry_run;
    config.transform_content = input.transform_content;
    config.ai = resolve_ai_config(input.provider.clone(), input.model.clone());
    config.firecrawl_api_key = std::env::var("FIRECRAWL_API_KEY").ok();

    pipeline::run(&config).await.map_err(|e| miette!("{}", e))
}

fn resolve_output_from_input(input: &PluginSiteRunInput) -> miette::Result<PathBuf> {
    if let Some(ref o) = input.output {
        return Ok(PathBuf::from(o));
    }
    match input.subcommand.as_str() {
        "redesign" | "clone" => {
            let url = input
                .url
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("site-output");
            let domain = url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .unwrap_or_else(|| "site-output".to_string());
            let dir_name = domain.replace("www.", "").replace('.', "-");
            Ok(PathBuf::from(&input.working_dir).join(dir_name))
        }
        "create" => Ok(PathBuf::from(&input.working_dir).join("site-output")),
        "generate-page" => Err(miette!(
            "--output is required for generate-page. Specify the existing project directory."
        )),
        _ => Ok(PathBuf::from(&input.working_dir).join("site-output")),
    }
}

/// Resolve the output directory from CLI args or derive from the URL.
fn resolve_output_dir(
    session: &AppzSession,
    output: Option<PathBuf>,
    url: &str,
) -> miette::Result<PathBuf> {
    if let Some(o) = output {
        return Ok(o);
    }
    // Derive directory name from URL domain
    let domain = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "site-output".to_string());
    let dir_name = domain
        .replace("www.", "")
        .replace('.', "-");
    Ok(session.working_dir.join(dir_name))
}

/// Resolve AI config from CLI args and environment.
fn resolve_ai_config(provider: Option<String>, model: Option<String>) -> AiConfig {
    let provider = provider.unwrap_or_else(|| {
        // Auto-detect provider from available API keys.
        // Priority: Gemini → Groq → OpenAI → Anthropic.
        // Each is only selected when its API key is configured.
        if std::env::var("GEMINI_API_KEY").is_ok() {
            "gemini".to_string()
        } else if std::env::var("GROQ_API_KEY").is_ok() {
            "groq".to_string()
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            "openai".to_string()
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            "anthropic".to_string()
        } else {
            eprintln!("⚠ No AI API key found. Set one of: GEMINI_API_KEY, GROQ_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY");
            // Default to gemini; will fail at call time with a clear error.
            "gemini".to_string()
        }
    });

    let api_key = match provider.as_str() {
        "openai" => std::env::var("OPENAI_API_KEY").ok(),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        "groq" => std::env::var("GROQ_API_KEY").ok(),
        "gemini" => std::env::var("GEMINI_API_KEY").ok(),
        "ollama" => None,
        _ => std::env::var("AI_API_KEY").ok(),
    };

    let base_url = match provider.as_str() {
        "ollama" => Some(
            std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        ),
        "groq" => Some(
            std::env::var("GROQ_BASE_URL")
                .unwrap_or_else(|_| "https://api.groq.com/openai".to_string()),
        ),
        "gemini" => Some(
            std::env::var("GEMINI_BASE_URL")
                .unwrap_or_else(|_| {
                    "https://generativelanguage.googleapis.com/v1beta/openai".to_string()
                }),
        ),
        _ => None,
    };

    let (analysis_model, generation_model) = if let Some(m) = model {
        (m.clone(), m)
    } else {
        match provider.as_str() {
            "openai" => ("gpt-4o-mini".to_string(), "gpt-4o".to_string()),
            "anthropic" => (
                "claude-sonnet-4-20250514".to_string(),
                "claude-sonnet-4-20250514".to_string(),
            ),
            "groq" => (
                "llama-3.3-70b-versatile".to_string(),
                "llama-3.3-70b-versatile".to_string(),
            ),
            "gemini" => (
                "gemini-2.0-flash".to_string(),
                "gemini-2.0-flash".to_string(),
            ),
            "ollama" => ("llama3".to_string(), "llama3".to_string()),
            _ => ("gpt-4o-mini".to_string(), "gpt-4o".to_string()),
        }
    };

    AiConfig {
        provider,
        analysis_model,
        generation_model,
        api_key,
        base_url,
    }
}
