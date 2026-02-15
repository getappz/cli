//! Page generation for Astro projects.

use super::templates::generate_client_only_page;
use super::transform::transform_page_to_astro;
use crate::types::ProjectAnalysis;
use biome_fs::BiomePath;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::fs;
use std::io::Write;

pub(super) fn generate_pages(pages_dir: &Utf8PathBuf, analysis: &ProjectAnalysis) -> Result<()> {
    for route in &analysis.routes {
        let page_name = if route.path == "/" {
            "index".to_string()
        } else if route.is_catch_all {
            "404".to_string()
        } else {
            route.path.trim_start_matches('/').replace('/', "-")
        };

        let source_tsx = analysis.source_dir.join("src/pages").join(format!("{}.tsx", route.component));
        let source_ts = analysis.source_dir.join("src/pages").join(format!("{}.ts", route.component));
        let page_path = pages_dir.join(format!("{}.astro", page_name));

        if source_tsx.exists() || source_ts.exists() {
            let actual = if source_tsx.exists() { &source_tsx } else { &source_ts };
            let is_client = analysis.components.iter().any(|c| c.file_path == *actual && c.is_client_side);

            let page_content = if is_client {
                generate_client_only_page(&route.component)
            } else {
                let content = BiomePath::new(actual.clone()).read_to_string()
                    .map_err(|e| miette!("Failed to read page: {}", e))?;
                transform_page_to_astro(&content, &route.component, analysis)
            };

            let mut file = fs::File::create(&page_path).map_err(|e| miette!("Failed to create page: {}", e))?;
            file.write_all(page_content.as_bytes()).map_err(|e| miette!("Failed to write page: {}", e))?;
        } else {
            let fallback = format!(
                "---\nimport Layout from '../layouts/Layout.astro';\n---\n<Layout>\n  <h1>{}</h1>\n  <p>Page content migrated</p>\n</Layout>\n",
                route.component
            );
            let mut file = fs::File::create(&page_path).map_err(|e| miette!("Failed to create page: {}", e))?;
            file.write_all(fallback.as_bytes()).map_err(|e| miette!("Failed to write page: {}", e))?;
        }
    }
    Ok(())
}
