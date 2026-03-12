//! Test LanceDB's built-in sentence-transformers embedding for code search.
//!
//! Compares LanceDB embeddings vs the existing fastembed pipeline by indexing
//! sample code chunks and running semantic search queries.

use arrow_array::{RecordBatch, RecordBatchIterator, StringArray, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::{
    connect,
    database::CreateTableMode,
    embeddings::{EmbeddingDefinition, EmbeddingRegistry, MemoryRegistry},
    query::{ExecutableQuery, QueryBase, Select},
};
use lancedb::embeddings::sentence_transformers::SentenceTransformersEmbeddingsBuilder;
use std::sync::Arc;

fn sample_chunks() -> Vec<(u64, &'static str, &'static str, u64)> {
    vec![
        (
            0,
            "src/auth/login.ts",
            r#"export async function login(email: string, password: string) {
  const user = await db.users.findFirst({ where: { email } });
  if (!user || !(await verifyPassword(password, user.hash))) {
    throw new Error("Invalid credentials");
  }
  return createSession(user.id);
}"#,
            1,
        ),
        (
            1,
            "src/api/users.ts",
            r#"export async function getUsers() {
  return db.users.findMany({ select: { id: true, email: true } });
}"#,
            1,
        ),
        (
            2,
            "src/auth/session.ts",
            r#"export function createSession(userId: string) {
  const token = generateToken(userId);
  return { token, expiresAt: Date.now() + 3600000 };
}"#,
            1,
        ),
        (
            3,
            "src/utils/db.ts",
            r#"export const db = new PrismaClient();
export async function runQuery<T>(fn: () => Promise<T>) {
  return fn();
}"#,
            1,
        ),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("lancedb_test");
    std::fs::create_dir_all(&db_path)?;
    let uri = db_path.to_string_lossy();

    // 1. Create embedding registry with sentence-transformers (all-MiniLM-L6-v2)
    println!("Loading sentence-transformers model (all-MiniLM-L6-v2)...");
    // Default model: sentence-transformers/all-MiniLM-L6-v2 (384 dims)
    let st = SentenceTransformersEmbeddingsBuilder::default().ndims(384);
    let embedding: Arc<dyn lancedb::embeddings::EmbeddingFunction> = Arc::new(st.build()?);
    let registry = Arc::new(MemoryRegistry::new());
    registry.register("st", embedding)?;

    // 2. Connect with embedding registry
    let db = connect(uri.as_ref())
        .embedding_registry(registry)
        .execute()
        .await?;

    // 3. Build RecordBatch from sample chunks
    let chunks = sample_chunks();
    let ids: Vec<u64> = chunks.iter().map(|(id, _, _, _)| *id).collect();
    let paths: Vec<&str> = chunks.iter().map(|(_, p, _, _)| *p).collect();
    let contents: Vec<&str> = chunks.iter().map(|(_, _, c, _)| *c).collect();
    let line_starts: Vec<u64> = chunks.iter().map(|(_, _, _, s)| *s).collect();

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
    )?;

    let batches = RecordBatchIterator::new(
        vec![batch].into_iter().map(Ok),
        schema.clone(),
    );

    // 4. Create table with embedding on "content" -> "vector"
    println!("Creating table with embedding...");
    let ed = EmbeddingDefinition {
        source_column: "content".to_string(),
        dest_column: Some("vector".to_string()),
        embedding_name: "st".to_string(),
    };

    let table = db
        .create_table("code_chunks", Box::new(batches))
        .mode(CreateTableMode::Overwrite)
        .add_embedding(ed)?
        .execute()
        .await?;

    println!("Indexed {} chunks", chunks.len());

    // 5. Run search - embed query and use vector_search (accepts Array directly)
    let registry_ref = db.embedding_registry();
    let embed_fn = registry_ref.get("st").ok_or("embedding not found")?;
    let query_text = "user authentication login";
    let query_arr = Arc::new(StringArray::from(vec![query_text]));
    let query_vector = embed_fn.compute_query_embeddings(query_arr)?;

    println!("\nSearch: \"{}\"", query_text);
    let results = table
        .vector_search(query_vector)?
        .limit(3)
        .select(Select::columns(&["path", "content", "line_start"]))
        .execute()
        .await?
        .try_collect::<Vec<_>>()
        .await?;

    for (i, batch) in results.iter().enumerate() {
        let path_col = batch
            .column_by_name("path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .unwrap();
        let content_col = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .unwrap();
        for row in 0..batch.num_rows() {
            let path = path_col.value(row);
            let content = content_col.value(row);
            let preview = if content.len() > 120 {
                format!("{}...", &content[..117])
            } else {
                content.to_string()
            };
            println!("  {}: {} -> {}", i + 1, path, preview.replace('\n', " "));
        }
    }

    println!("\nLanceDB embedding test complete.");
    Ok(())
}
