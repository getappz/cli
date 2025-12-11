//! Tunnel service for exposing local development servers to the internet

use crate::shell::command_exists;
use miette::Result;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

/// Trait for tunnel services
#[async_trait::async_trait]
pub trait TunnelService: Send + Sync {
    /// Start the tunnel pointing to the local port
    async fn start(&mut self, port: u16) -> Result<String>;

    /// Stop the tunnel
    async fn stop(&mut self) -> Result<()>;

    /// Get the public URL (if available)
    fn public_url(&self) -> Option<&str>;
}

/// Cloudflared tunnel implementation
pub struct CloudflaredTunnel {
    process: Option<Child>,
    public_url: Option<String>,
    binary_path: Option<PathBuf>,
}

impl CloudflaredTunnel {
    /// Create a new CloudflaredTunnel instance
    pub fn new() -> Self {
        Self {
            process: None,
            public_url: None,
            binary_path: None,
        }
    }

    /// Ensure cloudflared is installed, installing if necessary
    async fn ensure_installed(&mut self) -> Result<PathBuf> {
        // Check if cloudflared exists in PATH
        if command_exists("cloudflared") {
            if let Ok(path) = which::which("cloudflared") {
                debug!("Found cloudflared in PATH: {}", path.display());
                return Ok(path);
            }
        }

        // Check if we already have it cached
        if let Some(cached_path) = self.get_cached_binary_path() {
            if cached_path.exists() {
                debug!("Using cached cloudflared: {}", cached_path.display());
                return Ok(cached_path);
            }
        }

        // Need to download and install
        info!("cloudflared not found, downloading...");
        self.download_and_install().await
    }

    /// Get the path where cloudflared binary should be cached
    fn get_cached_binary_path(&self) -> Option<PathBuf> {
        use starbase_utils::dirs;

        let cache_dir = dirs::home_dir()?.join(".appz").join("cache");
        let binary_name = if cfg!(target_os = "windows") {
            "cloudflared.exe"
        } else {
            "cloudflared"
        };
        Some(cache_dir.join(binary_name))
    }

    /// Download and install cloudflared
    async fn download_and_install(&mut self) -> Result<PathBuf> {
        use crate::http::HTTP;
        use crate::utils::fs;
        use starbase_utils::dirs;
        use ui::progress;

        let cache_dir = dirs::home_dir()
            .ok_or_else(|| miette::miette!("Could not determine home directory"))?
            .join(".appz")
            .join("cache");

        // Create cache directory if it doesn't exist
        starbase_utils::fs::create_dir_all(&cache_dir)
            .map_err(|e| miette::miette!("Failed to create cache directory: {}", e))?;

        // Determine OS and architecture
        let (os, arch, ext) = if cfg!(target_os = "windows") {
            ("windows", "amd64", ".exe")
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                ("darwin", "arm64", "")
            } else {
                ("darwin", "amd64", "")
            }
        } else {
            ("linux", "amd64", "")
        };

        let binary_name = format!("cloudflared{}", ext);
        let download_url = format!(
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-{}-{}",
            os, arch
        );
        let download_url = if cfg!(target_os = "windows") {
            format!("{}.exe", download_url)
        } else {
            download_url
        };

        let binary_path = cache_dir.join(&binary_name);

        info!("Downloading cloudflared from: {}", download_url);

        // Create progress bar (using existing ui::progress module)
        let pb = progress::progress_bar(0, "Downloading cloudflared");

        // Download using our HTTP client with progress reporting
        HTTP.download_file(&download_url, &binary_path, Some(&pb))
            .await
            .map_err(|e| miette::miette!("Failed to download cloudflared: {}", e))?;

        // Make executable on Unix
        fs::make_executable(&binary_path)?;

