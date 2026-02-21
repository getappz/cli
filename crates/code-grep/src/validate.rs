//! Input validation for search requests.

use anyhow::Result;

use crate::schema::SearchRequest;

/// Validate search request — reject empty, too long, or dangerous inputs.
pub fn validate_request(req: &SearchRequest) -> Result<()> {
    if req.query.trim().is_empty() {
        anyhow::bail!("Query cannot be empty");
    }
    if req.query.len() > 500 {
        anyhow::bail!("Query too long (max 500 chars)");
    }
    if let Some(ref glob) = req.file_glob {
        if glob.contains("..") {
            anyhow::bail!("Invalid glob: '..' not allowed");
        }
    }
    Ok(())
}
