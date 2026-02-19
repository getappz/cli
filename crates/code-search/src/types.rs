//! Shared types for code search results.

/// Search configuration for semantic code search.
///
/// Controls result limits, relevance filtering, path scoping, and re-ranking.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to return.
    pub limit: usize,
    /// Minimum similarity score (0.0 to 1.0). Results below this are dropped.
    pub threshold: Option<f32>,
    /// Enable path-based re-ranking to prefer source over tests.
    pub rerank: bool,
    /// SQL predicate for path column, e.g. `path LIKE 'src/%'` or `path LIKE '%.rs'`.
    pub path_filter: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            limit: 10,
            threshold: None,
            rerank: false,
            path_filter: None,
        }
    }
}

impl SearchConfig {
    /// Create config with the given limit and defaults for other options.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub path: String,
    pub content: String,
    pub line_start: usize,
    pub score: f32,
}
