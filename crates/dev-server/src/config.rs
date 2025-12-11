use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the dev server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server address (default: "127.0.0.1")
    pub address: String,
    /// Server port (default: 3000)
    pub port: u16,
    /// Root directory to serve files from
    pub root_dir: PathBuf,
    /// Enable hot reload via WebSocket
    pub hot_reload: bool,
    /// Enable form data processing
    pub enable_forms: bool,
    /// Directory for uploaded files (if form processing enabled)
    pub upload_dir: Option<PathBuf>,
    /// Enable CORS headers
    pub cors: bool,
    /// Enable directory listing
    pub directory_listing: bool,
    /// SPA fallback (serve index.html for 404s)
    pub spa_fallback: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 3000,
            root_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            hot_reload: true,
            enable_forms: false,
            upload_dir: None,
            cors: true,
            directory_listing: false,
            spa_fallback: true,
        }
    }
}

impl ServerConfig {
    /// Create a new config with default values
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            ..Default::default()
        }
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }

    /// Get the upload directory, defaulting to root_dir/uploads if not set
    pub fn upload_directory(&self) -> PathBuf {
        self.upload_dir
            .clone()
            .unwrap_or_else(|| self.root_dir.join("uploads"))
    }
}
