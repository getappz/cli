//! LanceDB-backed vector store for code search.
//!
//! Uses sentence-transformers (all-MiniLM-L6-v2) for embeddings. No external services required.

use arrow_array::{RecordBatch, RecordBatchIterator, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::{
    connect,
    database::CreateTableMode,
    embeddings::{EmbeddingDefinition, EmbeddingRegistry, MemoryRegistry},
    query::{ExecutableQuery, QueryBase, Select},
    DistanceType,
};
use lancedb::embeddings::sentence_transformers::SentenceTransformersEmbeddingsBuilder;
use std::path::Path;
use std::sync::Arc;
use tracing::instrument;

use crate::chunk::CodeChunk;
use crate::error::CodeSearchError;
use crate::types::{SearchConfig, SearchResult};

const TABLE_NAME: &str = "code_chunks";
const EMBEDDING_NAME: &str = "st";

fn index_base_dir(workdir: &Path) -> Result<std::path::PathBuf, CodeSearchError> {
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
    Ok(dir)
}

fn lancedb_uri(workdir: &Path) -> Result<String, CodeSearchError> {
    let dir = index_base_dir(workdir)?;
    let db_path = dir.join("lancedb");
    std::fs::create_dir_all(&db_path).map_err(|e| e.to_string())?;
    Ok(db_path.to_string_lossy().to_string())
}

pub fn table_name(workdir: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let canonical = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let mut hasher = rustc_hash::FxHasher::default();
    canonical.to_string_lossy().hash(&mut hasher);
    format!("code_{:016x}", hasher.finish())
}

fn build_embedding_registry() -> Result<Arc<MemoryRegistry>, CodeSearchError> {
    let st = SentenceTransformersEmbeddingsBuilder::default().ndims(384);
    let embedding: Arc<dyn lancedb::embeddings::EmbeddingFunction> = Arc::new(
        st.build().map_err(|e| CodeSearchError(format!("Failed to build embedding: {}", e)))?,
    );
    let registry = Arc::new(MemoryRegistry::new());
    registry
        .register(EMBEDDING_NAME, embedding)
        .map_err(|e| CodeSearchError(format!("Failed to register embedding: {}", e)))?;
    Ok(registry)
}

#[instrument(skip_all)]
pub async fn upsert<F>(
    workdir: &Path,
    _table: &str,
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

    progress("Loading embedding model...");
    let registry = build_embedding_registry()?;

    let uri = lancedb_uri(workdir)?;
    let db = connect(uri.as_ref())
        .embedding_registry(registry)
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("LanceDB connect failed: {}", e)))?;

    progress("Building index...");
    let ids: Vec<u64> = (0..chunks.len() as u64).collect();
    let paths: Vec<&str> = chunks.iter().map(|c| c.path.as_str()).collect();
    let contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
    let line_starts: Vec<u64> = chunks.iter().map(|c| c.line_start as u64).collect();

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("path", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("line_start", DataType::UInt64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from(ids)),
            Arc::new(StringArray::from(paths)),
            Arc::new(StringArray::from(contents)),
            Arc::new(UInt64Array::from(line_starts)),
        ],
    )
    .map_err(|e| CodeSearchError(format!("RecordBatch failed: {}", e)))?;

    let batches = RecordBatchIterator::new(
        vec![batch].into_iter().map(Ok),
        schema.clone(),
    );

    let ed = EmbeddingDefinition {
        source_column: "content".to_string(),
        dest_column: Some("vector".to_string()),
        embedding_name: EMBEDDING_NAME.to_string(),
    };

    db.create_table(TABLE_NAME, Box::new(batches))
        .mode(CreateTableMode::Overwrite)
        .add_embedding(ed)
        .map_err(|e| CodeSearchError(format!("Add embedding failed: {}", e)))?
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("Create table failed: {}", e)))?;

    Ok(())
}

#[instrument(skip_all)]
pub async fn search(
    workdir: &Path,
    _table: &str,
    query: &str,
    config: SearchConfig,
) -> Result<Vec<SearchResult>, CodeSearchError> {
    let uri = lancedb_uri(workdir)?;
    let registry = build_embedding_registry()?;

    let db = connect(uri.as_ref())
        .embedding_registry(registry)
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("LanceDB connect failed: {}", e)))?;

    let tables = db
        .table_names()
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("List tables failed: {}", e)))?;
    if !tables.contains(&TABLE_NAME.to_string()) {
        return Ok(Vec::new());
    }

    let table = db
        .open_table(TABLE_NAME)
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("Open table failed: {}", e)))?;

    let embed_fn = db
        .embedding_registry()
        .get(EMBEDDING_NAME)
        .ok_or_else(|| CodeSearchError("Embedding not found".into()))?;

    let query_arr = Arc::new(StringArray::from(vec![query]));
    let query_vector = embed_fn
        .compute_query_embeddings(query_arr)
        .map_err(|e| CodeSearchError(format!("Query embedding failed: {}", e)))?;

    let mut search_builder = table
        .vector_search(query_vector)
        .map_err(|e| CodeSearchError(format!("Vector search failed: {}", e)))?
        .distance_type(DistanceType::Cosine)
        .limit(config.limit + if config.threshold.is_some() || config.rerank { 50 } else { 0 })
        .select(Select::columns(&["path", "content", "line_start", "_distance"]));

    if let Some(ref pred) = config.path_filter {
        search_builder = search_builder.only_if(pred.as_str());
    }

    let results = search_builder
        .execute()
        .await
        .map_err(|e| CodeSearchError(format!("Search execute failed: {}", e)))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| CodeSearchError(format!("Collect failed: {}", e)))?;

    let mut out = Vec::new();
    for batch in results {
        let path_col = batch
            .column_by_name("path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| CodeSearchError("Missing path column".into()))?;
        let content_col = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| CodeSearchError("Missing content column".into()))?;
        let line_col = batch
            .column_by_name("line_start")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>());
        let dist_col = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

        for row in 0..batch.num_rows() {
            let path = path_col.value(row).to_string();
            let content = content_col.value(row).to_string();
            let line_start = line_col
                .map(|c| c.value(row) as usize)
                .unwrap_or(1);
            let score = dist_col
                .map(|c| {
                    let d = c.value(row);
                    1.0 - d
                })
                .unwrap_or(0.0);

            out.push(SearchResult {
                path,
                content,
                line_start,
                score,
            });
        }
    }

    if let Some(threshold) = config.threshold {
        out.retain(|r| r.score >= threshold);
    }

    if config.rerank {
        rerank_by_path(&mut out);
    }

    out.truncate(config.limit);
    Ok(out)
}

/// Re-rank results to prefer source code over tests and common entry points.
fn rerank_by_path(results: &mut [SearchResult]) {
    const SOURCE_BOOST: f32 = 1.08;
    const ENTRY_BOOST: f32 = 1.05;

    for r in results.iter_mut() {
        let path_lower = r.path.to_lowercase();
        if path_lower.contains("test") || path_lower.contains("spec") || path_lower.contains("__tests__") {
            r.score *= 1.0 / SOURCE_BOOST;
        } else if path_lower.ends_with("lib.rs")
            || path_lower.ends_with("mod.rs")
            || path_lower.ends_with("index.ts")
            || path_lower.ends_with("main.rs")
        {
            r.score *= ENTRY_BOOST;
        } else if path_lower.contains("src/") && !path_lower.contains("test") {
            r.score *= SOURCE_BOOST;
        }
    }
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}
