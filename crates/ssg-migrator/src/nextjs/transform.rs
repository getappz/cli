//! Single-pass file transforms for Next.js migration.

use super::convert::NextJsTransform;
use super::regex::{RE_CSS_CHARSET, RE_CSS_IMPORT, RE_IMAGE_IMPORT, RE_REACT_ROUTER};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

/// Helper: returns true if a transform should be applied. When transforms is None, apply all.
fn should_apply(transforms: &Option<Vec<NextJsTransform>>, t: NextJsTransform) -> bool {
    match transforms {
        None => true,
        Some(list) if list.contains(&NextJsTransform::All) => true,
        Some(list) => list.contains(&t),
    }
}

pub(super) fn transform_client_files(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
    client_rel: &str,
    transforms_opt: Option<&str>,
) -> Result<()> {
    let transforms: Option<Vec<NextJsTransform>> = transforms_opt
        .map(|s| super::convert::parse_transforms(s))
        .filter(|v| !v.is_empty());
    let client_path = output_dir.join(client_rel);
    if !vfs.exists(client_path.as_str()) {
        return Ok(());
    }

    for entry in vfs.walk_dir(client_path.as_str())? {
        if !entry.is_file {
            continue;
        }

        let path = Utf8PathBuf::from(&entry.path);
        let ext = path
            .extension()
            .unwrap_or("");
        let needs_transform = matches!(ext, "css" | "tsx" | "jsx" | "ts" | "js");
        if !needs_transform {
            continue;
        }

        let content = vfs
            .read_to_string(&entry.path)
            .map_err(|e| miette!("Failed to read {}: {}", entry.path, e))?;

        let new_content = match ext {
            "css" => transform_css(&content),
            "tsx" | "jsx" => {
                let mut c = content.to_string();
                if should_apply(&transforms, NextJsTransform::Router) {
                    c = replace_react_router(&c);
                }
                if should_apply(&transforms, NextJsTransform::Client) {
                    c = add_use_client_directive(&c);
                }
                if should_apply(&transforms, NextJsTransform::Helmet) {
                    c = replace_react_helmet(&c);
                }
                if should_apply(&transforms, NextJsTransform::Context) {
                    c = ensure_context_client(&c);
                }
                if should_apply(&transforms, NextJsTransform::Image) {
                    c = fix_image_imports(&c);
                }
                Some(c)
            }
            "ts" | "js" => {
                let has_router = RE_REACT_ROUTER.is_match(&content);
                let has_images = RE_IMAGE_IMPORT.is_match(&content);
                let has_helmet =
                    content.contains("react-helmet") || content.contains("react-helmet-async");
                let has_context = content.contains("createContext")
                    || content.contains("useContext")
                    || content.contains("React.createContext");
                if has_router || has_images || has_helmet || has_context {
                    let mut c = content.clone();
                    if has_router && should_apply(&transforms, NextJsTransform::Router) {
                        c = replace_react_router(&c);
                    }
                    if has_helmet && should_apply(&transforms, NextJsTransform::Helmet) {
                        c = replace_react_helmet(&c);
                    }
                    if has_context && should_apply(&transforms, NextJsTransform::Context) {
                        c = ensure_context_client(&c);
                    }
                    if has_images && should_apply(&transforms, NextJsTransform::Image) {
                        c = fix_image_imports(&c);
                    }
                    Some(c)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(new) = new_content {
            if new != content {
                // Write back using the absolute path
                vfs.write_string(&entry.path, &new)
                    .map_err(|e| miette!("Failed to write {}: {}", entry.path, e))?;
            }
        }
    }
    Ok(())
}

pub(super) fn transform_css(content: &str) -> Option<String> {
    let charsets: Vec<String> = RE_CSS_CHARSET
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();
    let imports: Vec<String> = RE_CSS_IMPORT
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();

    if charsets.is_empty() && imports.is_empty() {
        return None;
    }

    let mut rest = content.to_string();
    for s in charsets.iter().chain(imports.iter()) {
        rest = rest.replace(s, "");
    }
    while rest.contains("\n\n\n") {
        rest = rest.replace("\n\n\n", "\n\n");
    }
    rest = rest.trim().to_string();

    let header: String = charsets
        .into_iter()
        .chain(imports)
        .map(|s| s + "\n")
        .collect();

    Some(if rest.is_empty() {
        header.trim_end().to_string() + "\n"
    } else {
        format!("{}\n\n{}\n", header.trim_end(), rest)
    })
}

pub(super) fn add_use_client_directive(content: &str) -> String {
    if content.trim_start().starts_with("\"use client\"")
        || content.trim_start().starts_with("'use client'")
    {
        content.to_string()
    } else {
        format!("\"use client\";\n{}", content)
    }
}

pub(super) fn replace_react_router(content: &str) -> String {
    RE_REACT_ROUTER.replace_all(content, "@App/useRouter").to_string()
}

pub(super) fn fix_image_imports(content: &str) -> String {
    if !RE_IMAGE_IMPORT.is_match(content) {
        return content.to_string();
    }

    RE_IMAGE_IMPORT
        .replace_all(content, |caps: &regex::Captures| {
            let full_match = caps.get(0).unwrap().as_str();
            let var_name = caps.get(1).unwrap().as_str();
            let private_name = format!("_{var_name}_img");
            let new_import = full_match.replacen(var_name, &private_name, 1);
            format!("{new_import}\nconst {var_name} = {private_name}.src;")
        })
        .to_string()
}

fn replace_react_helmet(content: &str) -> String {
    if !content.contains("react-helmet") && !content.contains("react-helmet-async") {
        return content.to_string();
    }
    let mut result = content.to_string();
    let helmet_import =
        regex::Regex::new(r#"import\s+\{[^}]*\bHelmet\b[^}]*\}\s+from\s+["']react-helmet(?:-async)?["'];\s*\n?"#)
            .unwrap();
    result = helmet_import.replace_all(&result, "").to_string();
    let helmet_default =
        regex::Regex::new(r#"import\s+Helmet\s+from\s+["']react-helmet(?:-async)?["'];\s*\n?"#).unwrap();
    result = helmet_default.replace_all(&result, "").to_string();
    let helmet_title = regex::Regex::new(r#"<Helmet>\s*<title>([^<]*)</title>\s*</Helmet>"#).unwrap();
    if let Some(cap) = helmet_title.captures(&result) {
        let title = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if !title.is_empty() {
            let meta_comment = format!(
                "/* Migrated from React Helmet: use metadata export in layout.tsx:\n   export const metadata = {{ title: '{}' }};\n*/",
                title.replace('\'', "\\'")
            );
            result = helmet_title.replace_all(&result, meta_comment).to_string();
        } else {
            result = helmet_title
                .replace_all(&result, "/* Migrated from React Helmet */")
                .to_string();
        }
    }
    let helmet_block = regex::Regex::new(r"<Helmet[^>]*>[\s\S]*?</Helmet>").unwrap();
    result = helmet_block
        .replace_all(&result, "/* React Helmet removed - add metadata in layout.tsx */")
        .to_string();
    result
}

fn ensure_context_client(content: &str) -> String {
    let has_context = content.contains("createContext")
        || content.contains("useContext")
        || content.contains("React.createContext");
    if !has_context {
        return content.to_string();
    }
    add_use_client_directive(content)
}
