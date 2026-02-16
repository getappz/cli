//! package.json and config file generation/copying.

use crate::common::{copy_tailwind_config, filter_deps};
use crate::types::ProjectAnalysis;
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use serde_json::{json, Map, Value};

pub(super) fn write_package_json(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
    analysis: &ProjectAnalysis,
    project_name: &str,
    static_export: bool,
) -> Result<()> {
    let mut dependencies = Map::new();
    dependencies.insert("next".to_string(), json!("15.5.6"));
    dependencies.insert("react".to_string(), json!("19.1.0"));
    dependencies.insert("react-dom".to_string(), json!("19.1.0"));

    let filtered = filter_deps(&analysis.dependencies);
    for (dep, version) in &filtered {
        dependencies.insert(dep.clone(), json!(version));
    }

    let mut dev_dependencies = Map::new();
    dev_dependencies.insert("@types/node".to_string(), json!("^20"));
    dev_dependencies.insert("@types/react".to_string(), json!("^19"));
    dev_dependencies.insert("@types/react-dom".to_string(), json!("^19"));
    dev_dependencies.insert("typescript".to_string(), json!("^5"));
    if analysis.has_tailwind {
        let tw_ver = dependencies.remove("tailwindcss").unwrap_or_else(|| json!("^3.4.0"));
        let pc_ver = dependencies.remove("postcss").unwrap_or_else(|| json!("^8"));
        let ap_ver = dependencies.remove("autoprefixer").unwrap_or_else(|| json!("^10"));
        dev_dependencies.insert("tailwindcss".to_string(), tw_ver);
        dev_dependencies.insert("postcss".to_string(), pc_ver);
        dev_dependencies.insert("autoprefixer".to_string(), ap_ver);
        if !dependencies.contains_key("tailwind-merge") {
            dependencies.insert("tailwind-merge".to_string(), json!("^2.6.0"));
        }
        if !dependencies.contains_key("tailwindcss-animate") {
            dependencies.insert("tailwindcss-animate".to_string(), json!("^1.0.7"));
        }
    }

    let scripts = if static_export {
        json!({
            "dev": "next dev --turbopack",
            "build": "next build",
            "start": "npx serve out"
        })
    } else {
        json!({
            "dev": "next dev --turbopack",
            "build": "next build --turbopack",
            "start": "next start"
        })
    };

    let package: Value = json!({
        "name": project_name,
        "version": "0.1.0",
        "private": true,
        "type": "module",
        "scripts": scripts,
        "dependencies": dependencies,
        "devDependencies": dev_dependencies
    });

    let formatted = serde_json::to_string_pretty(&package)
        .map_err(|e| miette!("Failed to serialize package.json: {}", e))?;
    vfs.write_string(output_dir.join("package.json").as_str(), &formatted)
        .map_err(|e| miette!("Failed to write package.json: {}", e))?;
    Ok(())
}

pub(super) fn copy_config_files(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
    source_dir: &Utf8PathBuf,
    static_export: bool,
) -> Result<()> {
    let static_lines = if static_export {
        r#"
  output: "export",
  images: { unoptimized: true },"#
    } else {
        ""
    };

    let next_config = format!(
        r#"import type {{ NextConfig }} from "next";
import path from "path";

const nextConfig: NextConfig = {{{static_lines}
  turbopack: {{
    root: path.join(__dirname, "./"),
  }},
}};

export default nextConfig;
"#
    );
    vfs.write_string(output_dir.join("next.config.ts").as_str(), &next_config)
        .map_err(|e| miette!("Failed to write next.config.ts: {}", e))?;

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2017",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": true,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "preserve",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": {
      "@/*": ["./src/client/*"],
      "@App/*": ["./src/app/*"]
    }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
"#;
    vfs.write_string(output_dir.join("tsconfig.json").as_str(), tsconfig)
        .map_err(|e| miette!("Failed to write tsconfig.json: {}", e))?;

    copy_tailwind_config(vfs, source_dir, output_dir.as_str())?;

    let postcss_ts = source_dir.join("postcss.config.ts");
    if vfs.exists(postcss_ts.as_str()) {
        vfs.copy_file(
            postcss_ts.as_str(),
            output_dir.join("postcss.config.ts").as_str(),
        )
        .map_err(|e| miette!("Failed to copy postcss.config.ts: {}", e))?;
    }
    let postcss_js = source_dir.join("postcss.config.js");
    if vfs.exists(postcss_js.as_str()) {
        vfs.copy_file(
            postcss_js.as_str(),
            output_dir.join("postcss.config.js").as_str(),
        )
        .map_err(|e| miette!("Failed to copy postcss.config.js: {}", e))?;
    }

    let components_json = source_dir.join("components.json");
    if vfs.exists(components_json.as_str()) {
        let content = vfs
            .read_to_string(components_json.as_str())
            .map_err(|e| miette!("Failed to read components.json: {}", e))?;
        let rewritten = content.replace("src/", "src/client/");
        vfs.write_string(
            output_dir.join("components.json").as_str(),
            &rewritten,
        )
        .map_err(|e| miette!("Failed to write components.json: {}", e))?;
    }
    Ok(())
}
