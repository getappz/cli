use crate::services::template_error::TemplateError;
use crate::templates::get_builtin_template;
use serde_json;
use starbase_archive::Archiver;
use starbase_utils::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, instrument};
use ui::progress;

/// Service for downloading and extracting templates from various sources
pub struct TemplateService;

impl TemplateService {
    /// Download a template from GitHub (or other Git platforms) as an archive
    #[instrument(skip_all, fields(url = %url))]
    pub async fn download_github_template(
        url: &str,
        subfolder: Option<&str>,
        branch: Option<&str>,
    ) -> Result<PathBuf, TemplateError> {
        // Parse the GitHub URL
        let (user, repo, platform) = Self::parse_git_url(url)?;

        // Determine which branch to use
        let branches_to_try = if let Some(branch) = branch {
            // User specified a branch, use it directly
            vec![branch.to_string()]
        } else {
            // No branch specified, try to detect it
            let mut branches = Vec::new();

            // Try to detect default branch from GitHub API (only for GitHub)
            if platform == "github.com" {
                match Self::detect_github_default_branch(&user, &repo).await {
                    Ok(detected_branch) => {
                        debug!("Detected default branch: {}", detected_branch);
                        branches.push(detected_branch);
                    }
                    Err(TemplateError::NotFound(_)) => {
                        // Repository doesn't exist, return early with clear error
                        return Err(TemplateError::NotFound(format!(
                            "Repository not found: {}/{}",
                            user, repo
                        )));
                    }
                    Err(_) => {
                        // API call failed (rate limit, network error, etc.), fall back to common names
                        debug!("Failed to detect default branch, using fallback");
                    }
                }
            }

            // Add fallback branches if detection didn't work or for non-GitHub platforms
            if branches.is_empty() {
                branches.push("main".to_string());
                branches.push("master".to_string());
            }

            branches
        };

        // Try each branch until one succeeds
        let mut last_error: Option<TemplateError> = None;
        for ref_name in &branches_to_try {
            match Self::try_download_github_archive(&user, &repo, &platform, ref_name, subfolder)
                .await
            {
                Ok(result) => {
                    return Ok(result);
                }
                Err(e) => {
                    // Check if it's a 404 - might be wrong branch
                    if let TemplateError::DownloadFailed(msg) = &e {
                        if msg.contains("404") {
                            debug!("Branch '{}' not found, trying next", ref_name);
                            last_error = Some(TemplateError::DownloadFailed(format!(
                                "Branch '{}' not found: {}",
                                ref_name, msg
                            )));
                            continue;
                        }
                    }
                    // For other errors (network, timeout, etc.), return immediately
                    return Err(e);
                }
            }
        }

        // All branches failed
        Err(last_error.unwrap_or_else(|| {
            TemplateError::DownloadFailed(format!(
                "Failed to download template from {}/{}: tried branches {:?}",
                user, repo, branches_to_try
            ))
        }))
    }

    /// Attempt to download a GitHub archive for a specific branch
    #[instrument(skip_all, fields(user = %user, repo = %repo, branch = %ref_name))]
    async fn try_download_github_archive(
        user: &str,
        repo: &str,
        platform: &str,
        ref_name: &str,
        subfolder: Option<&str>,
    ) -> Result<PathBuf, TemplateError> {
        let ref_type = if ref_name.starts_with("v")
            || ref_name.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            "tags"
        } else {
            "heads"
        };

        // Construct archive URL
        let archive_url = format!(
            "https://{}/{}/{}/archive/refs/{}/{}.zip",
            platform, user, repo, ref_type, ref_name
        );

        debug!(archive_url = %archive_url, "Downloading GitHub archive");

        // Create temp directory for extraction
        let temp_dir = std::env::temp_dir().join(format!(
            "appz-template-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).map_err(|e| {
            TemplateError::FsError(format!("Failed to create temp directory: {}", e))
        })?;

        let archive_file = temp_dir.join("archive.zip");

