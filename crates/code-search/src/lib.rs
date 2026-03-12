//! Semantic code search using Repomix and LanceDB.
//!
//! Packs project code via Repomix, chunks it, embeds with sentence-transformers,
//! and stores in LanceDB for semantic search. Qdrant backend is optional.

mod chunk;
mod error;
mod pack;
mod parse;
mod types;

#[cfg(feature = "lancedb")]
mod lancedb_store;
#[cfg(feature = "qdrant")]
mod embed;
#[cfg(feature = "qdrant")]
mod qdrant;
#[cfg(feature = "qdrant")]
mod qdrant_startup;

pub use error::CodeSearchError;
pub use types::{SearchConfig, SearchResult};

use std::path::Path;
use tracing::instrument;

/// Callback for progress updates during indexing. Receives human-readable step messages.
pub type IndexProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Index a project directory: pack with Repomix, chunk, embed, upsert to vector store.
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

    step("Packing codebase with Repomix...");
    let packed_path = pack::pack(workdir).await?;

    step("Parsing packed output...");
    let files = parse::parse(&packed_path)?;

    step("Chunking files...");
    let chunks = chunk::chunk(&files)?;

    let collection = table_name(workdir);

    step("Generating embeddings and upserting to vector store...");
    upsert(workdir, &collection, &chunks, on_step).await?;

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
    let collection = table_name(workdir);
    let config = SearchConfig::with_limit(limit.unwrap_or(10));
    do_search(workdir, &collection, query, config).await
}

/// Semantic search with full configuration.
#[instrument(skip_all)]
pub async fn search_with_config(
    workdir: &Path,
    query: &str,
    config: SearchConfig,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    let collection = table_name(workdir);
    do_search(workdir, &collection, query, config).await
}

#[cfg(all(feature = "lancedb", feature = "qdrant"))]
compile_error!("code-search: features 'lancedb' and 'qdrant' are mutually exclusive");

#[cfg(feature = "lancedb")]
fn table_name(workdir: &Path) -> String {
    lancedb_store::table_name(workdir)
}

#[cfg(all(feature = "qdrant", not(feature = "lancedb")))]
fn table_name(workdir: &Path) -> String {
    qdrant::collection_name(workdir)
}

#[cfg(feature = "lancedb")]
async fn upsert<F>(
    workdir: &Path,
    table: &str,
    chunks: &[chunk::CodeChunk],
    on_step: Option<&F>,
) -> Result<(), CodeSearchError>
where
    F: Fn(&str) + Send + Sync,
{
    lancedb_store::upsert(workdir, table, chunks, on_step).await
}

#[cfg(all(feature = "qdrant", not(feature = "lancedb")))]
async fn upsert<F>(
    workdir: &Path,
    table: &str,
    chunks: &[chunk::CodeChunk],
    on_step: Option<&F>,
) -> Result<(), CodeSearchError>
where
    F: Fn(&str) + Send + Sync,
{
    qdrant_startup::ensure_qdrant_running().await?;
    qdrant::upsert(workdir, table, chunks, on_step).await
}

#[cfg(feature = "lancedb")]
async fn do_search(
    workdir: &Path,
    table: &str,
    query: &str,
    config: SearchConfig,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    lancedb_store::search(workdir, table, query, config).await
}

#[cfg(all(feature = "qdrant", not(feature = "lancedb")))]
async fn do_search(
    workdir: &Path,
    table: &str,
    query: &str,
    config: SearchConfig,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    qdrant_startup::ensure_qdrant_running().await?;
    qdrant::search(table, query, config.limit).await
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
