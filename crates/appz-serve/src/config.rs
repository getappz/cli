use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub root_dir: PathBuf,
    pub hot_reload: bool,
    pub spa_fallback: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 4321,
            root_dir: PathBuf::from("."),
            hot_reload: false,
            spa_fallback: false,
        }
    }
}

impl ServerConfig {
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// Find an available port starting from `preferred`, trying up to 20 candidates.
pub fn find_available_port(preferred: u16) -> u16 {
    for offset in 0..20u16 {
        let port = preferred.saturating_add(offset);
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    preferred
}