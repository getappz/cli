//! Search request and result types.

use serde::{Deserialize, Serialize};

/// Search request — whitelisted fields, no arbitrary flags.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub is_regex: Option<bool>,
    /// Filter results to source files matching this glob (e.g. "*.rs")
    pub file_glob: Option<String>,
    pub max_results: Option<usize>,
}

/// One match from ripgrep — line-in-file, column, snippet.
#[derive(Debug, Clone, Serialize)]
pub struct RawMatch {
    pub line: u64,
    pub column: Option<u64>,
    pub snippet: String,
}

/// Search result with source file path (mapped from pack section).
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub file: String,
    pub line: u64,
    pub column: Option<u64>,
    pub snippet: String,
}
