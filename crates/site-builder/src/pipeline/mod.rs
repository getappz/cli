//! Pipeline orchestration for the site builder.
//!
//! The pipeline runs in 5 phases:
//! 1. Crawl - Firecrawl API (map + scrape + branding)
//! 2. Analyze - AI classification + IA rebuild
//! 3. Scaffold - Astro bootstrap + theme + components
//! 4. Generate - Content transform + page assembly
//! 5. Build - astro build
//!
//! All file I/O and command execution (npm, astro build) run inside a sandbox
//! for isolation and tool management (Node via mise).

pub mod analyze;
pub mod assets;
pub mod build;
pub mod crawl;
pub mod generate;
pub mod scaffold;

use std::sync::Arc;

use sandbox::{create_sandbox, SandboxConfig, SandboxProvider, SandboxSettings};

use crate::cache::ArtifactCache;
use crate::config::SiteBuilderConfig;
use crate::config::SiteMode;
use crate::error::{SiteBuilderError, SiteBuilderResult};

/// Run the full site builder pipeline.
pub async fn run(config: &SiteBuilderConfig) -> SiteBuilderResult<()> {
    let session_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let cache = ArtifactCache::new(&config.output_dir, &session_id)?;
    let mut state = cache.load_state()?;

    let _ = ui::status::info(&format!(
        "Site builder: {:?} mode, output: {}",
        config.mode,
        config.output_dir.display()
    ));

    // Phase 1: Crawl (skip for create mode)
    let crawl_data = if config.mode != SiteMode::Create {
        if state.crawl_done && config.resume {
            let _ = ui::status::info("Resuming: crawl phase already complete.");
            cache.read_artifact("crawl.json")?
        } else {
            let url = config.url.as_deref().ok_or_else(|| SiteBuilderError::ConfigError {
                reason: "URL is required for redesign/clone mode".to_string(),
            })?;
            let data = crawl::run(url, config).await?;
            cache.write_artifact("crawl.json", &data)?;
            state.crawl_done = true;
            cache.save_state(&state)?;
            Some(data)
        }
    } else {
        None
    };

    // Phase 2: Analyze
    let analysis = if state.analyze_done && config.resume {
        let _ = ui::status::info("Resuming: analyze phase already complete.");
        cache
            .read_artifact::<analyze::AnalysisResult>("analysis.json")?
            .ok_or_else(|| SiteBuilderError::CacheError {
                reason: "analysis.json missing from cache".to_string(),
            })?
    } else {
        let result = analyze::run(config, crawl_data.as_ref()).await?;
        cache.write_artifact("analysis.json", &result)?;
        state.analyze_done = true;
        cache.save_state(&state)?;
        result
    };

    // Create sandbox for scaffold / generate / build (file I/O and npm).
    // Skip for dry_run.
    let sandbox: Option<Arc<dyn SandboxProvider>> = if config.dry_run {
        None
    } else {
        let sb_config = SandboxConfig::new(&config.output_dir)
            .with_settings(SandboxSettings::default().with_tool("node", Some("22")));
        let sb = create_sandbox(sb_config)
            .await
            .map_err(|e| SiteBuilderError::ScaffoldFailed {
                reason: format!("Sandbox setup failed: {}", e),
            })?;
        Some(Arc::from(sb))
    };

    // Phase 3: Scaffold
    if !(state.scaffold_done && config.resume) {
        let sandbox_ref = sandbox
            .as_ref()
            .ok_or_else(|| SiteBuilderError::ConfigError {
                reason: "Sandbox required for scaffold (dry_run?)".to_string(),
            })?;
        scaffold::run(config, crawl_data.as_ref(), &analysis, sandbox_ref.as_ref()).await?;
        state.scaffold_done = true;
        cache.save_state(&state)?;
    } else {
        let _ = ui::status::info("Resuming: scaffold phase already complete.");
    }

    // Phase 4: Generate (incremental, page by page)
    if !(state.generate_done && config.resume) {
        let sandbox_ref = sandbox
            .as_ref()
            .ok_or_else(|| SiteBuilderError::ConfigError {
                reason: "Sandbox required for generate (dry_run?)".to_string(),
            })?;
        let result = generate::run(
            config,
            crawl_data.as_ref(),
            &analysis,
            &state.generated_pages,
            sandbox_ref.as_ref(),
        )
        .await?;

        // Record newly generated pages.
        for page in &result.generated {
            if !state.generated_pages.contains(page) {
                state.generated_pages.push(page.clone());
            }
        }

        if result.remaining.is_empty() {
            state.generate_done = true;
        }
        cache.save_state(&state)?;

        // If there are remaining pages, stop and tell the user what to do next.
        if !result.remaining.is_empty() {
            print_next_steps(config, &result.remaining);
            return Ok(());
        }
    } else {
        let _ = ui::status::info("Resuming: generate phase already complete.");
    }

    // Phase 5: Build
    if config.build && !config.dry_run {
        if !(state.build_done && config.resume) {
            let sandbox_ref = sandbox
                .as_ref()
                .ok_or_else(|| SiteBuilderError::ConfigError {
                    reason: "Sandbox required for build (dry_run?)".to_string(),
                })?;
            build::run(config, sandbox_ref.as_ref()).await?;
            state.build_done = true;
            cache.save_state(&state)?;
        } else {
            let _ = ui::status::info("Resuming: build phase already complete.");
        }
    }

    let _ = ui::status::success(&format!(
        "Site builder complete! Project at: {}",
        config.output_dir.display()
    ));

    Ok(())
}

/// Print user-friendly next-step commands after home page generation.
fn print_next_steps(config: &SiteBuilderConfig, remaining: &[String]) {
    let output_flag = format!(" -o {}", config.output_dir.display());

    eprintln!();
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  ✅ Home page generated! Review it in: {}", config.output_dir.display());
    eprintln!();
    eprintln!("  Remaining pages to generate:");
    for (i, page) in remaining.iter().enumerate() {
        eprintln!("    {}. {}", i + 1, page);
    }
    eprintln!();
    eprintln!("  Next steps:");
    eprintln!();

    // Build the base command depending on mode
    let base_cmd = match config.mode {
        SiteMode::Redesign => {
            let url = config.url.as_deref().unwrap_or("<url>");
            format!("appz site generate-page --url {}{}", url, output_flag)
        }
        SiteMode::Create => {
            format!("appz site generate-page --create{}", output_flag)
        }
        SiteMode::Clone => {
            let url = config.url.as_deref().unwrap_or("<url>");
            format!("appz site generate-page --url {}{}", url, output_flag)
        }
    };

    eprintln!("  # Generate a specific page:");
    if let Some(first) = remaining.first() {
        eprintln!("  {} --page {}", base_cmd, first);
    }
    eprintln!();
    eprintln!("  # Generate multiple pages:");
    eprintln!("  {} --page /about --page /contact", base_cmd);
    eprintln!();
    eprintln!("  # Generate all remaining pages:");
    eprintln!("  {} --all", base_cmd);
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();
}
