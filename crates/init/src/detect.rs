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

/// Check whether a slug is a known framework or CMS.
///
/// Returns true if the slug matches any of:
/// - a framework with a create command (astro, nextjs, vite, etc.)
/// - the string "wordpress" (case-insensitive)
/// - a framework known to the frameworks registry
pub fn is_known_framework(slug: &str) -> bool {
    if slug.eq_ignore_ascii_case("wordpress") {
        return true;
    }
    if has_create_command(slug) {
        return true;
    }
    frameworks::find_by_slug(slug).is_some()
}

/// Split "framework/blueprint" into ("framework", "blueprint") when the first
/// segment is a known framework slug.  Returns None for any other pattern.
pub fn parse_framework_blueprint(source: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = source.splitn(2, '/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        if is_known_framework(parts[0]) {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}

/// Detect which provider to use based on the source string.
///
/// Priority:
/// 1. `npm:` prefix → NpmProvider
/// 2. `git:` prefix → GitProvider (escape hatch; prefix stripped)
/// 3. Known framework slug + `/name`  (e.g. `nextjs/ecommerce`) → BlueprintProvider
/// 4. Known framework slug alone      (e.g. `nextjs`, `wordpress`) → BlueprintProvider
/// 5. Archive URL (http/https + .zip/.tar.gz/etc.) → RemoteArchiveProvider
/// 6. Git URL or `user/repo` (first segment NOT a known framework) → GitProvider
/// 7. Local path (`./`, `../`, `/`, Windows drive) → LocalProvider
pub fn resolve_source(source: &str) -> InitResult<ResolvedSource> {
    let source = source.trim();
    if source.is_empty() {
        return Err(InitError::SourceNotFound("empty".to_string()));
    }

    // 1. npm: prefix
    if source.starts_with("npm:") {
        let provider = find_provider("npm")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // 2. git: escape hatch — strip prefix and treat remainder as a git source
    if let Some(rest) = source.strip_prefix("git:") {
        let provider = find_provider("git")?;
        return Ok(ResolvedSource {
            provider,
            source: rest.to_string(),
        });
    }

    // 3. Known framework slug + "/blueprint-name"
    if let Some(_parts) = parse_framework_blueprint(source) {
        let provider = find_provider("blueprint")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // 4. Known framework slug alone (no slashes)
    if !source.contains('/') && is_known_framework(source) {
        let provider = find_provider("blueprint")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // 5. Remote archive URL
    if (source.starts_with("https://") || source.starts_with("http://")) && is_archive_url(source)
    {
        let provider = find_provider("remote-archive")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // 6. Git URL or user/repo (first segment is NOT a known framework)
    if is_git_source(source) {
        let provider = find_provider("git")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    // 7. Local path
    if is_local_path(source) {
        let provider = find_provider("local")?;
        return Ok(ResolvedSource {
            provider,
            source: source.to_string(),
        });
    }

    Err(InitError::SourceNotFound(source.to_string()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_provider(slug: &str) -> InitResult<Box<dyn InitProvider>> {
    create_provider_registry()
        .into_iter()
        .find(|p| p.slug() == slug)
        .ok_or_else(|| InitError::SourceNotFound(slug.to_string()))
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
    // user/repo format: at least one slash, no leading dot, no drive letter,
    // and first segment must NOT be a known framework (those go to blueprint).
    if s.contains('/') && !s.starts_with("./") && !s.starts_with("../") {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            // If first segment is a known framework, it belongs to BlueprintProvider
            // (already handled above in resolve_source), so skip here.
            return !is_known_framework(parts[0]);
        }
    }
    false
}

fn is_local_path(s: &str) -> bool {
    s.starts_with("./")
        || s.starts_with("../")
        || s.starts_with('/')
        || {
            // Windows drive letter (e.g. C:\path or C:/path)
            s.len() >= 2
                && s.chars().nth(1) == Some(':')
                && !s.contains("github.com")
                && !s.contains("gitlab.com")
                && !s.contains("bitbucket.org")
        }
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
        // framework/name must NOT be treated as git
        assert!(!is_git_source("nextjs/ecommerce"));
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
        assert_eq!(r.provider.slug(), "blueprint");
        assert_eq!(r.source, "astro");
    }

    #[test]
    fn test_resolve_wordpress() {
        let r = resolve_source("wordpress").unwrap();
        assert_eq!(r.provider.slug(), "blueprint");
        assert_eq!(r.source, "wordpress");
    }
}
