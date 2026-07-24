use std::path::PathBuf;

/// A git hosting platform `appz init` knows how to resolve a remote source
/// against — used both for `git clone` (any host git itself understands)
/// and for building a template-download archive URL (host-specific format).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    GitHub,
    GitLab,
    Bitbucket,
}

impl Host {
    pub fn domain(self) -> &'static str {
        match self {
            Host::GitHub => "github.com",
            Host::GitLab => "gitlab.com",
            Host::Bitbucket => "bitbucket.org",
        }
    }

    fn from_domain(s: &str) -> Option<Host> {
        match s.to_ascii_lowercase().as_str() {
            "github.com" => Some(Host::GitHub),
            "gitlab.com" => Some(Host::GitLab),
            "bitbucket.org" => Some(Host::Bitbucket),
            _ => None,
        }
    }
}

/// A remote source parsed from a git URL: which host, whose repo, and the
/// original URL string (passed to `git clone` as-is).
#[derive(Debug, Clone)]
pub struct RemoteSource {
    pub host: Host,
    pub owner: String,
    pub repo: String,
    pub url: String,
}

/// What `appz init <source>` resolved `source` to.
pub enum InitSource {
    Local(PathBuf),
    Remote(RemoteSource),
}

/// Classify an `appz init` source argument. Only a string that parses as a
/// supported git URL is `Remote` — everything else (including malformed or
/// nonexistent local paths) is `Local` and handled exactly as today, letting
/// the existing path-canonicalization error surface for typos.
pub fn classify(source: &str) -> InitSource {
    match parse_remote(source) {
        Some(r) => InitSource::Remote(r),
        None => InitSource::Local(PathBuf::from(source)),
    }
}

/// Parse `source` as `https://<host>/<owner>/<repo>(.git)?`,
/// `http://<host>/...`, or `git@<host>:<owner>/<repo>(.git)?` for
/// `github.com` / `gitlab.com` / `bitbucket.org`. Returns `None` for
/// anything else (bare `owner/repo` shorthand, unknown hosts, local paths).
pub fn parse_remote(source: &str) -> Option<RemoteSource> {
    let trimmed = source.trim();

    if let Some(rest) = trimmed.strip_prefix("git@") {
        let (host_part, path) = rest.split_once(':')?;
        let host = Host::from_domain(host_part)?;
        let (owner, repo) = split_owner_repo(path)?;
        return Some(RemoteSource {
            host,
            owner,
            repo,
            url: trimmed.to_string(),
        });
    }

    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let (host_part, path) = rest.split_once('/')?;
    let host = Host::from_domain(host_part)?;
    let (owner, repo) = split_owner_repo(path)?;
    Some(RemoteSource {
        host,
        owner,
        repo,
        url: trimmed.to_string(),
    })
}

