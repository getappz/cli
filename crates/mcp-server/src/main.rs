//! MCP server binary entry point.
//!
//! Can be run standalone as `appz-mcp-server` or via `appz mcp` subcommand.

use mcp_server::run_server;

#[tokio::main]
async fn main() {
    if let Err(e) = run_server().await {
        eprintln!("MCP server error: {}", e);
        std::process::exit(1);
    }
}
