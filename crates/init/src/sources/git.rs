//! Git provider source: download from GitHub, GitLab, Bitbucket archives.

use std::path::PathBuf;
use std::time::Duration;

use starbase_archive::Archiver;
use starbase_utils::fs;
use tokio::time::timeout;
use tracing::debug;

use super::download::download_with_progress;
use crate::error::{InitError, InitResult};

/// Parsed git source with platform-specific info.
#[derive(Debug)]
pub struct GitSource {
    pub user: String,
    pub repo: String,
    pub platform: String,
    pub branch: Option<String>,
    pub subfolder: Option<String>,
}

/// Parse a git URL (user/repo or full URL) into platform, user, repo, branch, subfolder.
pub fn parse_git_source(source: &str) -> InitResult<GitSource> {
    let source = source
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let (path, platform) = if source.contains("gitlab.com/") {
        let idx = source.find("gitlab.com/").unwrap();
        (source[idx + 11..].trim_start_matches('/'), "gitlab.com")
    } else if source.contains("bitbucket.org/") {
        let idx = source.find("bitbucket.org/").unwrap();
        (source[idx + 14..].trim_start_matches('/'), "bitbucket.org")
    } else if source.contains("github.com/") {
        let idx = source.find("github.com/").unwrap();
        (source[idx + 10..].trim_start_matches('/'), "github.com")
    } else {
        (source.trim_start_matches('/'), "github.com")
    };

    let (base, branch, subfolder) = parse_branch_subfolder(path)?;
    let parts: Vec<&str> = base.as_str().split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return Err(InitError::InvalidFormat(format!(
            "Invalid Git URL: expected user/repo, got: {}",
            source
        )));
    }

    let user = parts[0].to_string();
    let repo = parts[1].trim_end_matches(".git").to_string();

    Ok(GitSource {
        user,
        repo,
        platform: platform.to_string(),
        branch,
        subfolder,
    })
}

fn parse_branch_subfolder(path: &str) -> InitResult<(String, Option<String>, Option<String>)> {
    if let Some(hash_idx) = path.find('#') {
        let base = path[..hash_idx].to_string();
        let rest = &path[hash_idx + 1..];
        if let Some(slash_idx) = rest.find('/') {
            Ok((
                base,
                Some(rest[..slash_idx].to_string()),
                Some(rest[slash_idx + 1..].to_string()),
            ))
        } else {
            Ok((base, Some(rest.to_string()), None))
        }
    } else if let Some(at_idx) = path.find('@') {
        let base = path[..at_idx].to_string();
        let rest = &path[at_idx + 1..];
        if let Some(slash_idx) = rest.find('/') {
            Ok((
                base,
                Some(rest[..slash_idx].to_string()),
                Some(rest[slash_idx + 1..].to_string()),
            ))
        } else {
            Ok((base, Some(rest.to_string()), None))
        }
    } else {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 4 && parts[2] == "tree" {
            // GitHub /tree/BRANCH/PATH (e.g. user/repo/tree/main/skills/dwsy/project-planner)
            let base = format!("{}/{}", parts[0], parts[1]);
            let branch = Some(parts[3].to_string());
            let subfolder = if parts.len() > 4 {
                Some(parts[4..].join("/"))
            } else {
                None
            };
            Ok((base, branch, subfolder))
        } else if parts.len() > 2 {
            let base = format!("{}/{}", parts[0], parts[1]);
            let subfolder = parts[2..].join("/");
            Ok((base, None, Some(subfolder)))
        } else {
            Ok((path.to_string(), None, None))
        }
    }
}

/// Build archive URL for the given platform.
fn archive_url(
    user: &str,
    repo: &str,
    platform: &str,
    ref_name: &str,
    format: &str,
) -> String {
    let ref_type = if ref_name.starts_with('v')
        || ref_name.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        "tags"
    } else {
        "heads"
    };

    match platform {
        "github.com" => format!(
            "https://github.com/{}/{}/archive/refs/{}/{}.{}",
            user, repo, ref_type, ref_name, format
        ),
        "gitlab.com" => format!(
            "https://gitlab.com/{}/{}/-/archive/{}/{}-{}.{}",
            user, repo, ref_name, repo, ref_name, format
        ),
        "bitbucket.org" => format!(
            "https://bitbucket.org/{}/{}/get/{}.{}",
            user, repo, ref_name, format
        ),
        _ => format!(
            "https://github.com/{}/{}/archive/refs/{}/{}.{}",
            user, repo, ref_type, ref_name, format
        ),
    }
}

