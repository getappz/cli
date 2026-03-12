//! Content-hash result caching for incremental checks.
//!
//! Caches check results by file content hash so unchanged files can be
//! skipped on subsequent runs. The cache is stored in `.appz/check-cache.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rustc_hash::FxHasher;
use serde::{Deserialize, Serialize};
use std::hash::Hasher;

use crate::error::{CheckResult, CheckerError};
use crate::output::CheckIssue;

/// Cache entry for a single file + provider combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// FxHash of the file content.
    content_hash: u64,
    /// Cached issues found by the provider.
    issues: Vec<CheckIssue>,
}

/// The check cache — maps `(file, provider_slug)` to cached results.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckCache {
    /// Version of the cache format (for migration).
    version: u32,
    /// Entries keyed by "file::provider".
    entries: HashMap<String, CacheEntry>,
}

impl CheckCache {
    /// Current cache format version.
    const VERSION: u32 = 1;

    /// Load the cache from disk, or return an empty cache.
    pub fn load(project_dir: &Path) -> Self {
        let cache_path = Self::cache_path(project_dir);
        if !cache_path.exists() {
            return Self::new();
        }

        match std::fs::read_to_string(&cache_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| Self::new()),
            Err(_) => Self::new(),
        }
    }

    /// Save the cache to disk.
    pub fn save(&self, project_dir: &Path) -> CheckResult<()> {
        let cache_path = Self::cache_path(project_dir);

        // Ensure .appz directory exists.
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CheckerError::CacheError {
                reason: format!("Failed to create cache directory: {}", e),
            })?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| CheckerError::CacheError {
                reason: format!("Failed to serialize cache: {}", e),
            })?;

        std::fs::write(&cache_path, content).map_err(|e| CheckerError::CacheError {
            reason: format!("Failed to write cache: {}", e),
        })?;

        Ok(())
    }

    /// Look up cached issues for a file + provider.
    ///
    /// Returns `Some(issues)` if the file content hasn't changed since last check.
    pub fn lookup(
        &self,
        file_path: &str,
        provider_slug: &str,
        current_hash: u64,
    ) -> Option<&Vec<CheckIssue>> {
        let key = Self::make_key(file_path, provider_slug);
        self.entries.get(&key).and_then(|entry| {
            if entry.content_hash == current_hash {
                Some(&entry.issues)
            } else {
                None
            }
        })
    }

    /// Store issues for a file + provider.
    pub fn store(
        &mut self,
        file_path: &str,
        provider_slug: &str,
        content_hash: u64,
        issues: Vec<CheckIssue>,
    ) {
        let key = Self::make_key(file_path, provider_slug);
        self.entries.insert(
            key,
            CacheEntry {
                content_hash,
                issues,
            },
        );
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn new() -> Self {
        Self {
            version: Self::VERSION,
            entries: HashMap::new(),
        }
    }

    fn cache_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".appz").join("check-cache.json")
    }

    fn make_key(file_path: &str, provider_slug: &str) -> String {
        format!("{}::{}", file_path, provider_slug)
    }
}

/// Hash file content using FxHash (extremely fast, non-cryptographic).
pub fn hash_content(content: &[u8]) -> u64 {
    let mut hasher = FxHasher::default();
    hasher.write(content);
    hasher.finish()
}
