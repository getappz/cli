//! Virtual filesystem trait for the WordPress-to-Markdown converter.
//!
//! Abstracts file I/O and HTTP downloads so the core library can run
//! both natively (std::fs + reqwest) and inside a WASM plugin (PDK
//! host functions).

use miette::Result;

/// Filesystem and network abstraction for wp2md.
///
/// All paths are plain strings (not `Utf8Path`) to match the
/// ssg-migrator convention and simplify WASM interop.
pub trait Wp2mdVfs {
    /// Read the entire contents of a file as a UTF-8 string.
    fn read_to_string(&self, path: &str) -> Result<String>;

    /// Write a UTF-8 string to a file, creating parent directories as needed.
    fn write_string(&self, path: &str, content: &str) -> Result<()>;

    /// Write raw bytes to a file, creating parent directories as needed.
    fn write_bytes(&self, path: &str, data: &[u8]) -> Result<()>;

    /// Check whether a file or directory exists at the given path.
    fn exists(&self, path: &str) -> bool;

    /// Create a directory and all parent directories.
    fn create_dir_all(&self, path: &str) -> Result<()>;

    /// Download a URL and write its contents directly to a file.
    ///
    /// `strict_ssl` controls whether to verify SSL certificates.
    fn download_to_file(&self, url: &str, dest: &str, strict_ssl: bool) -> Result<()>;
}