/// Detect default branch from GitHub API (only for GitHub).
async fn detect_github_default_branch(user: &str, repo: &str) -> InitResult<String> {
    let api_url = format!("https://api.github.com/repos/{}/{}", user, repo);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("appz-cli")
        .build()
        .map_err(|e| InitError::DownloadFailed(format!("Failed to create HTTP client: {}", e)))?;

    let response = timeout(Duration::from_secs(10), client.get(&api_url).send())
        .await
        .map_err(|_| {
            InitError::DownloadFailed("Request timeout while detecting default branch".to_string())
        })?
        .map_err(|e| InitError::DownloadFailed(format!("HTTP request failed: {}", e)))?;

    if response.status() == 404 {
        return Err(InitError::NotFound(format!(
            "Repository not found: {}/{}",
            user, repo
        )));
    }

    if !response.status().is_success() {
        return Err(InitError::DownloadFailed(format!(
            "GitHub API error: {}",
            response.status()
        )));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| {
        InitError::DownloadFailed(format!("Failed to parse JSON response: {}", e))
    })?;

    let default_branch = json["default_branch"]
        .as_str()
        .ok_or_else(|| {
            InitError::DownloadFailed(format!(
                "No default_branch in GitHub API response for {}/{}",
                user, repo
            ))
        })?
        .to_string();

    Ok(default_branch)
}

/// Download and extract a git repository.
/// `progress_label` is used for the download progress bar (e.g. "Downloading template..." or "Downloading skill...").
pub async fn download_git(
    source: &str,
    subfolder: Option<&str>,
    branch: Option<&str>,
    quiet: bool,
    progress_label: Option<&str>,
) -> InitResult<PathBuf> {
    let parsed = parse_git_source(source)?;
    let subfolder = subfolder.or(parsed.subfolder.as_deref());
    let branch = branch.or(parsed.branch.as_deref());

    let branches_to_try: Vec<String> = if let Some(b) = branch {
        vec![b.to_string()]
    } else if parsed.platform == "github.com" {
        match detect_github_default_branch(&parsed.user, &parsed.repo).await {
            Ok(b) => vec![b],
            Err(InitError::NotFound(_)) => return Err(InitError::NotFound(format!(
                "Repository not found: {}/{}",
                parsed.user, parsed.repo
            ))),
            Err(_) => vec!["main".to_string(), "master".to_string()],
        }
    } else {
        vec!["main".to_string(), "master".to_string()]
    };

    let label = progress_label.unwrap_or("Downloading template...");
    let mut last_error: Option<InitError> = None;
    for ref_name in &branches_to_try {
        match try_download_archive(
            &parsed.user,
            &parsed.repo,
            &parsed.platform,
            ref_name,
            subfolder,
            quiet,
            label,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(e) => {
                if let InitError::DownloadFailed(msg) = &e {
                    if msg.contains("404") {
                        debug!("Branch '{}' not found, trying next", ref_name);
                        last_error = Some(e);
                        continue;
                    }
                }
                return Err(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| InitError::DownloadFailed(format!(
        "Failed to download from {}/{}: tried branches {:?}",
        parsed.user, parsed.repo, branches_to_try
    ))))
}

async fn try_download_archive(
    user: &str,
    repo: &str,
    platform: &str,
    ref_name: &str,
    subfolder: Option<&str>,
    quiet: bool,
    progress_label: &str,
) -> InitResult<PathBuf> {
    let format = if platform == "bitbucket.org" {
        "zip" // Bitbucket uses zip
    } else {
        "zip"
    };

    let archive_url = archive_url(user, repo, platform, ref_name, format);
    debug!(url = %archive_url, "Downloading git archive");

    let temp_dir = std::env::temp_dir().join(format!(
        "appz-init-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| InitError::FsError(format!("Failed to create temp dir: {}", e)))?;

    let archive_file = temp_dir.join("archive.zip");
    download_with_progress(&archive_url, &archive_file, progress_label, quiet).await?;

    let extracted_dir = temp_dir.join("extracted");
    fs::create_dir_all(&extracted_dir)
        .map_err(|e| InitError::FsError(format!("Failed to create extracted dir: {}", e)))?;

    Archiver::new(&extracted_dir, &archive_file)
        .unpack_from_ext()
        .map_err(|e| InitError::Archive(format!("Failed to extract: {:?}", e)))?;

    let _ = starbase_utils::fs::remove_file(&archive_file);

    // Find extracted dir (repo-branch for GitHub, project-ref for GitLab, etc.)
    let extracted_repo_dir = fs::read_dir(&extracted_dir)
        .map_err(|e| InitError::FsError(format!("Failed to read extracted dir: {}", e)))?
        .into_iter()
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| InitError::ExtractionFailed("No directory in archive".to_string()))?;

    let result = if let Some(sf) = subfolder {
        let p = extracted_repo_dir.join(sf);
        if !p.exists() {
            return Err(InitError::NotFound(format!("Subfolder '{}' not found", sf)));
        }
        p
    } else {
        extracted_repo_dir
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::parse_git_source;

    #[test]
    fn parse_git_source_strips_dot_git_from_repo() {
        // source_parser produces URLs with .git suffix; parse_git_source must strip it
        // so the GitHub API receives "owner/repo" not "owner/repo.git"
        let parsed = parse_git_source("https://github.com/astrolicious/agent-skills.git").unwrap();
        assert_eq!(parsed.user, "astrolicious");
        assert_eq!(parsed.repo, "agent-skills");
        assert_eq!(parsed.platform, "github.com");
    }

    #[test]
    fn parse_git_source_owner_repo_without_git() {
        let parsed = parse_git_source("vercel-labs/skills").unwrap();
        assert_eq!(parsed.user, "vercel-labs");
        assert_eq!(parsed.repo, "skills");
    }
}
