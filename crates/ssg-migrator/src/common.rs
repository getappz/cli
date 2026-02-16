//! Shared migration helpers used by both Astro and Next.js generators.

use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::collections::HashMap;

/// Copy `public/` from source directory into output at `{dst_rel}`.
pub fn copy_public_assets(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    dst_dir: &str,
) -> Result<()> {
    let source_public = source_dir.join("public");
    if vfs.exists(source_public.as_str()) {
        vfs.copy_dir(source_public.as_str(), dst_dir)
            .map_err(|e| miette!("Failed to copy public assets: {}", e))?;
    }
    Ok(())
}

/// Copy tailwind.config.ts or tailwind.config.js from source to output root.
pub fn copy_tailwind_config(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    output_dir: &str,
) -> Result<()> {
    for ext in ["ts", "js"] {
        let src = source_dir.join(format!("tailwind.config.{ext}"));
        if vfs.exists(src.as_str()) {
            let dst = format!("{}/tailwind.config.{ext}", output_dir);
            vfs.copy_file(src.as_str(), &dst)
                .map_err(|e| miette!("Failed to copy tailwind.config.{}: {}", ext, e))?;
        }
    }
    Ok(())
}

/// Dependencies to **exclude** when migrating (build tools, bundlers, routers, dev-only).
/// Entries are prefix-matched: `"vite"` blocks `vite`, `vite-plugin-foo`, etc.
const DEP_DENYLIST: &[&str] = &[
    "vite",
    "@vitejs/",
    "react-router",
    "react-scripts",
    "@types/",
    "typescript",
    "eslint",
    "@eslint/",
    "eslint-",
    "globals",
    "lovable-tagger",
    "@tailwindcss/",
];

/// Exact dependency names to exclude (react, react-dom are pinned by the generator).
const DEP_DENYLIST_EXACT: &[&str] = &["react", "react-dom"];

/// Filter dependencies for migration using a denylist approach:
/// keep everything the user had, except build tools, bundlers, and router packages.
pub fn filter_deps(deps: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    'outer: for (dep, version) in deps {
        if DEP_DENYLIST_EXACT.contains(&dep.as_str()) {
            continue;
        }
        for prefix in DEP_DENYLIST {
            if dep.starts_with(prefix) {
                continue 'outer;
            }
        }
        out.insert(dep.clone(), version.clone());
    }
    out
}

pub use filter_deps as filter_lovable_deps;
