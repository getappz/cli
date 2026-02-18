//! Embedding via fastembed AllMiniLML6V2.

use crate::error::CodeSearchError;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use tracing::instrument;

const MODEL: EmbeddingModel = EmbeddingModel::AllMiniLML6V2;

pub fn dimension() -> u64 {
    384
}

/// Embedder holds the model for reuse across multiple embed calls (e.g. batched upserts).
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self, CodeSearchError> {
        let model = TextEmbedding::try_new(
            TextInitOptions::new(MODEL).with_show_download_progress(false),
        )
        .map_err(|e| CodeSearchError(format!("Failed to load embedding model: {}", e)))?;
        Ok(Self { model })
    }

    #[instrument(skip_all)]
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, CodeSearchError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let docs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        self.model
            .embed(docs, None)
            .map_err(|e| CodeSearchError(format!("Embedding failed: {}", e)))
    }
}

/// Embed texts (convenience for single-shot use, e.g. search). For batched indexing, use Embedder.
#[instrument(skip_all)]
pub fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>, CodeSearchError> {
    Embedder::new()?.embed(texts)
}