        // Download with progress bar
        let pb = progress::progress_bar(0, "Downloading template...");
        let download_result = timeout(
            Duration::from_secs(300), // 5 minute timeout
            Self::download_with_progress(&archive_url, &archive_file, &pb),
        )
        .await;

        match download_result {
            Ok(Ok(_)) => {
                pb.finish_with_message("✓ Download complete");
            }
            Ok(Err(e)) => {
                pb.finish();
                error!(error = %e, branch = %ref_name, "Failed to download archive");
                return Err(TemplateError::DownloadFailed(format!(
                    "Download failed: {}",
                    e
                )));
            }
            Err(_) => {
                pb.finish();
                return Err(TemplateError::DownloadFailed(
                    "Download timeout".to_string(),
                ));
            }
        }

        // Extract archive
        let extracted_dir = temp_dir.join("extracted");
        fs::create_dir_all(&extracted_dir).map_err(|e| {
            TemplateError::FsError(format!("Failed to create extracted directory: {}", e))
        })?;

        Archiver::new(&extracted_dir, &archive_file)
            .unpack_from_ext()
            .map_err(|e| TemplateError::Archive(format!("Failed to extract archive: {:?}", e)))?;

        // Clean up archive file
        let _ = std::fs::remove_file(&archive_file);

        // Find the extracted directory (usually repo-name-branch)
        let extracted_repo_dir = fs::read_dir(&extracted_dir)
            .map_err(|e| {
                TemplateError::FsError(format!("Failed to read extracted directory: {}", e))
            })?
            .into_iter()
            .filter_map(|entry| {
                if entry.path().is_dir() {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .next()
            .ok_or_else(|| {
                TemplateError::ExtractionFailed("No directory found in archive".to_string())
            })?;

        // If subfolder specified, return path to subfolder
        if let Some(subfolder) = subfolder {
            let subfolder_path = extracted_repo_dir.join(subfolder);
            if !subfolder_path.exists() {
                return Err(TemplateError::SubfolderNotFound(format!(
                    "Subfolder '{}' not found in template",
                    subfolder
                )));
            }
            Ok(subfolder_path)
        } else {
            Ok(extracted_repo_dir)
        }
    }

    /// Download a template from npm registry
    #[instrument(skip_all, fields(package = %package))]
    pub async fn download_npm_template(package: &str) -> Result<PathBuf, TemplateError> {
        // Parse package name (support scoped packages)
        let (package_name, mut version) = Self::parse_npm_package(package)?;

        // Resolve "latest" version if needed
        if version == "latest" {
            version = Self::resolve_npm_latest_version(&package_name).await?;
        }

        // Construct tarball URL
        let tarball_url = if let Some(index) = package_name.find('/') {
            // Scoped package: @scope/package
            let pkg_name = &package_name[index + 1..];
            format!(
                "https://registry.npmjs.org/{}/-/{}-{}.tgz",
                package_name, pkg_name, version
            )
        } else {
            // Regular package
            format!(
                "https://registry.npmjs.org/{}/-/{}-{}.tgz",
                package_name, package_name, version
            )
        };

        debug!(tarball_url = %tarball_url, "Downloading npm package");

        // Create temp directory
        let temp_dir = std::env::temp_dir().join(format!(
            "appz-template-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).map_err(|e| {
            TemplateError::FsError(format!("Failed to create temp directory: {}", e))
        })?;

        let tarball_file = temp_dir.join("package.tgz");

        // Download with progress bar
        let pb = progress::progress_bar(0, "Downloading npm package...");
        let download_result = timeout(
            Duration::from_secs(300),
            Self::download_with_progress(&tarball_url, &tarball_file, &pb),
        )
        .await;

        match download_result {
            Ok(Ok(_)) => {
                pb.finish_with_message("✓ Download complete");
            }
            Ok(Err(e)) => {
                pb.finish();
                error!(error = %e, "Failed to download npm package");
                return Err(TemplateError::DownloadFailed(format!(
                    "Download failed: {}",
                    e
                )));
            }
            Err(_) => {
                pb.finish();
                return Err(TemplateError::DownloadFailed(
                    "Download timeout".to_string(),
                ));
            }
        }

        // Extract tarball
        let extracted_dir = temp_dir.join("extracted");
        fs::create_dir_all(&extracted_dir).map_err(|e| {
            TemplateError::FsError(format!("Failed to create extracted directory: {}", e))
        })?;

        Archiver::new(&extracted_dir, &tarball_file)
            .set_prefix("package")
            .unpack_from_ext()
            .map_err(|e| {
                TemplateError::Archive(format!("Failed to extract npm package: {:?}", e))
            })?;

        // Clean up tarball
        let _ = std::fs::remove_file(&tarball_file);

        Ok(extracted_dir)
    }

    /// Copy a local template directory
    #[instrument(skip_all, fields(path = %path.display()))]
    pub async fn copy_local_template(path: &Path) -> Result<PathBuf, TemplateError> {
        if !path.exists() {
            return Err(TemplateError::NotFound(format!(
                "Template path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(TemplateError::InvalidFormat(format!(
                "Template path is not a directory: {}",
                path.display()
            )));
        }

        // Return the path as-is (caller will copy it)
        Ok(path.to_path_buf())
    }

    /// Get built-in template by name
    pub fn get_builtin_template(name: &str) -> Option<(&str, Option<&str>)> {
        get_builtin_template(name)
    }

    /// Detect the default branch for a GitHub repository using the GitHub API
    #[instrument(skip_all, fields(user = %user, repo = %repo))]
    async fn detect_github_default_branch(user: &str, repo: &str) -> Result<String, TemplateError> {
        let api_url = format!("https://api.github.com/repos/{}/{}", user, repo);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("appz-cli")
            .build()
            .map_err(|e| {
                TemplateError::DownloadFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        let response = timeout(Duration::from_secs(10), client.get(&api_url).send())
            .await
            .map_err(|_| {
                TemplateError::DownloadFailed(
                    "Request timeout while detecting default branch".to_string(),
                )
            })?
            .map_err(|e| TemplateError::DownloadFailed(format!("HTTP request failed: {}", e)))?;

        if response.status() == 404 {
            return Err(TemplateError::NotFound(format!(
                "Repository not found: {}/{}",
                user, repo
            )));
        }

        if !response.status().is_success() {
            // If rate limited or other API error, return error but don't fail completely
            // The fallback mechanism will try common branch names
            let status = response.status();
            debug!(
                "GitHub API returned {} for {}/{}, will use fallback",
                status, user, repo
            );
            return Err(TemplateError::DownloadFailed(format!(
                "GitHub API error: {}",
                status
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| {
            TemplateError::DownloadFailed(format!("Failed to parse JSON response: {}", e))
        })?;

        let default_branch = json["default_branch"].as_str().ok_or_else(|| {
            TemplateError::DownloadFailed(format!(
                "No default_branch field in GitHub API response for {}/{}",
                user, repo
            ))
        })?;

        Ok(default_branch.to_string())
    }

    /// Parse GitHub/GitLab/Bitbucket URL
    fn parse_git_url(url: &str) -> Result<(String, String, String), TemplateError> {
        // Remove protocol if present
        let url = url
            .trim_start_matches("https://")
            .trim_start_matches("http://");

        // Detect platform and remove domain prefix
        let (url, platform) = if url.starts_with("gitlab.com") {
            (url.trim_start_matches("gitlab.com"), "gitlab.com")
        } else if url.starts_with("bitbucket.org") {
            (url.trim_start_matches("bitbucket.org"), "bitbucket.org")
        } else if url.starts_with("github.com") {
            (url.trim_start_matches("github.com"), "github.com")
        } else {
            // No domain prefix, assume github.com
            (url, "github.com")
        };

        // Remove leading slashes
        let url = url.trim_start_matches('/');

        // Parse user/repo format
        let parts: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() < 2 {
            return Err(TemplateError::InvalidFormat(format!(
                "Invalid Git URL format: expected user/repo, got: {}",
                url
            )));
        }

        let user = parts[0].to_string();
        let repo = parts[1].to_string();

        Ok((user, repo, platform.to_string()))
    }

    /// Parse npm package string (package@version or just package)
    /// Handles both scoped (@scope/package) and non-scoped packages correctly
    fn parse_npm_package(package: &str) -> Result<(String, String), TemplateError> {
        // Remove npm: prefix if present
        let package = package.trim_start_matches("npm:");

        // Check if this is a scoped package (starts with @)
        if package.starts_with('@') {
            // For scoped packages, the version separator @ must come after the /
            // Example: @babel/core@7.0.0 -> @babel/core and 7.0.0
            // Example: @babel/core -> @babel/core and latest
            if let Some(slash_index) = package.find('/') {
                // Look for @ after the slash (this would be the version separator)
                if let Some(at_index) = package[slash_index..].find('@') {
                    // The actual index is slash_index + at_index
                    let version_index = slash_index + at_index;
                    let name = package[..version_index].to_string();
                    let version = package[version_index + 1..].to_string();
                    Ok((name, version))
                } else {
                    // No version specified, default to latest
                    Ok((package.to_string(), "latest".to_string()))
                }
            } else {
                // Invalid scoped package format (no /), but treat as package name
                Ok((package.to_string(), "latest".to_string()))
            }
        } else {
            // Non-scoped package: any @ can be a version separator
            if let Some(at_index) = package.rfind('@') {
                let name = package[..at_index].to_string();
                let version = package[at_index + 1..].to_string();
                Ok((name, version))
            } else {
                // Default to latest version
                Ok((package.to_string(), "latest".to_string()))
            }
        }
    }

    /// Resolve latest version from npm registry
    async fn resolve_npm_latest_version(package: &str) -> Result<String, TemplateError> {
        let registry_url = format!("https://registry.npmjs.org/{}", package);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                TemplateError::DownloadFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        let response = timeout(Duration::from_secs(30), client.get(&registry_url).send())
            .await
            .map_err(|_| TemplateError::DownloadFailed("Request timeout".to_string()))?
            .map_err(|e| TemplateError::DownloadFailed(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(TemplateError::NotFound(format!(
                "Package not found: {}",
                package
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| {
            TemplateError::DownloadFailed(format!("Failed to parse JSON response: {}", e))
        })?;

        let latest = json["dist-tags"]["latest"].as_str().ok_or_else(|| {
            TemplateError::NotFound(format!("No latest version found for package: {}", package))
        })?;

        Ok(latest.to_string())
    }

    /// Parse template source string to determine type and extract parameters
    pub fn parse_template_source(source: &str) -> Result<TemplateSource, TemplateError> {
        // Check for npm package
        if source.starts_with("npm:") {
            let package = source.trim_start_matches("npm:");
            return Ok(TemplateSource::Npm(package.to_string()));
        }

        // Check for built-in template
        if let Some((repo, subfolder)) = get_builtin_template(source) {
            return Ok(TemplateSource::Builtin {
                repo: repo.to_string(),
                subfolder: subfolder.map(|s| s.to_string()),
            });
        }

        // Check for local path
        if source.starts_with("./")
            || source.starts_with("../")
            || source.starts_with('/')
            || ((source.len() > 1 && source.chars().nth(1) == Some(':'))
                && !source.contains("github.com")
                && !source.contains("gitlab.com")
                && !source.contains("bitbucket.org"))
        {
            // Windows drive letter, but not a Git URL
            return Ok(TemplateSource::Local(source.to_string()));
        }

        // Check for full GitHub/GitLab/Bitbucket URLs and extract user/repo
        let source = if source.starts_with("https://") || source.starts_with("http://") {
            // Parse full URL to extract user/repo part
            let url = source
                .trim_start_matches("https://")
                .trim_start_matches("http://");

            // Find the domain and extract the path after it
            if let Some(idx) = url.find("github.com/") {
                let path = &url[idx + 10..]; // "github.com/".len() = 10
                path.trim_end_matches('/')
            } else if let Some(idx) = url.find("gitlab.com/") {
                let path = &url[idx + 11..]; // "gitlab.com/".len() = 11
                path.trim_end_matches('/')
            } else if let Some(idx) = url.find("bitbucket.org/") {
                let path = &url[idx + 14..]; // "bitbucket.org/".len() = 14
                path.trim_end_matches('/')
            } else {
                // Not a recognized Git hosting URL, treat as-is
                source
            }
        } else {
            source
        };

        // Parse GitHub URL with optional branch/tag and subfolder
        // Format: user/repo#branch/path/to/subfolder or user/repo@tag/path/to/subfolder
        let (base_url, branch, subfolder) = if let Some(hash_idx) = source.find('#') {
            let base = &source[..hash_idx];
            let rest = &source[hash_idx + 1..];
            if let Some(slash_idx) = rest.find('/') {
                (base, Some(&rest[..slash_idx]), Some(&rest[slash_idx + 1..]))
            } else {
                (base, Some(rest), None)
            }
        } else if let Some(at_idx) = source.find('@') {
            let base = &source[..at_idx];
            let rest = &source[at_idx + 1..];
            if let Some(slash_idx) = rest.find('/') {
                (base, Some(&rest[..slash_idx]), Some(&rest[slash_idx + 1..]))
            } else {
                (base, Some(rest), None)
            }
        } else if let Some(_slash_idx) = source.find('/') {
            // Check if it's user/repo/subfolder format
            let parts: Vec<&str> = source.split('/').filter(|s| !s.is_empty()).collect();
            if parts.len() > 2 {
                // user/repo/path/to/subfolder
                let base = format!("{}/{}", parts[0], parts[1]);
                let subfolder = parts[2..].join("/");
                return Ok(TemplateSource::GitHub {
                    url: base,
                    branch: None,
                    subfolder: Some(subfolder),
                });
            } else {
                (source, None, None)
            }
        } else {
            (source, None, None)
        };

        Ok(TemplateSource::GitHub {
            url: base_url.to_string(),
            branch: branch.map(|s| s.to_string()),
            subfolder: subfolder.map(|s| s.to_string()),
        })
    }

    /// Download a file with progress bar
    async fn download_with_progress(
        url: &str,
        file_path: &Path,
        pb: &progress::ProgressBarHandle,
    ) -> Result<(), TemplateError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| {
                TemplateError::DownloadFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        let mut response =
            client.get(url).send().await.map_err(|e| {
                TemplateError::DownloadFailed(format!("HTTP request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_msg = if status == 404 {
                format!(
                    "HTTP error: {} Not Found - The repository or branch may not exist",
                    status
                )
            } else {
                format!("HTTP error: {}", status)
            };
            return Err(TemplateError::DownloadFailed(error_msg));
        }

        // Get content length for progress bar
        let total_size = response.content_length().unwrap_or(0);

        if total_size > 0 {
            pb.set_length(total_size);
        }

        // Create file and write with progress updates
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create(file_path)
            .map_err(|e| TemplateError::FsError(format!("Failed to create file: {}", e)))?;

        let mut downloaded: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| TemplateError::DownloadFailed(format!("Failed to read chunk: {}", e)))?
        {
            file.write_all(&chunk)
                .map_err(|e| TemplateError::FsError(format!("Failed to write chunk: {}", e)))?;

            downloaded += chunk.len() as u64;
            if total_size > 0 {
                pb.set_position(downloaded);
            } else {
                pb.inc_by(chunk.len() as u64);
            }
        }

        file.flush()
            .map_err(|e| TemplateError::FsError(format!("Failed to flush file: {}", e)))?;

        Ok(())
    }
}

/// Represents a parsed template source
#[derive(Debug, Clone)]
pub enum TemplateSource {
    GitHub {
        url: String,
        branch: Option<String>,
        subfolder: Option<String>,
    },
    Npm(String),
    Local(String),
    Builtin {
        repo: String,
        subfolder: Option<String>,
    },
}
