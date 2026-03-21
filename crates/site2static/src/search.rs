//! Pagefind CLI invocation for search index generation.

use std::path::Path;
use std::process::Command;

use crate::MirrorError;

/// Check whether the `pagefind` binary is available on PATH.
/// Returns the path to the binary, or an error with install instructions.
pub fn check_pagefind() -> Result<std::path::PathBuf, MirrorError> {
    which::which("pagefind").map_err(|_| MirrorError::SearchBinaryNotFound {
        binary: "pagefind".into(),
        hint: "Install via `mise use -g pagefind` or `npm install -g pagefind`".into(),
    })
}

/// Run `pagefind --site <output_dir> --bundle-dir pagefind` to build the search index.
/// Returns the number of pages indexed (parsed from stdout), or 0 if unparseable.
pub fn run_pagefind(output_dir: &Path) -> Result<usize, MirrorError> {
    let bin = check_pagefind()?;

    let output = Command::new(&bin)
        .arg("--site")
        .arg(output_dir)
        .arg("--bundle-dir")
        .arg("pagefind")
        .output()
        .map_err(|e| MirrorError::SearchIndexingFailed {
            message: format!("failed to execute pagefind: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MirrorError::SearchIndexingFailed {
            message: format!("pagefind exited with {}: {}", output.status, stderr.trim()),
        });
    }

    // Parse page count from stdout. Pagefind prints something like:
    // "Running Pagefind ... on 42 page(s)"
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pages = parse_page_count(&stdout);
    tracing::info!("Pagefind indexed {} pages in {}", pages, output_dir.display());
    Ok(pages)
}

/// Extract page count from pagefind stdout.
fn parse_page_count(stdout: &str) -> usize {
    // Look for pattern like "42 page" in output
    for word in stdout.split_whitespace().collect::<Vec<_>>().windows(2) {
        if word[1].starts_with("page") {
            if let Ok(n) = word[0].parse::<usize>() {
                return n;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_count_from_pagefind_output() {
        assert_eq!(parse_page_count("Running Pagefind v1.4.0 on 42 page(s)"), 42);
    }

    #[test]
    fn returns_zero_for_unparseable_output() {
        assert_eq!(parse_page_count("no match here"), 0);
        assert_eq!(parse_page_count(""), 0);
    }

    #[test]
    fn parses_single_page() {
        assert_eq!(parse_page_count("Indexed 1 page"), 1);
    }
}
