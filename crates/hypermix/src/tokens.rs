//! Token counting for generated XML files.
//! Uses chars/4 approximation (can upgrade to tiktoken-rs later).

use std::path::Path;

use miette::IntoDiagnostic;

/// Count approximate tokens: chars / 4 (GPT-4 style).
pub fn count_tokens(content: &str) -> u64 {
    (content.len() as u64 + 3) / 4
}

/// Count tokens per file. Returns (filename -> tokens, total).
pub fn count_tokens_in_files(paths: &[impl AsRef<Path>]) -> miette::Result<(Vec<(String, u64)>, u64)> {
    let mut results = Vec::new();
    let mut total = 0u64;

    for p in paths {
        let path = p.as_ref();
        if path.exists() {
            let content = std::fs::read_to_string(path).into_diagnostic()?;
            let tokens = count_tokens(&content);
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            results.push((name, tokens));
            total += tokens;
        }
    }

    Ok((results, total))
}
