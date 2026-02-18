//! Qdrant client, collection management, upsert, and search.

use crate::chunk::CodeChunk;
use crate::embed::{embed, Embedder};
use crate::error::CodeSearchError;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, SearchPointsBuilder, UpsertPointsBuilder,
    VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use std::path::Path;
use tracing::instrument;

const QDRANT_DIM: u64 = 384;
const UPSERT_BATCH_SIZE: usize = 256;

fn qdrant_url() -> String {
    std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string())
}

pub fn collection_name(workdir: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let canonical = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let mut hasher = rustc_hash::FxHasher::default();
    canonical.to_string_lossy().hash(&mut hasher);
    format!("code_{:016x}", hasher.finish())
}

fn client() -> Result<Qdrant, CodeSearchError> {
    Qdrant::from_url(&qdrant_url())
        .build()
        .map_err(|e| CodeSearchError(format!("Failed to connect to Qdrant: {}", e)))
}

#[instrument(skip_all)]
pub async fn upsert<F>(
    _workdir: &Path,
    collection: &str,
    chunks: &[CodeChunk],
    on_progress: Option<&F>,
) -> Result<(), CodeSearchError>
where
    F: Fn(&str) + Send + Sync,
{
    let progress = |msg: &str| {
        if let Some(f) = on_progress {
            f(msg);
        }
    };

    if chunks.is_empty() {
        return Ok(());
    }

    let cli = client()?;

    let exists = cli
        .collection_exists(collection)
        .await
        .map_err(|e| CodeSearchError(format!("Qdrant error: {}", e)))?;

    if !exists {
        cli.create_collection(
            CreateCollectionBuilder::new(collection)
                .vectors_config(VectorParamsBuilder::new(QDRANT_DIM, Distance::Cosine)),
        )
        .await
        .map_err(|e| CodeSearchError(format!("Failed to create collection: {}", e)))?;
    }

    let mut embedder = Embedder::new()?;
    let total_batches = (chunks.len() + UPSERT_BATCH_SIZE - 1) / UPSERT_BATCH_SIZE;

    for (batch_idx, batch) in chunks.chunks(UPSERT_BATCH_SIZE).enumerate() {
        let batch_num = batch_idx + 1;
        progress(&format!(
            "Embedding batch {}/{}...",
            batch_num, total_batches
        ));

        let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
        let vectors = embedder.embed(&texts)?;

        let base_id = batch_idx * UPSERT_BATCH_SIZE;
        let points: Vec<PointStruct> = batch
            .iter()
            .zip(vectors.iter())
            .enumerate()
            .map(|(i, (chunk, vec))| {
                let payload = Payload::try_from(serde_json::json!({
                    "path": chunk.path,
                    "content": chunk.content,
                    "line_start": chunk.line_start
                }))
                .map_err(|e| CodeSearchError(format!("Invalid payload: {}", e)))?;
                Ok(PointStruct::new((base_id + i) as u64, vec.clone(), payload))
            })
            .collect::<Result<Vec<PointStruct>, CodeSearchError>>()?;

        progress(&format!(
            "Upserting batch {}/{}...",
            batch_num, total_batches
        ));

        cli.upsert_points(UpsertPointsBuilder::new(collection, points))
            .await
            .map_err(|e| CodeSearchError(format!("Failed to upsert: {}", e)))?;
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub path: String,
    pub content: String,
    pub line_start: usize,
    pub score: f32,
}

#[instrument(skip_all)]
pub async fn search(
    collection: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    let vectors = embed(&[query.to_string()])?;
    let query_vec = vectors
        .first()
        .ok_or_else(|| CodeSearchError("Empty embedding".into()))?;

    let cli = client()?;

    let exists = cli
        .collection_exists(collection)
        .await
        .map_err(|e| CodeSearchError(format!("Qdrant error: {}", e)))?;

    if !exists {
        return Ok(Vec::new());
    }

    let search_result = cli
        .search_points(
            SearchPointsBuilder::new(collection, query_vec.clone(), limit as u64)
                .with_payload(true),
        )
        .await
        .map_err(|e| CodeSearchError(format!("Search failed: {}", e)))?;

    #[derive(serde::Deserialize)]
    struct ChunkPayload {
        path: String,
        content: String,
        #[serde(default)]
        line_start: usize,
    }

    let results: Vec<SearchResult> = search_result
        .result
        .into_iter()
        .filter_map(|p| {
            let payload = Payload::from(p.payload);
            let chunk: ChunkPayload = payload.deserialize().ok()?;
            Some(SearchResult {
                path: chunk.path,
                content: chunk.content,
                line_start: chunk.line_start,
                score: p.score,
            })
        })
        .collect();

    Ok(results)
}
