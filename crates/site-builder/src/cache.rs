//! Artifact cache for the site builder pipeline.
//!
//! Each pipeline run produces JSON artifacts stored in
//! `.appz/cache/site-builder/{session_id}/`. If a phase fails,
//! re-running the command resumes from the last completed phase.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SiteBuilderResult;

/// Tracks which pipeline phases have completed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineState {
    pub session_id: String,
    pub crawl_done: bool,
    pub analyze_done: bool,
    pub scaffold_done: bool,
    pub generate_done: bool,
    pub build_done: bool,
    /// Page paths that have been successfully generated (e.g. ["/", "/about"]).
    #[serde(default)]
    pub generated_pages: Vec<String>,
}

/// Manages the artifact cache directory for a pipeline session.
pub struct ArtifactCache {
    cache_dir: PathBuf,
}

impl ArtifactCache {
    /// Create or open a cache for the given session.
    pub fn new(project_dir: &Path, session_id: &str) -> SiteBuilderResult<Self> {
        let cache_dir = project_dir
            .join(".appz")
            .join("cache")
            .join("site-builder")
            .join(session_id);
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Path to the cache directory.
    pub fn dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Write a JSON artifact.
    pub fn write_artifact<T: Serialize>(&self, name: &str, data: &T) -> SiteBuilderResult<()> {
        let path = self.cache_dir.join(name);
        let json = serde_json::to_string_pretty(data).map_err(|e| {
            crate::error::SiteBuilderError::CacheError {
                reason: format!("Failed to serialize {}: {}", name, e),
            }
        })?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Read a JSON artifact if it exists.
    pub fn read_artifact<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> SiteBuilderResult<Option<T>> {
        let path = self.cache_dir.join(name);
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&path)?;
        let data: T =
            serde_json::from_str(&contents).map_err(|e| crate::error::SiteBuilderError::CacheError {
                reason: format!("Failed to parse {}: {}", name, e),
            })?;
        Ok(Some(data))
    }

    /// Load or create the pipeline state.
    pub fn load_state(&self) -> SiteBuilderResult<PipelineState> {
        match self.read_artifact("state.json")? {
            Some(state) => Ok(state),
            None => {
                let state = PipelineState {
                    session_id: self
                        .cache_dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    ..Default::default()
                };
                self.write_artifact("state.json", &state)?;
                Ok(state)
            }
        }
    }

    /// Save the pipeline state.
    pub fn save_state(&self, state: &PipelineState) -> SiteBuilderResult<()> {
        self.write_artifact("state.json", state)
    }
}
