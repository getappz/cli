//! "Markdown for Agents" at deploy time: every scanned `.html` gets a sibling
//! `.md` that is uploaded as an ordinary blob, so the serve worker can answer
//! `Accept: text/markdown` straight from storage.

use crate::static_cas::{LocalBlob, ScannedFileEntry, ScannedTree, sha256_hex};
use std::collections::HashSet;
use std::path::Path;
use tempfile::TempDir;

const MARKDOWN_MIME: &str = "text/markdown; charset=utf-8";

/// Owns the generated `.md` files; keep it alive until uploads finish, since
/// the [`LocalBlob`] paths in the tree point into it.
#[derive(Debug)]
pub struct SiblingGuard {
    _dir: TempDir,
    /// How many siblings were generated.
    pub generated: usize,
}

/// `/about/index.html` → `/about/index.md`, `/about.html` → `/about.md`;
/// anything that is not `.html` has no sibling. Mirrors `markdownSiblingPath`
/// on the serve side — change both together.
pub fn markdown_sibling_path(html_path: &str) -> Option<String> {
    html_path
        .strip_suffix(".html")
        .map(|stem| format!("{stem}.md"))
}

/// The URL a page is served at, for resolving its relative links.
fn page_url(origin: &url::Url, html_path: &str) -> Option<url::Url> {
    let served = match html_path.strip_suffix("/index.html") {
        Some(dir) => format!("{dir}/"),
        None => html_path.strip_suffix(".html")?.to_owned(),
    };
    origin.join(&served).ok()
}

/// Convert each `.html` in `tree` and add the result as a `.md` entry. Pages
/// that fail to convert (not UTF-8, empty, too large) are skipped; an existing
/// file at the sibling path is never replaced. `origin` of `None` leaves links
/// relative.
pub fn add_markdown_siblings(
    output_dir: &Path,
    tree: &mut ScannedTree,
    origin: Option<&url::Url>,
) -> Result<SiblingGuard, String> {
    let dir = TempDir::new().map_err(|e| format!("cannot create markdown scratch dir: {e}"))?;
    let existing: HashSet<String> = tree.files.iter().map(|f| f.path.clone()).collect();

    let mut pages: Vec<(String, String)> = tree
        .files
        .iter()
        .filter_map(|f| {
            let sibling = markdown_sibling_path(&f.path)?;
            (!existing.contains(&sibling)).then(|| (f.path.clone(), sibling))
        })
        .collect();
    pages.sort();

    let mut generated = 0;
    for (html_path, md_path) in pages {
        let Ok(html) = std::fs::read(output_dir.join(html_path.trim_start_matches('/'))) else {
            continue;
        };
        let converted = match origin {
            Some(o) => match page_url(o, &html_path) {
                Some(url) => appz_html2md::convert(&html, &url),
                None => appz_html2md::convert_relative(&html),
            },
            None => appz_html2md::convert_relative(&html),
        };
        let Ok(converted) = converted else { continue };

        let bytes = converted.markdown.into_bytes();
        let hash = sha256_hex(&bytes);
        let blob_path = dir.path().join(format!("{hash}.md"));
        if !blob_path.exists() {
            std::fs::write(&blob_path, &bytes)
                .map_err(|e| format!("cannot write {}: {e}", blob_path.display()))?;
        }
        tree.total_bytes += bytes.len();
        tree.blobs_by_hash
            .entry(hash.clone())
            .or_insert_with(|| LocalBlob {
                hash: hash.clone(),
                path: blob_path,
            });
        tree.files.push(ScannedFileEntry {
            path: md_path,
            hash,
            size: bytes.len(),
            r#type: MARKDOWN_MIME.to_owned(),
            immutable: None,
        });
        generated += 1;
    }
    Ok(SiblingGuard {
        _dir: dir,
        generated,
    })
}
