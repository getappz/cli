use async_trait::async_trait;

/// Filesystem abstraction for framework detection
/// Similar to DetectorFilesystem in Vercel's TypeScript implementation
#[async_trait]
pub trait DetectorFilesystem: Send + Sync {
    /// Check if a path exists
    async fn has_path(&self, path: &str) -> bool;

    /// Check if a path is a file
    async fn is_file(&self, path: &str) -> bool;

    /// Read file contents as string
    async fn read_file(&self, path: &str) -> Result<String, std::io::Error>;
}

/// Standard filesystem implementation using std::fs
pub struct StdFilesystem {
    base_path: Option<std::path::PathBuf>,
}

impl StdFilesystem {
    /// Create a new filesystem detector with optional base path
    pub fn new(base_path: Option<impl Into<std::path::PathBuf>>) -> Self {
        Self {
            base_path: base_path.map(Into::into),
        }
    }

    fn resolve_path(&self, path: &str) -> std::path::PathBuf {
        if let Some(base) = &self.base_path {
            base.join(path)
        } else {
            path.into()
        }
    }
}

#[async_trait]
impl DetectorFilesystem for StdFilesystem {
    async fn has_path(&self, path: &str) -> bool {
        let resolved = self.resolve_path(path);
        tokio::task::spawn_blocking(move || resolved.exists())
            .await
            .unwrap_or(false)
    }

    async fn is_file(&self, path: &str) -> bool {
        let resolved = self.resolve_path(path);
        tokio::task::spawn_blocking(move || resolved.is_file())
            .await
            .unwrap_or(false)
    }

    async fn read_file(&self, path: &str) -> Result<String, std::io::Error> {
        let resolved = self.resolve_path(path);
        // Use tokio::task::spawn_blocking for file I/O
        tokio::task::spawn_blocking(move || std::fs::read_to_string(resolved))
            .await
            .map_err(|e| std::io::Error::other(format!("Task join error: {}", e)))?
    }
}
