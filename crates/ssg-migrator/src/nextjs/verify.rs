//! SSG compatibility verification for static export.

use super::regex::{
    RE_IMAGE_IMPORT, RE_NEXT_CACHE_IMPORT, RE_NEXT_HEADERS_IMPORT, RE_REDIRECT_IMPORT,
    RE_USE_SEARCH_PARAMS, RE_USE_SERVER,
};
use crate::types::{SsgSeverity, SsgWarning};
use miette::{miette, Result};
use sandbox::ScopedFs;
use walkdir::WalkDir;

pub(super) fn verify_static_export(fs: &ScopedFs) -> Result<Vec<SsgWarning>> {
    let mut warnings: Vec<SsgWarning> = Vec::new();
    let root = fs.root();

    let config_path = root.join("next.config.ts");
    if config_path.exists() {
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| miette!("Failed to read next.config.ts: {}", e))?;
        if !config_content.contains("output:") || !config_content.contains("export") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `output: \"export\"` — static build will produce SSR output instead of static HTML".to_string(),
                file: Some("next.config.ts".to_string()),
            });
        }
        if !config_content.contains("unoptimized") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `images: { unoptimized: true }` — image optimization requires a Node server".to_string(),
                file: Some("next.config.ts".to_string()),
            });
        }
    } else {
        warnings.push(SsgWarning {
            severity: SsgSeverity::Error,
            message: "next.config.ts not found".to_string(),
            file: None,
        });
    }

    let app_dir = root.join("src/app");
    if app_dir.exists() {
        for entry in WalkDir::new(&app_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() || path.file_name().and_then(|n| n.to_str()) != Some("page.tsx") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if !rel.contains('[') {
                continue;
            }
            let content = std::fs::read_to_string(path)
                .map_err(|e| miette!("Failed to read {}: {}", rel, e))?;
            if !content.contains("generateStaticParams") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Error,
                    message: "Dynamic route page is missing `generateStaticParams()`".to_string(),
                    file: Some(rel),
                });
            }
        }
    }

    let src_dir = root.join("src");
    if src_dir.exists() {
        for entry in WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "ts" | "tsx" | "jsx" | "js") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if RE_NEXT_HEADERS_IMPORT.is_match(&content) {
                let cap = RE_NEXT_HEADERS_IMPORT.captures(&content).unwrap();
                let api = cap.get(1).map(|m| m.as_str()).unwrap_or("cookies/headers");
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: format!("Imports `{api}` from `next/headers` — not available in static export"),
                    file: Some(rel.clone()),
                });
            }
            if RE_NEXT_CACHE_IMPORT.is_match(&content) {
                let cap = RE_NEXT_CACHE_IMPORT.captures(&content).unwrap();
                let api = cap.get(1).map(|m| m.as_str()).unwrap_or("revalidatePath/revalidateTag");
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: format!("Imports `{api}` from `next/cache` — not available in static export"),
                    file: Some(rel.clone()),
                });
            }
            if RE_REDIRECT_IMPORT.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Imports `redirect` — use client-side useRouter().push instead".to_string(),
                    file: Some(rel.clone()),
                });
            }
            if RE_USE_SERVER.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Contains `\"use server\"` — server actions not available in static export".to_string(),
                    file: Some(rel.clone()),
                });
            }

            if RE_IMAGE_IMPORT.is_match(&content) {
                let raw_vars: Vec<String> = RE_IMAGE_IMPORT
                    .captures_iter(&content)
                    .filter_map(|cap| {
                        let var = cap.get(1)?.as_str();
                        if var.starts_with('_') && var.ends_with("_img") {
                            None
                        } else {
                            Some(var.to_string())
                        }
                    })
                    .collect();
                for var in &raw_vars {
                    warnings.push(SsgWarning {
                        severity: SsgSeverity::Warning,
                        message: format!("Image import `{var}` was not rewritten"),
                        file: Some(rel.clone()),
                    });
                }
            }

            if RE_USE_SEARCH_PARAMS.is_match(&content) && !content.contains("Suspense") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Uses `useSearchParams` without `<Suspense>` boundary".to_string(),
                    file: Some(rel.clone()),
                });
            }
        }
    }

    Ok(warnings)
}
