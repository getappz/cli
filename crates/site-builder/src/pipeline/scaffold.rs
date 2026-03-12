//! Phase 3: Scaffold the Astro project with theme and components.

use sandbox::SandboxProvider;

use crate::config::SiteBuilderConfig;
use crate::error::SiteBuilderResult;
use crate::firecrawl::types::CrawlData;
use crate::pipeline::analyze::AnalysisResult;
use crate::templates;
use crate::themes;
use crate::themes::css::generate_global_css;
use crate::themes::tailwind::generate_tailwind_config;

/// Run the scaffold phase.
pub async fn run(
    config: &SiteBuilderConfig,
    crawl_data: Option<&CrawlData>,
    analysis: &AnalysisResult,
    sandbox: &dyn SandboxProvider,
) -> SiteBuilderResult<()> {
    let _ = ui::status::info("Phase 3: Scaffolding Astro project...");
    let fs = sandbox.fs();

    // Create directory structure
    create_project_dirs(fs)?;

    // Resolve theme
    let branding = crawl_data.and_then(|c| c.branding.as_ref());
    let theme = themes::resolve_theme(
        config.theme.as_deref(),
        branding,
        Some(&analysis.classification.suggested_theme),
    );

    let _ = ui::status::info(&format!("  Using theme: {}", theme.name));

    // Write core config files
    write_file(fs, "astro.config.mjs", templates::ASTRO_CONFIG)?;
    write_file(fs, "src/content/config.ts", templates::CONTENT_CONFIG)?;
    write_file(
        fs,
        "tailwind.config.mjs",
        &generate_tailwind_config(&theme),
    )?;
    write_file(fs, "src/styles/global.css", &generate_global_css(&theme))?;
    write_package_json(fs)?;
    write_tsconfig(fs)?;

    // Write base layout
    write_file(fs, "src/layouts/BaseLayout.astro", templates::BASE_LAYOUT)?;

    // Write components
    write_file(fs, "src/components/Hero.astro", templates::components::HERO)?;
    write_file(fs, "src/components/Section.astro", templates::components::SECTION)?;
    write_file(fs, "src/components/CardGrid.astro", templates::components::CARD_GRID)?;
    write_file(fs, "src/components/CTA.astro", templates::components::CTA)?;
    write_file(fs, "src/components/Navbar.astro", templates::components::NAVBAR)?;
    write_file(fs, "src/components/Footer.astro", templates::components::FOOTER)?;
    write_file(fs, "src/components/Stats.astro", templates::components::STATS)?;
    write_file(
        fs,
        "src/components/Testimonial.astro",
        templates::components::TESTIMONIAL,
    )?;

    // Write site data JSON (consumed by BaseLayout for nav/footer).
    write_site_data(fs, crawl_data, analysis)?;

    // Download brand assets if available
    if let Some(branding) = branding {
        download_brand_assets(fs, branding).await?;
    }

    let _ = ui::status::success("Scaffold complete.");
    Ok(())
}

