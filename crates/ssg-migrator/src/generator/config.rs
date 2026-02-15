//! Astro config, package.json, and tsconfig generation.

use crate::common::filter_deps;
use crate::types::ProjectAnalysis;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::fs;
use std::io::Write;

pub(super) fn generate_astro_config(
    output_dir: &Utf8PathBuf,
    analysis: &ProjectAnalysis,
) -> Result<()> {
    let config_path = output_dir.join("astro.config.mjs");
    let mut file = fs::File::create(&config_path)
        .map_err(|e| miette!("Failed to create astro.config.mjs: {}", e))?;

    let mut config = String::from(
        "import { defineConfig } from 'astro/config';\nimport react from '@astrojs/react';\n",
    );

    if analysis.has_tailwind {
        config.push_str("import tailwind from '@astrojs/tailwind';\n");
    }

    config.push_str("\nexport default defineConfig({\n");
    config.push_str("  integrations: [\n");
    config.push_str("    react(),\n");
    if analysis.has_tailwind {
        config.push_str("    tailwind(),\n");
    }
    config.push_str("  ],\n");
    config.push_str("  output: 'static',\n");
    config.push_str("  vite: {\n");
    config.push_str("    resolve: {\n");
    config.push_str("      alias: {\n");
    config.push_str("        '@': new URL('./src', import.meta.url).pathname,\n");
    config.push_str("      },\n");
    config.push_str("    },\n");
    config.push_str("  },\n");
    config.push_str("});\n");

    file.write_all(config.as_bytes())
        .map_err(|e| miette!("Failed to write astro.config.mjs: {}", e))?;
    Ok(())
}

pub(super) fn generate_package_json(
    output_dir: &Utf8PathBuf,
    project_name: &str,
    analysis: &ProjectAnalysis,
) -> Result<()> {
    use serde_json::{json, Map};

    let mut dependencies = Map::new();

    dependencies.insert("astro".to_string(), json!("^4.0.0"));
    dependencies.insert("@astrojs/react".to_string(), json!("^3.0.0"));
    dependencies.insert("react".to_string(), json!("^18.3.0"));
    dependencies.insert("react-dom".to_string(), json!("^18.3.0"));

    if analysis.has_tailwind {
        dependencies.insert("@astrojs/tailwind".to_string(), json!("^5.0.0"));
        dependencies.insert("tailwindcss".to_string(), json!("^3.4.0"));
    }

    let filtered = filter_deps(&analysis.dependencies);
    for (dep, version) in &filtered {
        if !dependencies.contains_key(dep) {
            dependencies.insert(dep.clone(), json!(version));
        }
    }

    let mut dev_dependencies = Map::new();
    dev_dependencies.insert("@types/react".to_string(), json!("^18.3.0"));
    dev_dependencies.insert("@types/react-dom".to_string(), json!("^18.3.0"));
    dev_dependencies.insert("typescript".to_string(), json!("^5"));

    for key in ["tailwindcss", "postcss", "autoprefixer"] {
        if let Some(v) = dependencies.remove(key) {
            dev_dependencies.insert(key.to_string(), v);
        }
    }

    let package_json = json!({
        "name": project_name,
        "type": "module",
        "version": "0.0.1",
        "scripts": {
            "dev": "astro dev",
            "start": "astro dev",
            "build": "astro build",
            "preview": "astro preview"
        },
        "dependencies": dependencies,
        "devDependencies": dev_dependencies
    });

    let package_path = output_dir.join("package.json");
    let mut file = fs::File::create(&package_path)
        .map_err(|e| miette!("Failed to create package.json: {}", e))?;

    let formatted = serde_json::to_string_pretty(&package_json)
        .map_err(|e| miette!("Failed to serialize package.json: {}", e))?;

    file.write_all(formatted.as_bytes())
        .map_err(|e| miette!("Failed to write package.json: {}", e))?;
    Ok(())
}

pub(super) fn generate_tsconfig(output_dir: &Utf8PathBuf) -> Result<()> {
    let tsconfig_path = output_dir.join("tsconfig.json");
    let mut file = fs::File::create(&tsconfig_path)
        .map_err(|e| miette!("Failed to create tsconfig.json: {}", e))?;

    let tsconfig = r#"{
  "extends": "astro/tsconfigs/strict",
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
"#;
    file.write_all(tsconfig.as_bytes())
        .map_err(|e| miette!("Failed to write tsconfig.json: {}", e))?;
    Ok(())
}
