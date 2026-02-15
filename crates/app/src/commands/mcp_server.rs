//! Run the MCP server for AI assistants.

use crate::session::AppzSession;
use starbase::AppResult;

pub async fn mcp_server(_session: AppzSession) -> AppResult {
    mcp_server::run_server()
        .await
        .map_err(|e| miette::miette!("MCP server error: {}", e))?;
    Ok(None)
}