/// Write `src/data/site.json` so the layout can import nav/footer/site info.
fn write_site_data(
    fs: &sandbox::ScopedFs,
    crawl_data: Option<&CrawlData>,
    analysis: &AnalysisResult,
) -> SiteBuilderResult<()> {
    let site_name = &analysis.classification.primary_category;
    // Derive a display-friendly site name from the crawl data or classification
    let display_name = crawl_data
        .and_then(|c| c.pages.first())
        .and_then(|p| p.title.clone())
        .map(|t| {
            // Take the site name portion before " - " or " | " separators
            t.split(&['-', '|'][..])
                .next_back()
                .unwrap_or(&t)
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| site_name.clone());

    let has_logo = crawl_data
        .and_then(|c| c.branding.as_ref())
        .and_then(|b| b.images.logo.as_ref())
        .is_some();

    let logo_url = if has_logo {
        "/images/logo.svg"
    } else {
        ""
    };

    let nav_links: Vec<serde_json::Value> = analysis
        .ia
        .navigation
        .primary
        .iter()
        .map(|link| {
            serde_json::json!({
                "label": link.label,
                "href": link.href,
            })
        })
        .collect();

    let footer_links: Vec<serde_json::Value> = if analysis.ia.navigation.footer.is_empty() {
        // Fall back to primary nav for footer if no specific footer links
        nav_links.clone()
    } else {
        analysis
            .ia
            .navigation
            .footer
            .iter()
            .map(|link| {
                serde_json::json!({
                    "label": link.label,
                    "href": link.href,
                })
            })
            .collect()
    };

    let site_data = serde_json::json!({
        "siteName": display_name,
        "logoUrl": logo_url,
        "navLinks": nav_links,
        "footerLinks": footer_links,
        "description": analysis.classification.brand_tone,
    });

    let content = serde_json::to_string_pretty(&site_data).unwrap_or_default();
    write_file(fs, "src/data/site.json", &content)
}

fn create_project_dirs(fs: &sandbox::ScopedFs) -> SiteBuilderResult<()> {
    let dirs = [
        "src/components",
        "src/data",
        "src/layouts",
        "src/pages",
        "src/styles",
        "src/content/pages",
        "public/images",
    ];
    for dir in &dirs {
        fs.create_dir_all(dir)?;
    }
    Ok(())
}

fn write_file(
    fs: &sandbox::ScopedFs,
    relative_path: &str,
    content: &str,
) -> SiteBuilderResult<()> {
    fs.write_string(relative_path, content)?;
    Ok(())
}

fn write_package_json(fs: &sandbox::ScopedFs) -> SiteBuilderResult<()> {
    let content = r#"{
  "name": "site",
  "type": "module",
  "version": "1.0.0",
  "scripts": {
    "dev": "astro dev",
    "build": "astro build",
    "preview": "astro preview"
  },
  "dependencies": {
    "astro": "^4.0.0",
    "@astrojs/tailwind": "^5.0.0",
    "@astrojs/mdx": "^3.0.0",
    "tailwindcss": "^3.4.0",
    "@tailwindcss/typography": "^0.5.0"
  }
}
"#;
    write_file(fs, "package.json", content)
}

fn write_tsconfig(fs: &sandbox::ScopedFs) -> SiteBuilderResult<()> {
    let content = r#"{
  "extends": "astro/tsconfigs/strict"
}
"#;
    write_file(fs, "tsconfig.json", content)
}

async fn download_brand_assets(
    fs: &sandbox::ScopedFs,
    branding: &crate::firecrawl::types::BrandingData,
) -> SiteBuilderResult<()> {
    let client = reqwest::Client::new();

    // Download logo
    if let Some(ref logo_url) = branding.images.logo {
        if let Err(e) = download_asset_to_fs(&client, fs, logo_url, "public/images/logo.svg").await {
            let _ = ui::status::warning(&format!("Could not download logo: {}", e));
        }
    }

    // Download favicon
    if let Some(ref favicon_url) = branding.images.favicon {
        if let Err(e) = download_asset_to_fs(&client, fs, favicon_url, "public/favicon.ico").await {
            let _ = ui::status::warning(&format!("Could not download favicon: {}", e));
        }
    }

    // Download OG image
    if let Some(ref og_url) = branding.images.og_image {
        if let Err(e) =
            download_asset_to_fs(&client, fs, og_url, "public/og-image.png").await
        {
            let _ = ui::status::warning(&format!("Could not download OG image: {}", e));
        }
    }

    Ok(())
}

async fn download_asset_to_fs(
    client: &reqwest::Client,
    fs: &sandbox::ScopedFs,
    url: &str,
    rel_path: &str,
) -> SiteBuilderResult<()> {
    let full_url = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    let response = client
        .get(&full_url)
        .send()
        .await
        .map_err(|e| crate::error::SiteBuilderError::AssetFailed {
            reason: format!("Failed to download {}: {}", url, e),
        })?;

    if !response.status().is_success() {
        return Err(crate::error::SiteBuilderError::AssetFailed {
            reason: format!("HTTP {} for {}", response.status(), url),
        });
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| crate::error::SiteBuilderError::AssetFailed {
            reason: format!("Failed to read response from {}: {}", url, e),
        })?;

    fs.write_file(rel_path, &bytes)?;
    Ok(())
}
