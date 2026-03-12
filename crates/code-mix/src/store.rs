//! Git-style content-addressable store for pack cache.
//!
//! Layout: ~/.appz/store/objects/ab/cd1234... with SQLite index at index.db.
//! Override via APPZ_STORE_DIR.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::repomix::RepomixError;

const STORE_SUBDIR: &str = "store";
const OBJECTS_DIR: &str = "objects";
const INDEX_FILENAME: &str = "index.db";

const SCHEMA_VERSION: i32 = 2;

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS cache_index (
  input_key TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL,
  created_at INTEGER,
  workdir TEXT,
  style TEXT,
  file_count INTEGER,
  workspace TEXT
);

CREATE INDEX IF NOT EXISTS idx_content_hash ON cache_index(content_hash);
";


/// Store root: ~/.appz/store or APPZ_STORE_DIR.
pub fn store_root() -> Result<PathBuf, RepomixError> {
    if let Ok(dir) = std::env::var("APPZ_STORE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::home_dir()
        .map(|h| h.join(".appz").join(STORE_SUBDIR))
        .ok_or_else(|| RepomixError("Could not determine home directory".into()))
}

/// Path to object: objects/{hash[0..2]}/{hash}.
pub fn object_path(store_root: &Path, content_hash: &str) -> PathBuf {
    let shard = if content_hash.len() >= 2 {
        &content_hash[..2]
    } else {
        "00"
    };
    store_root.join(OBJECTS_DIR).join(shard).join(content_hash)
}

/// Path to index.db.
pub fn index_path(store_root: &Path) -> PathBuf {
    store_root.join(INDEX_FILENAME)
}

/// Ensure store dirs exist (store_root, store_root/objects).
pub fn ensure_store_dirs(store_root: &Path) -> Result<(), RepomixError> {
    let objects = store_root.join(OBJECTS_DIR);
    std::fs::create_dir_all(&objects)
        .map_err(|e| RepomixError(format!("Failed to create store dir: {}", e)))?;
    Ok(())
}

/// Initialize index schema and run migrations.
pub fn init_index(conn: &Connection) -> Result<(), RepomixError> {
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| RepomixError(format!("Failed to init index: {}", e)))?;

    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if version < 2 {
        for sql in [
            "ALTER TABLE cache_index ADD COLUMN workdir TEXT",
            "ALTER TABLE cache_index ADD COLUMN style TEXT",
            "ALTER TABLE cache_index ADD COLUMN file_count INTEGER",
            "ALTER TABLE cache_index ADD COLUMN workspace TEXT",
        ] {
            if conn.execute(sql, []).is_err() {
                // Ignore "duplicate column name" (column already exists)
            }
        }
        conn.execute(
            &format!("PRAGMA user_version = {}", SCHEMA_VERSION),
            [],
        )
        .map_err(|e| RepomixError(format!("Failed to set schema version: {}", e)))?;
    }
    Ok(())
}

/// Open index connection (creates file if missing).
pub fn open_index(store_root: &Path) -> Result<Connection, RepomixError> {
    ensure_store_dirs(store_root)?;
    let path = index_path(store_root);
    Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|e| RepomixError(format!("Failed to open index: {}", e)))
    .and_then(|conn| {
        init_index(&conn)?;
        Ok(conn)
    })
}

/// Get content_hash for input_key.
pub fn get_content_hash(
    conn: &Connection,
    input_key: &str,
) -> Result<Option<String>, RepomixError> {
    let mut stmt = conn
        .prepare("SELECT content_hash FROM cache_index WHERE input_key = ?")
        .map_err(|e| RepomixError(format!("Failed to prepare query: {}", e)))?;
    let mut rows = stmt
        .query([input_key])
        .map_err(|e| RepomixError(format!("Failed to query: {}", e)))?;
    if let Some(row) = rows.next().map_err(|e| RepomixError(e.to_string()))? {
        let hash: String = row.get(0).map_err(|e| RepomixError(e.to_string()))?;
        Ok(Some(hash))
    } else {
        Ok(None)
    }
}

/// Metadata for a cached pack (user-friendly display).
#[derive(Debug, Clone, Default)]
pub struct PackMetadata {
    pub workdir: Option<String>,
    pub style: Option<String>,
    pub file_count: Option<i64>,
    pub workspace: Option<String>,
}

/// Insert index entry with metadata.
pub fn insert_index(
    conn: &Connection,
    input_key: &str,
    content_hash: &str,
    created_at: i64,
    meta: &PackMetadata,
) -> Result<(), RepomixError> {
    conn.execute(
        "INSERT OR REPLACE INTO cache_index (input_key, content_hash, created_at, workdir, style, file_count, workspace) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            input_key,
            content_hash,
            created_at,
            meta.workdir,
            meta.style,
            meta.file_count,
            meta.workspace,
        ],
    )
    .map_err(|e| RepomixError(format!("Failed to insert index: {}", e)))?;
    Ok(())
}

/// Full list entry with metadata.
#[derive(Debug, Clone)]
pub struct ListEntry {
    pub input_key: String,
    pub content_hash: String,
    pub created_at: i64,
    pub workdir: Option<String>,
    pub style: Option<String>,
    pub file_count: Option<i64>,
    pub workspace: Option<String>,
}

/// List all entries with metadata.
pub fn list_entries(conn: &Connection) -> Result<Vec<ListEntry>, RepomixError> {
    let mut stmt = conn
        .prepare(
            "SELECT input_key, content_hash, COALESCE(created_at, 0), workdir, style, file_count, workspace \
             FROM cache_index ORDER BY created_at DESC",
        )
        .map_err(|e| RepomixError(format!("Failed to prepare list: {}", e)))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ListEntry {
                input_key: row.get(0)?,
                content_hash: row.get(1)?,
                created_at: row.get(2)?,
                workdir: row.get(3)?,
                style: row.get(4)?,
                file_count: row.get(5)?,
                workspace: row.get(6)?,
            })
        })
        .map_err(|e| RepomixError(format!("Failed to list: {}", e)))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| RepomixError(e.to_string()))?);
    }
    Ok(out)
}

/// Remove index rows by content_hash.
pub fn remove_by_hash(conn: &Connection, hashes: &[String]) -> Result<(), RepomixError> {
    for h in hashes {
        conn.execute("DELETE FROM cache_index WHERE content_hash = ?", [h])
            .map_err(|e| RepomixError(format!("Failed to delete: {}", e)))?;
    }
    Ok(())
}

/// Remove all index rows.
pub fn remove_all(conn: &Connection) -> Result<(), RepomixError> {
    conn.execute("DELETE FROM cache_index", [])
        .map_err(|e| RepomixError(format!("Failed to delete all: {}", e)))?;
    Ok(())
}

/// Delete object file and remove empty shard dir.
pub fn delete_object(store_root: &Path, content_hash: &str) -> Result<(), RepomixError> {
    let path = object_path(store_root, content_hash);
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| RepomixError(format!("Failed to delete object: {}", e)))?;
        if let Some(shard_dir) = path.parent() {
            if shard_dir.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false) {
                let _ = std::fs::remove_dir(shard_dir);
            }
        }
    }
    Ok(())
}

/// Check if object exists.
pub fn object_exists(store_root: &Path, content_hash: &str) -> bool {
    object_path(store_root, content_hash).exists()
}

/// Get object size in bytes.
pub fn object_size(store_root: &Path, content_hash: &str) -> Option<u64> {
    std::fs::metadata(object_path(store_root, content_hash))
        .ok()
        .map(|m| m.len())
}
