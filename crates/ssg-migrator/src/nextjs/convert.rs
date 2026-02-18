//! Content-only Next.js conversion for single-file convert command.

use super::transform::{add_use_client_directive, fix_image_imports};
use miette::Result;
use regex::Regex;

/// Transform options for Next.js conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextJsTransform {
    Router,
    Client,
    Helmet,
    Context,
    Image,
    Env,
    All,
}

/// Parse comma-separated transform names into a vec. "all" returns empty (apply-all).
pub fn parse_transforms(s: &str) -> Vec<NextJsTransform> {
    let mut out = Vec::new();
    for t in s.split(',').map(str::trim) {
        match t.to_lowercase().as_str() {
            "router" => out.push(NextJsTransform::Router),
            "client" => out.push(NextJsTransform::Client),
            "helmet" => out.push(NextJsTransform::Helmet),
            "context" => out.push(NextJsTransform::Context),
            "image" => out.push(NextJsTransform::Image),
            "env" => out.push(NextJsTransform::Env),
            "all" => return vec![NextJsTransform::All],
            _ => {}
        }
    }
    out
}

/// Convert React content to Next.js format (content-only, no project context).
pub fn convert_to_nextjs(content: &str, transforms: &[NextJsTransform]) -> Result<String> {
    let apply_all = transforms.is_empty() || transforms.contains(&NextJsTransform::All);

    let mut result = content.to_string();

    if apply_all || transforms.contains(&NextJsTransform::Router) {
        result = replace_react_router_for_nextjs(&result);
    }
    if apply_all || transforms.contains(&NextJsTransform::Client) {
        result = add_use_client_directive(&result);
    }
    if apply_all || transforms.contains(&NextJsTransform::Helmet) {
        result = replace_react_helmet(&result);
    }
    if apply_all || transforms.contains(&NextJsTransform::Context) {
        result = ensure_context_client(&result);
    }
    if apply_all || transforms.contains(&NextJsTransform::Image) {
        result = fix_image_imports(&result);
    }
    if apply_all || transforms.contains(&NextJsTransform::Env) {
        result = replace_react_app_env(&result);
    }

    Ok(result)
}

/// Replace REACT_APP_ with NEXT_PUBLIC_ for Next.js env var prefix.
fn replace_react_app_env(content: &str) -> String {
    content.replace("REACT_APP_", "NEXT_PUBLIC_")
}

/// Replace React Helmet with Next.js metadata pattern.
/// For App Router: converts static <Helmet><title>X</title></Helmet> to metadata export.
fn replace_react_helmet(content: &str) -> String {
    if !content.contains("react-helmet") && !content.contains("react-helmet-async") {
        return content.to_string();
    }

    let mut result = content.to_string();

    // Remove Helmet imports
    let helmet_import =
        Regex::new(r#"import\s+\{[^}]*\bHelmet\b[^}]*\}\s+from\s+["']react-helmet(?:-async)?["'];\s*\n?"#)
            .unwrap();
    result = helmet_import.replace_all(&result, "").to_string();

    // Remove default Helmet import
    let helmet_default =
        Regex::new(r#"import\s+Helmet\s+from\s+["']react-helmet(?:-async)?["'];\s*\n?"#).unwrap();
    result = helmet_default.replace_all(&result, "").to_string();

    // Replace <Helmet><title>Static Title</title></Helmet> with metadata export
    // For simplicity: remove Helmet usage and add a comment; full conversion would need AST
    let helmet_title = Regex::new(r#"<Helmet>\s*<title>([^<]*)</title>\s*</Helmet>"#).unwrap();
    if helmet_title.is_match(&result) {
        // Extract first static title for metadata placeholder
        if let Some(cap) = helmet_title.captures(&result) {
            let title = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if !title.is_empty() {
                let meta_comment = format!(
                    "/* Migrated from React Helmet: use metadata export in layout.tsx:\n   export const metadata = {{ title: '{}' }};\n*/",
                    title.replace('\'', "\\'")
                );
                result = helmet_title.replace_all(&result, meta_comment).to_string();
            } else {
                result = helmet_title.replace_all(&result, "/* Migrated from React Helmet */").to_string();
            }
        }
    }

    // Remove any remaining <Helmet>...</Helmet> blocks (generic fallback)
    let helmet_block = Regex::new(r"<Helmet[^>]*>[\s\S]*?</Helmet>").unwrap();
    result = helmet_block
        .replace_all(&result, "/* React Helmet removed - add metadata in layout.tsx */")
        .to_string();

    result
}

/// Ensure files using React Context have "use client" (required in Next.js App Router).
fn ensure_context_client(content: &str) -> String {
    let has_context = content.contains("createContext")
        || content.contains("useContext")
        || content.contains("React.createContext");
    if !has_context {
        return content.to_string();
    }
    add_use_client_directive(content)
}

/// Replace React Router with standard Next.js navigation.
fn replace_react_router_for_nextjs(content: &str) -> String {
    let mut result = content.to_string();

    let has_router = result.contains("react-router-dom");
    let has_navigate = Regex::new(r"\buseNavigate\s*\(").unwrap().is_match(&result);
    let has_location = Regex::new(r"\buseLocation\s*\(").unwrap().is_match(&result);

    if !has_router && !has_navigate && !has_location {
        return result;
    }

    // Remove react-router-dom import
    let router_import =
        Regex::new(r#"import\s+\{[^}]*\}\s+from\s+["']react-router-dom["'];\s*\n?"#).unwrap();
    result = router_import.replace_all(&result, "").to_string();

    // Build imports to add
    let mut imports = Vec::new();
    if has_router && !result.contains("next/link") {
        imports.push("import Link from 'next/link';");
    }
    if (has_navigate || has_location) && !result.contains("next/navigation") {
        imports.push("import { useRouter, usePathname } from 'next/navigation';");
    }
    if !imports.is_empty() {
        let insert = imports.join("\n") + "\n";
        let trimmed = result.trim_start();
        if trimmed.starts_with("'use client'") || trimmed.starts_with("\"use client\"") {
            let end = result.find('\n').map_or(result.len(), |i| i + 1);
            result = format!("{}{}\n{}", &result[..end], insert.trim_end(), &result[end..]);
        } else {
            result = format!("{}\n{}", insert.trim_end(), result);
        }
    }

    // useNavigate() -> useRouter(); replace navigate(path) with navigate.push(path)
    let use_navigate = Regex::new(r"const\s+(\w+)\s*=\s*useNavigate\(\)\s*;?").unwrap();
    let nav_vars: Vec<String> = use_navigate
        .captures_iter(&result)
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .collect();
    result = use_navigate
        .replace_all(&result, "const $1 = useRouter();")
        .to_string();
    for var in nav_vars {
        let pattern = format!(r"\b{}(\s*\()", regex::escape(&var));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, format!("{}.push(", var))
                .to_string();
        }
    }

    // useLocation() -> usePathname()
    let use_location = Regex::new(r"const\s+(\w+)\s*=\s*useLocation\(\)\s*;?").unwrap();
    result = use_location
        .replace_all(&result, "const $1 = usePathname();")
        .to_string();

    // location.pathname -> pathname (usePathname returns the path string directly)
    result = result.replace("location.pathname", "pathname");

    // Link to= -> href=
    result = result.replace(" to=", " href=");

    result
}
