//! SSG compatibility verification for static export.

use super::regex::{
    RE_IMAGE_IMPORT, RE_NEXT_CACHE_IMPORT, RE_NEXT_HEADERS_IMPORT, RE_REDIRECT_IMPORT,
    RE_USE_SEARCH_PARAMS, RE_USE_SERVER,
};
use crate::types::{SsgSeverity, SsgWarning};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;
use std::sync::LazyLock;

static RE_WINDOW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bwindow\b").unwrap());
static RE_DOCUMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bdocument\b").unwrap());
static RE_LOCALSTORAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\blocalStorage\b").unwrap());
static RE_REACT_APP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"REACT_APP_").unwrap());

pub(super) fn verify_static_export(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
) -> Result<Vec<SsgWarning>> {
    let mut warnings: Vec<SsgWarning> = Vec::new();

    let config_path = output_dir.join("next.config.ts");
    if vfs.exists(config_path.as_str()) {
        let config_content = vfs
            .read_to_string(config_path.as_str())
            .map_err(|e| miette!("Failed to read next.config.ts: {}", e))?;
        if !config_content.contains("output:") || !config_content.contains("export") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `output: \"export\"` — static build will produce SSR output instead of static HTML".to_string(),
                file: Some("next.config.ts".to_string()),
                rule_id: Some("build-static-export".to_string()),
            });
        }
        if !config_content.contains("unoptimized") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `images: { unoptimized: true }` — image optimization requires a Node server".to_string(),
                file: Some("next.config.ts".to_string()),
                rule_id: Some("build-static-export".to_string()),
            });
        }
    } else {
        warnings.push(SsgWarning {
            severity: SsgSeverity::Error,
            message: "next.config.ts not found".to_string(),
            file: None,
            rule_id: Some("setup-next-config".to_string()),
        });
    }

    let app_dir = output_dir.join("src/app");
    if vfs.exists(app_dir.as_str()) {
        for entry in vfs.walk_dir(app_dir.as_str())? {
            if !entry.is_file {
                continue;
            }
            let path = Utf8PathBuf::from(&entry.path);
            if path.file_name() != Some("page.tsx") {
                continue;
            }
            let rel = entry
                .path
                .strip_prefix(output_dir.as_str())
                .unwrap_or(&entry.path)
                .trim_start_matches('/')
                .replace('\\', "/");
            if !rel.contains('[') {
                continue;
            }
            let content = vfs
                .read_to_string(&entry.path)
                .map_err(|e| miette!("Failed to read {}: {}", rel, e))?;
            if !content.contains("generateStaticParams") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Error,
                    message: "Dynamic route page is missing `generateStaticParams()`".to_string(),
                    file: Some(rel),
                    rule_id: Some("routing-dynamic-routes".to_string()),
                });
            }
        }
    }

    let src_dir = output_dir.join("src");
    if vfs.exists(src_dir.as_str()) {
        for entry in vfs.walk_dir(src_dir.as_str())? {
            if !entry.is_file {
                continue;
            }
            let path = Utf8PathBuf::from(&entry.path);
            let ext = path.extension().unwrap_or("");
            if !matches!(ext, "ts" | "tsx" | "jsx" | "js") {
                continue;
            }
            let rel = entry
                .path
                .strip_prefix(output_dir.as_str())
                .unwrap_or(&entry.path)
                .trim_start_matches('/')
                .replace('\\', "/");
            let content = match vfs.read_to_string(&entry.path) {
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
                    rule_id: Some("gotchas-middleware".to_string()),
                });
            }
            if RE_NEXT_CACHE_IMPORT.is_match(&content) {
                let cap = RE_NEXT_CACHE_IMPORT.captures(&content).unwrap();
                let api = cap.get(1).map(|m| m.as_str()).unwrap_or("revalidatePath/revalidateTag");
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: format!("Imports `{api}` from `next/cache` — not available in static export"),
                    file: Some(rel.clone()),
                    rule_id: Some("data-revalidation".to_string()),
                });
            }
            if RE_REDIRECT_IMPORT.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Imports `redirect` — use client-side useRouter().push instead".to_string(),
                    file: Some(rel.clone()),
                    rule_id: Some("gotchas-redirect".to_string()),
                });
            }
            if RE_USE_SERVER.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Contains `\"use server\"` — server actions not available in static export".to_string(),
                    file: Some(rel.clone()),
                    rule_id: Some("data-server-actions".to_string()),
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
                        rule_id: Some("images-next-image".to_string()),
                    });
                }
            }

            if RE_USE_SEARCH_PARAMS.is_match(&content) && !content.contains("Suspense") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Uses `useSearchParams` without `<Suspense>` boundary".to_string(),
                    file: Some(rel.clone()),
                    rule_id: Some("routing-use-search-params".to_string()),
                });
            }

            verify_gotchas_file(&content, &rel, &mut warnings);
        }
    }

    Ok(warnings)
}

fn verify_gotchas_file(content: &str, rel: &str, warnings: &mut Vec<SsgWarning>) {
    let has_use_client = content.trim_start().starts_with("\"use client\"")
        || content.trim_start().starts_with("'use client'");

    if !has_use_client {
        if RE_WINDOW.is_match(content) || RE_DOCUMENT.is_match(content) {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Warning,
                message: "Uses `window` or `document` without `\"use client\"` — will fail during SSR".to_string(),
                file: Some(rel.to_string()),
                rule_id: Some("gotchas-window-undefined".to_string()),
            });
        }
        if RE_LOCALSTORAGE.is_match(content) {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Warning,
                message: "Uses `localStorage` without `\"use client\"` — will fail during SSR".to_string(),
                file: Some(rel.to_string()),
                rule_id: Some("gotchas-window-undefined".to_string()),
            });
        }
    }

    if RE_REACT_APP.is_match(content) {
        warnings.push(SsgWarning {
            severity: SsgSeverity::Error,
            message: "Contains `REACT_APP_` — run with env transform or replace with `NEXT_PUBLIC_`".to_string(),
            file: Some(rel.to_string()),
            rule_id: Some("env-prefix-change".to_string()),
        });
    }
}
