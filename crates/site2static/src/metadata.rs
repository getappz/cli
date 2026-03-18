use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

const METADATA_FILENAME: &str = ".site2static-metadata.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub file_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataCache {
    #[serde(flatten)]
    pub entries: HashMap<String, FileMetadata>,
}

impl Default for MetadataCache {
    fn default() -> Self { Self::new() }
}

impl MetadataCache {
    pub fn new() -> Self { Self { entries: HashMap::new() } }
    pub fn get(&self, url: &str) -> Option<&FileMetadata> { self.entries.get(url) }
    pub fn set(&mut self, url: String, metadata: FileMetadata) { self.entries.insert(url, metadata); }
}

pub fn load_metadata(output_dir: &Path) -> MetadataCache {
    let path = output_dir.join(METADATA_FILENAME);
    if !path.exists() { return MetadataCache::new(); }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse metadata {}: {}", path.display(), e);
            MetadataCache::new()
        }),
        Err(e) => {
            tracing::warn!("Failed to read metadata {}: {}", path.display(), e);
            MetadataCache::new()
        }
    }
}

pub fn save_metadata(output_dir: &Path, cache: &MetadataCache) {
    let path = output_dir.join(METADATA_FILENAME);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(cache) {
        Ok(content) => {
            if let Err(e) = fs::write(&path, content) {
                tracing::warn!("Failed to write metadata {}: {}", path.display(), e);
            }
        }
        Err(e) => tracing::warn!("Failed to serialize metadata: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_round_trip() {
        let dir = TempDir::new().unwrap();
        let mut cache = MetadataCache::new();
        cache.set("https://example.com/".into(), FileMetadata {
            etag: Some("W/\"abc\"".into()),
            last_modified: Some("Wed, 21 Oct 2023".into()),
            file_hash: None,
        });
        save_metadata(dir.path(), &cache);
        let loaded = load_metadata(dir.path());
        let entry = loaded.get("https://example.com/").unwrap();
        assert_eq!(entry.etag.as_deref(), Some("W/\"abc\""));
        assert_eq!(entry.last_modified.as_deref(), Some("Wed, 21 Oct 2023"));
    }

    #[test]
    fn test_metadata_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let cache = load_metadata(dir.path());
        assert!(cache.entries.is_empty());
    }
}
