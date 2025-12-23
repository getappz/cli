//! Database module for SQLite-based incremental SEO analysis

use rusqlite::{Connection, Result};
use crate::models::SeoIssue;
use crate::registry::lookup;

pub const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_pages.sql")),
    (3, include_str!("../migrations/0003_issue_definitions.sql")),
    (4, include_str!("../migrations/0004_page_issues.sql")),
];

/// Open or create a SQLite database with optimized PRAGMA settings
pub fn open_db(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        "#,
    )?;
    Ok(conn)
}

/// Run migrations to bring the database up to date  
pub fn migrate(conn: &mut Connection) -> Result<()> {
    // Get current version - if schema_version table doesn't exist, start from 0
    // We use a separate statement to ensure borrow is released before transaction
    let current: i64 = match conn.prepare("SELECT COALESCE(MAX(version), 0) FROM schema_version") {
        Ok(mut stmt) => stmt.query_row([], |row| row.get(0)).unwrap_or(0),
        Err(_) => 0, // Table doesn't exist, start from version 0
    };
    
    // Now start transaction (previous borrow should be released)
    let tx = conn.transaction()?;

    for (version, sql) in MIGRATIONS {
        if *version > current {
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_version (version, applied_at)
                 VALUES (?, strftime('%s','now'))",
                [version],
            )?;
        }
    }

    tx.commit()
}