        info!("✓ cloudflared installed to: {}", binary_path.display());
        self.binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    /// Extract public URL from cloudflared output
    fn extract_url_from_output(&self, line: &str) -> Option<String> {
        // Cloudflared outputs URLs like: "https://xxxx-xxxx-xxxx.trycloudflare.com"
        if let Some(start) = line.find("https://") {
            if let Some(end) =
                line[start..].find(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
            {
                let url = line[start..start + end].trim();
                if url.contains("trycloudflare.com") {
                    return Some(url.to_string());
                }
            } else {
                // URL might be at end of line
                let url = line[start..].trim();
                if url.contains("trycloudflare.com") {
                    return Some(url.to_string());
                }
            }
        }
        None
    }
}

#[async_trait::async_trait]
impl TunnelService for CloudflaredTunnel {
    async fn start(&mut self, port: u16) -> Result<String> {
        // Ensure cloudflared is installed
        let binary_path = self.ensure_installed().await?;

        info!("Starting cloudflared tunnel on port {}", port);

        // Spawn cloudflared process
        // Use "tunnel --url" for quick tunnels (no authentication required)
        // This creates a temporary tunnel and outputs the public URL
        let mut cmd = Command::new(&binary_path);
        cmd.args(["tunnel", "--url", &format!("http://localhost:{}", port)]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| miette::miette!("Failed to spawn cloudflared: {}", e))?;

        // Read both stdout and stderr to extract the public URL
        // Cloudflared may output the URL to either stream
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| miette::miette!("Failed to capture cloudflared stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| miette::miette!("Failed to capture cloudflared stderr"))?;

        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);
        let (url_sender, receiver) = oneshot::channel();

        // Store the process handle before moving it
        let process_handle = child;

        // Spawn a task to read from both stdout and stderr concurrently
        // and extract URL from whichever stream contains it first
        tokio::spawn(async move {
            let mut stdout_line = String::new();
            let mut stderr_line = String::new();
            let mut stdout_done = false;
            let mut stderr_done = false;

            loop {
                tokio::select! {
                    result = stdout_reader.read_line(&mut stdout_line), if !stdout_done => {
                        match result {
                            Ok(0) => stdout_done = true, // EOF
                            Ok(_) => {
                                debug!("cloudflared stdout: {}", stdout_line.trim());

                                // Try to extract URL from this line
                                if let Some(url) = Self::extract_url_from_output_static(&stdout_line) {
                                    let _ = url_sender.send(url);
                                    return;
                                }

                                stdout_line.clear();
                            }
                            Err(e) => {
                                error!("Error reading cloudflared stdout: {}", e);
                                stdout_done = true;
                            }
                        }
                    }
                    result = stderr_reader.read_line(&mut stderr_line), if !stderr_done => {
                        match result {
                            Ok(0) => stderr_done = true, // EOF
                            Ok(_) => {
                                debug!("cloudflared stderr: {}", stderr_line.trim());

                                // Try to extract URL from this line
                                if let Some(url) = Self::extract_url_from_output_static(&stderr_line) {
                                    let _ = url_sender.send(url);
                                    return;
                                }

                                stderr_line.clear();
                            }
                            Err(e) => {
                                error!("Error reading cloudflared stderr: {}", e);
                                stderr_done = true;
                            }
                        }
                    }
                }

                // If both streams are done, exit
                if stdout_done && stderr_done {
                    break;
                }
            }
        });

        self.process = Some(process_handle);

        // Wait for URL with timeout
        let timeout = tokio::time::Duration::from_secs(30);
        let url = tokio::time::timeout(timeout, receiver)
            .await
            .map_err(|_| miette::miette!("Timeout waiting for cloudflared URL"))?
            .map_err(|_| miette::miette!("Failed to get public URL from cloudflared"))?;

        self.public_url = Some(url.clone());
        info!("✓ Tunnel created: {}", url);
        Ok(url)
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            debug!("Stopping cloudflared tunnel");
            if let Err(e) = process.kill().await {
                warn!("Failed to kill cloudflared process: {}", e);
            }
            let _ = process.wait().await;
        }
        self.public_url = None;
        Ok(())
    }

    fn public_url(&self) -> Option<&str> {
        self.public_url.as_deref()
    }
}

impl CloudflaredTunnel {
    fn extract_url_from_output_static(line: &str) -> Option<String> {
        // Cloudflared outputs URLs in various formats:
        // - "https://xxxx-xxxx-xxxx.trycloudflare.com"
        // - "Visit it at: https://xxxx-xxxx-xxxx.trycloudflare.com"
        // - In boxed format with pipes

        // Look for https:// followed by trycloudflare.com
        if let Some(start) = line.find("https://") {
            let remaining = &line[start..];
            // Find the end of the URL (whitespace, newline, or end of string)
            let end = remaining
                .find(|c: char| c.is_whitespace() || c == '\n' || c == '\r' || c == '|')
                .unwrap_or(remaining.len());

            let url = remaining[..end].trim();
            if url.contains("trycloudflare.com") {
                // Extract just the URL part
                if let Some(url_end) =
                    url.find(|c: char| c.is_whitespace() || c == '\n' || c == '\r' || c == '|')
                {
                    return Some(url[..url_end].to_string());
                }
                return Some(url.to_string());
            }
        }
        None
    }
}

impl Drop for CloudflaredTunnel {
    fn drop(&mut self) {
        if self.process.is_some() {
            // Try to stop the process, but don't block
            let mut process = self.process.take().unwrap();
            tokio::spawn(async move {
                let _ = process.kill().await;
            });
        }
    }
}
