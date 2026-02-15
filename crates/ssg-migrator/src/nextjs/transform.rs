//! Single-pass file transforms for Next.js migration.

use super::regex::{RE_CSS_CHARSET, RE_CSS_IMPORT, RE_IMAGE_IMPORT, RE_REACT_ROUTER};
use miette::{miette, Result};
use sandbox::ScopedFs;
use walkdir::WalkDir;

pub(super) fn transform_client_files(fs: &ScopedFs, client_rel: &str) -> Result<()> {
    let root = fs.root();
    let client_path = root.join(client_rel);
    if !client_path.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&client_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let needs_transform = matches!(ext, "css" | "tsx" | "jsx" | "ts" | "js");
        if !needs_transform {
            continue;
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| miette!("Failed to read {}: {}", path.display(), e))?;

        let new_content = match ext {
            "css" => transform_css(&content),
            "tsx" | "jsx" => {
                let mut c = add_use_client_directive(&content);
                c = replace_react_router(&c);
                c = fix_image_imports(&c);
                Some(c)
            }
            "ts" | "js" => {
                let has_router = RE_REACT_ROUTER.is_match(&content);
                let has_images = RE_IMAGE_IMPORT.is_match(&content);
                if has_router || has_images {
                    let mut c = content.clone();
                    if has_router {
                        c = replace_react_router(&c);
                    }
                    if has_images {
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
                let rel = path.strip_prefix(root).map_err(|_| miette!("Path not under root"))?;
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                fs.write_string(&rel_str, &new)
                    .map_err(|e| miette!("Failed to write {}: {}", rel_str, e))?;
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
