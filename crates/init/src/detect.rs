//! Source detection: route `appz init <arg>` to the correct provider.

use crate::error::{InitError, InitResult};
use crate::provider::{create_provider_registry, InitProvider};
use crate::providers::framework::has_create_command;

/// Resolved source type with provider and parsed source.
pub struct ResolvedSource {
    /// The provider to use.
    pub provider: Box<dyn InitProvider>,

    /// The source string as passed by the user (possibly normalized).
    pub source: String,
}

/// Detect which provider to use based on the source string.
///
/// Priority:
/// 1. npm: prefix → NpmProvider
/// 2. Framework slug (astro, nextjs, vite, etc.) → FrameworkProvider
/// 3. http(s) URL ending in .zip/.tar.gz/.tar.xz/.tar.zstd/.tgz → RemoteArchiveProvider
/// 4. http(s) URL or user/repo (github, gitlab, bitbucket) → GitProvider
/// 5. Local path (./, ../, /, or path with :) → LocalProvider
pub fn resolve_source(source: &str) -> InitResult<ResolvedSource> {
    let source = source.trim();
    if source.is_empty() {
        return Err(InitError::SourceNotFound("empty".to_string()));
    }

    // npm: prefix
    if source.starts_with("npm:") {
        let provider = create_provider_registry()
            .into_iter()
            .find(|p| p.slug() == "npm")
            .ok_or_else(|| InitError::SourceNotFound("npm".to_string()))?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // Remote archive URL (must check before generic http)
    if (source.starts_with("https://") || source.starts_with("http://"))
        && is_archive_url(source)
    {
        let provider = create_provider_registry()
            .into_iter()
            .find(|p| p.slug() == "remote-archive")
            .ok_or_else(|| InitError::SourceNotFound("remote-archive".to_string()))?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // Git URL or user/repo
    if is_git_source(source) {
        let provider = create_provider_registry()
            .into_iter()
            .find(|p| p.slug() == "git")
            .ok_or_else(|| InitError::SourceNotFound("git".to_string()))?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // Local path
    if is_local_path(source) {
        let provider = create_provider_registry()
            .into_iter()
            .find(|p| p.slug() == "local")
            .ok_or_else(|| InitError::SourceNotFound("local".to_string()))?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // Framework slug (no slashes, no dots, alphanumeric + maybe hyphens)
    if is_framework_slug(source) {
        let provider = create_provider_registry()
            .into_iter()
            .find(|p| p.slug() == "framework")
            .ok_or_else(|| InitError::SourceNotFound("framework".to_string()))?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    Err(InitError::SourceNotFound(source.to_string()))
}

fn is_archive_url(s: &str) -> bool {
    let s = s.to_lowercase();
    s.ends_with(".zip")
        || s.ends_with(".tar.gz")
        || s.ends_with(".tgz")
        || s.ends_with(".tar.xz")
        || s.ends_with(".tar.zstd")
}

fn is_git_source(s: &str) -> bool {
    if s.starts_with("https://") || s.starts_with("http://") {
        let lower = s.to_lowercase();
        return lower.contains("github.com")
            || lower.contains("gitlab.com")
            || lower.contains("bitbucket.org");
    }
    // user/repo format (at least one slash, no leading dot, no drive letter)
    if s.contains('/') && !s.starts_with("./") && !s.starts_with("../") {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return true;
        }
    }
    false
}

fn is_local_path(s: &str) -> bool {
    s.starts_with("./") || s.starts_with("../") || s.starts_with('/') || {
        // Windows drive letter
        s.len() >= 2 && s.chars().nth(1) == Some(':') && !s.contains("github.com") && !s.contains("gitlab.com") && !s.contains("bitbucket.org")
    }
}

fn is_framework_slug(s: &str) -> bool {
    // Simple slug: alphanumeric, hyphens, no slashes
    !s.is_empty()
        && !s.contains('/')
        && !s.contains('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        && has_create_command(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_archive_url() {
        assert!(is_archive_url("https://example.com/foo.zip"));
        assert!(is_archive_url("https://example.com/foo.tar.gz"));
        assert!(is_archive_url("https://example.com/foo.tgz"));
        assert!(!is_archive_url("https://example.com/foo"));
        assert!(!is_archive_url("https://github.com/user/repo"));
    }

    #[test]
    fn test_is_git_source() {
        assert!(is_git_source("https://github.com/user/repo"));
        assert!(is_git_source("https://gitlab.com/user/repo"));
        assert!(is_git_source("user/repo"));
        assert!(!is_git_source("./local"));
        assert!(!is_git_source("astro"));
    }

    #[test]
    fn test_is_local_path() {
        assert!(is_local_path("./foo"));
        assert!(is_local_path("../foo"));
        assert!(is_local_path("/absolute"));
        assert!(!is_local_path("user/repo"));
    }

    #[test]
    fn test_resolve_npm() {
        let r = resolve_source("npm:create-foo").unwrap();
        assert_eq!(r.provider.slug(), "npm");
        assert_eq!(r.source, "npm:create-foo");
    }

    #[test]
    fn test_resolve_framework() {
        let r = resolve_source("astro").unwrap();
        assert_eq!(r.provider.slug(), "framework");
        assert_eq!(r.source, "astro");
    }
}
