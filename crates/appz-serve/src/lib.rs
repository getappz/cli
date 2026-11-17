//! Static file server for plain HTML projects (`appz dev` / `appz preview`).

mod config;
mod error;
mod file_cache;
mod reload;
mod server;
mod static_files;
mod watcher;

pub use config::{ServerConfig, find_available_port};
pub use error::{Result, ServeError};
pub use server::DevServer;

/// Run until interrupted (Ctrl+C). Prints the URL to stdout via the caller.
pub fn run_blocking(config: ServerConfig) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(ServeError::Io)?;
    rt.block_on(async {
        let mut server = DevServer::new(config.clone())?;
        server.start().await
    })
}

