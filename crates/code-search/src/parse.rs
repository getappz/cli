//! Parse Repomix markdown output into file path + content pairs.

use std::path::Path;

use crate::error::CodeSearchError;
use regex::Regex;
use tracing::instrument;

/// Extensions we want to embed (code and docs).
const EMBED_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "mts", "cts", "astro", "js", "jsx", "mjs", "cjs", "md", "mdx",
    "css", "scss",
];

/// Extensions to always exclude (binaries, lockfiles, large configs).
const EXCLUDE_EXTENSIONS: &[&str] = &[
    "lock", "png", "jpg", "jpeg", "svg", "ico", "woff", "woff2", "ttf", "eot",
    "mp4", "webm", "webp", "gif", "yaml", "yml",
];

/// Path segments that indicate build/output dirs, AI agent meta, or blog content (exclude).
const EXCLUDE_PATH_SEGMENTS: &[&str] = &[
    "node_modules",
    "dist",
    ".astro",
    "public",
    "build",
    "out",
    ".claude",
    ".cursor",
    ".codex",
    ".aider",
    ".continue",
    "data/post", // blog content, not component code
];

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub path: String,
    pub content: String,
}

fn should_embed(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    for segment in EXCLUDE_PATH_SEGMENTS {
        if path_lower.contains(segment) {
            return false;
        }
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if EXCLUDE_EXTENSIONS.iter().any(|e| ext == *e) {
        return false;
    }
    EMBED_EXTENSIONS.iter().any(|e| ext == *e)
}

/// Parse repomix markdown output. Splits on `## File: <path>` sections.
#[instrument(skip_all)]
pub fn parse(output_path: &Path) -> Result<Vec<ParsedFile>, CodeSearchError> {
    let content = std::fs::read_to_string(output_path)
        .map_err(|e| CodeSearchError(format!("Failed to read repomix output: {}", e)))?;

    // Match ## File: path/to/file
    let re = Regex::new(r"(?m)^## File: (.+)$").map_err(|e| CodeSearchError(e.to_string()))?;

    let mut files: Vec<ParsedFile> = Vec::new();
    let mut last_end = 0;

    for cap in re.captures_iter(&content) {
        let path = cap[1].trim().to_string();
        let match_start = cap.get(0).unwrap().start();
        let match_end = cap.get(0).unwrap().end();

        // Content of previous file is from last_end to match_start
        if !files.is_empty() {
            let prev_content = content[last_end..match_start].trim();
            if let Some(last) = files.last_mut() {
                last.content = prev_content.to_string();
            }
        }

        last_end = match_end;
        files.push(ParsedFile {
            path,
            content: String::new(),
        });
    }

    // Last file's content
    if let Some(last) = files.last_mut() {
        let rest = content[last_end..].trim();
        last.content = rest.to_string();
    }

    // Belt-and-suspenders: drop files we should not embed
    let files: Vec<ParsedFile> = files
        .into_iter()
        .filter(|f| should_embed(&f.path))
        .collect();

    Ok(files)
}