fn split_owner_repo(path: &str) -> Option<(String, String)> {
    // A single trailing slash is a common copy-paste artifact from a browser
    // URL bar and is not an extra path segment; strip at most one before
    // splitting.
    let path = path.strip_suffix('/').unwrap_or(path);
    let mut parts = path.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    // Anything beyond `owner/repo` (a subfolder, `tree/main`, etc.) or an
    // empty segment in the middle (e.g. `foo//bar`) is not a supported
    // remote-URL shape — reject rather than silently truncating/collapsing.
    if parts.next().is_some() {
        return None;
    }
    let repo = repo.trim_end_matches(".git");
    if owner.is_empty()
        || repo.is_empty()
        || is_dot_or_dotdot(owner)
        || is_dot_or_dotdot(repo)
    {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// `.` and `..` are filesystem-relative path components, not valid git
/// owner/repo names — reject them here so a downstream `cwd.join(repo)`
/// (used to compute the local target dir for `appz init`) can never resolve
/// to the current directory or its parent, which `--force` would otherwise
/// `remove_dir_all` on.
fn is_dot_or_dotdot(s: &str) -> bool {
    s == "." || s == ".."
}

/// Build the platform-specific archive-download URL for `ref_name` (a
/// branch name). Used by the template-download path (not-owned repos, or
/// GitLab/Bitbucket, which have no ownership signal).
pub fn archive_url(host: Host, owner: &str, repo: &str, ref_name: &str) -> String {
    match host {
        Host::GitHub => format!(
            "https://github.com/{owner}/{repo}/archive/refs/heads/{ref_name}.zip"
        ),
        Host::GitLab => format!(
            "https://gitlab.com/{owner}/{repo}/-/archive/{ref_name}/{repo}-{ref_name}.zip"
        ),
        Host::Bitbucket => format!("https://bitbucket.org/{owner}/{repo}/get/{ref_name}.zip"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remote_https_github() {
        let r = parse_remote("https://github.com/getappz/appz-dev-site").unwrap();
        assert_eq!(r.host, Host::GitHub);
        assert_eq!(r.owner, "getappz");
        assert_eq!(r.repo, "appz-dev-site");
        assert_eq!(r.url, "https://github.com/getappz/appz-dev-site");
    }

    #[test]
    fn parse_remote_strips_dot_git_suffix() {
        let r = parse_remote("https://github.com/getappz/appz-dev-site.git").unwrap();
        assert_eq!(r.repo, "appz-dev-site");
    }

    #[test]
    fn parse_remote_ssh_form() {
        let r = parse_remote("git@github.com:getappz/appz-dev-site.git").unwrap();
        assert_eq!(r.host, Host::GitHub);
        assert_eq!(r.owner, "getappz");
        assert_eq!(r.repo, "appz-dev-site");
        assert_eq!(r.url, "git@github.com:getappz/appz-dev-site.git");
    }

    #[test]
    fn parse_remote_gitlab() {
        let r = parse_remote("https://gitlab.com/foo/bar").unwrap();
        assert_eq!(r.host, Host::GitLab);
    }

    #[test]
    fn parse_remote_bitbucket() {
        let r = parse_remote("https://bitbucket.org/foo/bar").unwrap();
        assert_eq!(r.host, Host::Bitbucket);
    }

    #[test]
    fn parse_remote_rejects_unknown_host() {
        assert!(parse_remote("https://example.com/owner/repo").is_none());
    }

    #[test]
    fn parse_remote_rejects_bare_owner_repo_shorthand() {
        // No scheme -> not treated as remote (ambiguous with a local relative path).
        assert!(parse_remote("getappz/appz-dev-site").is_none());
    }

    #[test]
    fn parse_remote_rejects_local_paths() {
        assert!(parse_remote(".").is_none());
        assert!(parse_remote("./foo").is_none());
        assert!(parse_remote("../foo").is_none());
        assert!(parse_remote("C:\\Users\\shiva\\workspace\\appz-dev-site").is_none());
    }

    #[test]
    fn parse_remote_rejects_subfolder_path() {
        assert!(parse_remote("https://github.com/owner/repo/subfolder").is_none());
    }

    #[test]
    fn parse_remote_rejects_tree_branch_path() {
        assert!(parse_remote("https://github.com/owner/repo/tree/main").is_none());
    }

    #[test]
    fn parse_remote_rejects_double_slash() {
        assert!(parse_remote("https://github.com/foo//bar").is_none());
    }

    #[test]
    fn parse_remote_allows_single_trailing_slash() {
        let r = parse_remote("https://github.com/owner/repo/").unwrap();
        assert_eq!(r.owner, "owner");
        assert_eq!(r.repo, "repo");
    }

    #[test]
    fn parse_remote_rejects_dot_repo() {
        assert!(parse_remote("https://github.com/foo/.").is_none());
    }

    #[test]
    fn parse_remote_rejects_dotdot_repo() {
        assert!(parse_remote("https://github.com/foo/..").is_none());
    }

    #[test]
    fn parse_remote_rejects_dot_owner() {
        assert!(parse_remote("https://github.com/./repo").is_none());
    }

    #[test]
    fn parse_remote_rejects_dotdot_owner() {
        assert!(parse_remote("https://github.com/../repo").is_none());
    }

    #[test]
    fn parse_remote_accepts_uppercase_host() {
        let r = parse_remote("https://GitHub.com/owner/repo").unwrap();
        assert_eq!(r.host, Host::GitHub);
        assert_eq!(r.owner, "owner");
        assert_eq!(r.repo, "repo");
    }

    #[test]
    fn parse_remote_accepts_mixed_case_host() {
        let r = parse_remote("https://GitLab.COM/owner/repo").unwrap();
        assert_eq!(r.host, Host::GitLab);
    }

    #[test]
    fn classify_dot_is_local() {
        match classify(".") {
            InitSource::Local(p) => assert_eq!(p, std::path::PathBuf::from(".")),
            InitSource::Remote(_) => panic!("expected Local"),
        }
    }

    #[test]
    fn classify_github_url_is_remote() {
        match classify("https://github.com/getappz/appz-dev-site") {
            InitSource::Remote(r) => assert_eq!(r.repo, "appz-dev-site"),
            InitSource::Local(_) => panic!("expected Remote"),
        }
    }

    #[test]
    fn archive_url_github_format() {
        let url = archive_url(Host::GitHub, "getappz", "appz-dev-site", "main");
        assert_eq!(
            url,
            "https://github.com/getappz/appz-dev-site/archive/refs/heads/main.zip"
        );
    }

    #[test]
    fn archive_url_gitlab_format() {
        let url = archive_url(Host::GitLab, "foo", "bar", "main");
        assert_eq!(
            url,
            "https://gitlab.com/foo/bar/-/archive/main/bar-main.zip"
        );
    }

    #[test]
    fn archive_url_bitbucket_format() {
        let url = archive_url(Host::Bitbucket, "foo", "bar", "main");
        assert_eq!(url, "https://bitbucket.org/foo/bar/get/main.zip");
    }
}
