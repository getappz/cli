//! Semantic code search using Repomix and Qdrant.
//!
//! Packs project code via Repomix, chunks it, embeds with fastembed,
//! and stores in Qdrant for semantic search.

mod chunk;
mod embed;
mod error;
mod pack;
mod parse;
mod qdrant;
mod qdrant_startup;

pub use error::CodeSearchError;
pub use qdrant::SearchResult;

use std::path::Path;
use tracing::instrument;

/// Callback for progress updates during indexing. Receives human-readable step messages.
pub type IndexProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Index a project directory: pack with Repomix, chunk, embed, upsert to Qdrant.
/// Optionally report progress via `on_step` callback (e.g. for CLI spinner messages).
#[instrument(skip_all)]
pub async fn index(
    workdir: &Path,
    _force: bool,
    on_step: Option<&IndexProgressCallback>,
) -> Result<IndexResult, CodeSearchError> {
    let step = |msg: &str| {
        if let Some(f) = on_step {
            f(msg);
        }
    };

    step("Ensuring Qdrant is running...");
    qdrant_startup::ensure_qdrant_running().await?;

    step("Packing codebase with Repomix...");
    let packed_path = pack::pack(workdir).await?;

    step("Parsing packed output...");
    let files = parse::parse(&packed_path)?;

    step("Chunking files...");
    let chunks = chunk::chunk(&files)?;

    let collection = qdrant::collection_name(workdir);

    step("Generating embeddings and upserting to vector store...");
    qdrant::upsert(workdir, &collection, &chunks, on_step).await?;

    step("Writing index metadata...");
    let meta_path = meta_path(workdir)?;
    starbase_utils::json::write_file(
        &meta_path,
        &IndexMeta {
            indexed_at: chrono::Utc::now().to_rfc3339(),
            files: files.len(),
            chunks: chunks.len(),
            collection: collection.clone(),
        },
        true,
    )
    .map_err(|e| CodeSearchError(e.to_string()))?;

    Ok(IndexResult {
        indexed_files: files.len(),
        chunks: chunks.len(),
        collection,
    })
}

/// Semantic search over indexed code.
#[instrument(skip_all)]
pub async fn search(
    workdir: &Path,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    qdrant_startup::ensure_qdrant_running().await?;

    let collection = qdrant::collection_name(workdir);
    qdrant::search(&collection, query, limit.unwrap_or(10)).await
}

fn meta_path(workdir: &Path) -> Result<std::path::PathBuf, CodeSearchError> {
    use std::hash::{Hash, Hasher};
    let canonical = workdir.canonicalize().map_err(|e| e.to_string())?;
    let mut hasher = rustc_hash::FxHasher::default();
    canonical.to_string_lossy().hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    let dir = starbase_utils::dirs::home_dir()
        .ok_or("Could not determine home directory")?
        .join(".appz")
        .join("code-index")
        .join(hash);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("meta.json"))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IndexMeta {
    indexed_at: String,
    files: usize,
    chunks: usize,
    collection: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexResult {
    pub indexed_files: usize,
    pub chunks: usize,
    pub collection: String,
}