/// Compute blake3 hash of HTML content
pub fn compute_hash(html: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(html.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Check if a page should be analyzed based on content hash
pub fn should_analyze(
    conn: &Connection,
    url: &str,
    new_hash: &str,
) -> Result<bool> {
    let result: Result<String, _> = conn.query_row(
        "SELECT content_hash FROM pages WHERE url = ?",
        [url],
        |row| row.get(0),
    );

    match result {
        Ok(old_hash) => Ok(old_hash != new_hash),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
        Err(e) => Err(e),
    }
}

/// Insert or update a page record
pub fn upsert_page(
    conn: &Connection,
    url: &str,
    hash: &str,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO pages (url, content_hash, last_analyzed_at, last_seen_at)
        VALUES (?, ?, strftime('%s','now'), strftime('%s','now'))
        ON CONFLICT(url) DO UPDATE SET
            content_hash = excluded.content_hash,
            last_analyzed_at = excluded.last_analyzed_at,
            last_seen_at = excluded.last_seen_at
        "#,
        (url, hash),
    )?;
    Ok(())
}

/// Persist issues for a page, tracking occurrences
pub fn persist_issues(
    conn: &Connection,
    url: &str,
    issues: &[SeoIssue],
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    for issue in issues {
        conn.execute(
            r#"
            INSERT INTO page_issues (url, issue_code, first_seen_at, last_seen_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(url, issue_code) DO UPDATE SET
                last_seen_at = excluded.last_seen_at,
                occurrences = occurrences + 1
            "#,
            (url, issue.code, now, now),
        )?;
    }
    Ok(())
}

/// Database write plan for batch operations
/// Collects all DB operations and executes them in a single transaction
#[derive(Debug, Default)]
pub struct AuditDbPlan {
    /// Pages to upsert (url, hash)
    pub pages: Vec<(String, String)>,
    /// Issues to persist (url, issue_code)
    pub issues: Vec<(String, &'static str)>,
    /// Page hashes to check (url, hash) - for should_analyze checks
    pub page_hashes: Vec<(String, String)>,
}

impl AuditDbPlan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a page upsert operation
    pub fn add_page(&mut self, url: String, hash: String) {
        self.pages.push((url, hash));
    }

    /// Add an issue to persist
    pub fn add_issue(&mut self, url: String, issue_code: &'static str) {
        self.issues.push((url, issue_code));
    }

    /// Add a page hash check
    pub fn add_page_hash(&mut self, url: String, hash: String) {
        self.page_hashes.push((url, hash));
    }
}

/// Execute the database write plan in a single transaction
pub fn execute_plan(conn: &mut Connection, plan: AuditDbPlan) -> Result<()> {
    let tx = conn.transaction()?;

    // Batch upsert pages
    if !plan.pages.is_empty() {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO pages (url, content_hash, last_analyzed_at, last_seen_at)
            VALUES (?, ?, strftime('%s','now'), strftime('%s','now'))
            ON CONFLICT(url) DO UPDATE SET
                content_hash = excluded.content_hash,
                last_analyzed_at = excluded.last_analyzed_at,
                last_seen_at = excluded.last_seen_at
            "#,
        )?;

        for (url, hash) in plan.pages {
            stmt.execute((&url, &hash))?;
        }
    }

    // Batch persist issues
    if !plan.issues.is_empty() {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO page_issues (url, issue_code, first_seen_at, last_seen_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(url, issue_code) DO UPDATE SET
                last_seen_at = excluded.last_seen_at,
                occurrences = occurrences + 1
            "#,
        )?;

        for (url, issue_code) in plan.issues {
            stmt.execute((&url, issue_code, &now, &now))?;
        }
    }

    tx.commit()?;
    Ok(())
}

/// Batch check which pages should be analyzed
/// Returns a map of url -> should_analyze boolean
/// 
/// Note: For large batches, this could be optimized further with a temporary table,
/// but for typical use cases (hundreds of pages), this approach is sufficient.
pub fn batch_should_analyze(
    conn: &Connection,
    page_hashes: &[(String, String)],
) -> Result<std::collections::HashMap<String, bool>> {
    let mut result = std::collections::HashMap::new();

    if page_hashes.is_empty() {
        return Ok(result);
    }

    // Query all relevant pages in one go
    // SQLite doesn't support array parameters well, so we use individual queries
    // but within a read transaction for better performance
    let tx = conn.unchecked_transaction()?;
    
    let mut stmt = tx.prepare("SELECT content_hash FROM pages WHERE url = ?")?;
    
    for (url, new_hash) in page_hashes {
        let should_analyze = match stmt.query_row([url.as_str()], |row| row.get::<_, String>(0)) {
            Ok(old_hash) => old_hash != *new_hash,
            Err(rusqlite::Error::QueryReturnedNoRows) => true,
            Err(e) => return Err(e),
        };
        result.insert(url.clone(), should_analyze);
    }
    
    drop(stmt);
    tx.commit()?;

    Ok(result)
}

/// Load issues for a page from the database
/// Reconstructs SeoIssue objects from stored issue codes using the registry
pub fn load_issues(conn: &Connection, url: &str) -> Result<Vec<SeoIssue>> {
    let mut stmt = conn.prepare("SELECT issue_code FROM page_issues WHERE url = ?")?;
    let issue_codes: Vec<String> = stmt
        .query_map([url], |row| {
            let code: String = row.get(0)?;
            Ok(code)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut issues = Vec::new();
    for code in issue_codes {
        if let Some(def) = lookup(&code) {
            // Create a minimal SeoIssue with generic message
            // The exact message depends on page content, but this is sufficient for aggregation
            let message = match def.code {
                "SEO-META-001" => "Missing <title> tag".to_string(),
                "SEO-META-002" => "Missing meta description".to_string(),
                "SEO-H1-001" => "No H1 found".to_string(),
                "SEO-H1-002" => "Multiple H1 tags found".to_string(),
                "SEO-CONTENT-001" => "Thin content".to_string(),
                "SEO-IMG-001" => "Image missing alt text".to_string(),
                "SEO-IMG-002" => "Image missing loading attribute".to_string(),
                "SEO-LINK-001" => "Link has empty text".to_string(),
                _ => format!("Issue: {}", def.code),
            };
            
            let hint = match def.code {
                "SEO-META-001" => Some("Add a 50–60 character title".to_string()),
                "SEO-META-002" => Some("Add a 140–160 character description".to_string()),
                "SEO-H1-001" => Some("Add exactly one H1".to_string()),
                "SEO-H1-002" => Some("Keep exactly one H1".to_string()),
                "SEO-CONTENT-001" => Some("Expand content to at least 300 words".to_string()),
                "SEO-IMG-001" => Some("Add descriptive alt text".to_string()),
                "SEO-IMG-002" => Some("Use loading=lazy".to_string()),
                "SEO-LINK-001" => Some("Use descriptive anchor text".to_string()),
                _ => None,
            };

            issues.push(SeoIssue {
                code: def.code,
                severity: def.severity.clone(),
                message,
                hint,
                selector: None,
                suggestion: None,
            });
        }
    }

    Ok(issues)
}

