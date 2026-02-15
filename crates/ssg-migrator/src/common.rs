//! Shared migration helpers used by both Astro and Next.js generators.
//!
//! All file I/O uses `ScopedFs` for path safety. Source reads use absolute paths
//! (outside sandbox); writes go to fs-relative paths.

use camino::Utf8PathBuf;
use miette::{miette, Result};
use sandbox::ScopedFs;
use std::collections::HashMap;
use std::path::Path;

/// Copy `public/` from source directory into sandbox at `public`.
pub fn copy_public_assets(
    source_dir: &Utf8PathBuf,
    fs: &ScopedFs,
    dst_rel: &str,
) -> Result<()> {
    let source_public = source_dir.join("public");
    if source_public.exists() {
        fs.copy_from_external(source_public.as_path(), dst_rel)
            .map_err(|e| miette!("Failed to copy public assets: {}", e))?;
    }
    Ok(())
}

/// Copy tailwind.config.ts or tailwind.config.js from source to sandbox root.
pub fn copy_tailwind_config(source_dir: &Utf8PathBuf, fs: &ScopedFs) -> Result<()> {
    let source_ts = source_dir.join("tailwind.config.ts");
    if source_ts.exists() {
        fs.copy_from_external(source_ts.as_path(), "tailwind.config.ts")
            .map_err(|e| miette!("Failed to copy tailwind.config.ts: {}", e))?;
    }
    let source_js = source_dir.join("tailwind.config.js");
    if source_js.exists() {
        fs.copy_from_external(source_js.as_path(), "tailwind.config.js")
            .map_err(|e| miette!("Failed to copy tailwind.config.js: {}", e))?;
    }
    Ok(())
}

/// Copy a directory tree from an external absolute path into the sandbox.
pub fn copy_from_external(
    fs: &ScopedFs,
    src_abs: impl AsRef<Path>,
    dst_rel: impl AsRef<Path>,
) -> Result<()> {
    fs.copy_from_external(src_abs, dst_rel)
        .map_err(|e| miette!("Failed to copy from external: {}", e))?;
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

/// Filter dependencies for Next.js migration using a denylist approach:
/// keep everything the user had, except build tools, bundlers, and router packages.
pub fn filter_deps(deps: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    'outer: for (dep, version) in deps {
        // Exact matches
        if DEP_DENYLIST_EXACT.contains(&dep.as_str()) {
            continue;
        }
        // Prefix matches
        for prefix in DEP_DENYLIST {
            if dep.starts_with(prefix) {
                continue 'outer;
            }
        }
        out.insert(dep.clone(), version.clone());
    }
    out
}

// Keep the old name as an alias so the Astro generator (which still uses it) compiles.
pub use filter_deps as filter_lovable_deps;
