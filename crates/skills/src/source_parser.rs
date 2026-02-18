//! Parse skill sources into structured format.
//!
//! Supports: local paths, GitHub URLs, GitLab URLs, owner/repo@skill, owner/repo:skill1,skill2,
//! skills.sh URLs, direct skill.md URLs, well-known.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Result of parsing a source with optional skill filters (skillman-style).
#[derive(Clone, Debug)]
pub struct ParsedSourceWithSkills {
    pub parsed: ParsedSource,
    /// Specific skills to install (empty = all).
    pub skills: Vec<String>,
}

/// Parsed source type.
#[derive(Clone, Debug)]
pub enum ParsedSource {
    Local {
        path: PathBuf,
    },
    DirectUrl {
        url: String,
    },
    GitHub {
        url: String,
        ref_name: Option<String>,
        subpath: Option<String>,
        skill_filter: Option<String>,
        /// Multiple skills (when more than one specified).
        skill_filters: Option<Vec<String>>,
    },
    GitLab {
        url: String,
        ref_name: Option<String>,
        subpath: Option<String>,
    },
    WellKnown {
        url: String,
    },
    Git {
        url: String,
    },
}

/// Source aliases: common shorthand -> canonical source.
const SOURCE_ALIASES: &[(&str, &str)] = &[("coinbase/agentWallet", "coinbase/agentic-wallet-skills")];

/// Parse a source string into a structured format.
pub fn parse_source(input: &str) -> ParsedSource {
    let input = resolve_alias(input);

    if is_local_path(input) {
        let path = if input.starts_with('/')
            || (input.len() >= 2 && input.chars().nth(1) == Some(':'))
        {
            PathBuf::from(input)
        } else {
            PathBuf::from(".").join(input)
        };
        return ParsedSource::Local { path };
    }

    if is_direct_skill_url(input) {
        return ParsedSource::DirectUrl {
            url: input.to_string(),
        };
    }

    // GitHub URL with path: .../tree/branch/path
    if let Some(caps) = regex_for_tree_path().captures(input) {
        let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let ref_name = caps.get(3).map(|m| m.as_str().to_string());
        let subpath = caps.get(4).map(|m| m.as_str().to_string());
        if !owner.is_empty() && !repo.is_empty() {
            let url = format!("https://github.com/{}/{}.git", owner, repo);
            return ParsedSource::GitHub {
                url,
                ref_name,
                subpath,
                skill_filter: None,
                skill_filters: None,
            };
        }
    }

    // GitHub shorthand: owner/repo@skill-name
    if let Some(caps) = regex_for_at_skill().captures(input) {
        let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let skill_filter = caps.get(3).map(|m| m.as_str().to_string());
        if !owner.is_empty() && !repo.is_empty() && !input.contains(':') && !input.starts_with('.') && !input.starts_with('/') {
            let url = format!("https://github.com/{}/{}.git", owner, repo);
            return ParsedSource::GitHub {
                url,
                ref_name: None,
                subpath: None,
                skill_filter,
                skill_filters: None,
            };
        }
    }

    // skills.sh URLs: https://skills.sh/owner/repo/skill-name or skills.sh/owner/repo
    if let Some((source, skills)) = parse_skills_sh(input) {
        let url = format!("https://github.com/{}.git", source);
        let (skill_filter, skill_filters) = match skills.len() {
            0 => (None, None),
            1 => (Some(skills[0].clone()), None),
            _ => (None, Some(skills)),
        };
        return ParsedSource::GitHub {
            url,
            ref_name: None,
            subpath: None,
            skill_filter,
            skill_filters,
        };
    }

    // Colon format: owner/repo:skill1,skill2 or owner/repo:skill1:skill2
    if let Some((source, skills)) = parse_colon_format(input) {
        let url = format!("https://github.com/{}.git", source);
        let (skill_filter, skill_filters) = match skills.len() {
            0 => (None, None),
            1 => (Some(skills[0].clone()), None),
            _ => (None, Some(skills)),
        };
        return ParsedSource::GitHub {
            url,
            ref_name: None,
            subpath: None,
            skill_filter,
            skill_filters,
        };
    }

    // GitHub shorthand: owner/repo or owner/repo/path
    if let Some(caps) = regex_for_owner_repo().captures(input) {
        let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let subpath = caps.get(3).map(|m| m.as_str().to_string());
        if !owner.is_empty() && !repo.is_empty() && !input.contains(':') && !input.starts_with('.') && !input.starts_with('/') {
            let url = format!("https://github.com/{}/{}.git", owner, repo.trim_end_matches(".git"));
            return ParsedSource::GitHub {
                url,
                ref_name: None,
                subpath,
                skill_filter: None,
                skill_filters: None,
            };
        }
    }

    // GitHub full URL
    if input.contains("github.com/") {
        if let Some(caps) = regex_for_github_url().captures(input) {
            let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let repo = caps.get(2).map(|m| m.as_str().trim_end_matches(".git")).unwrap_or("");
            if !owner.is_empty() && !repo.is_empty() {
                return ParsedSource::GitHub {
                    url: format!("https://github.com/{}/{}.git", owner, repo),
                    ref_name: None,
                    subpath: None,
                    skill_filter: None,
                    skill_filters: None,
                };
            }
        }
    }

    // GitLab URL with tree path
    if input.contains("gitlab.com") && input.contains("/-/tree/") {
        if let Some(caps) = regex_for_gitlab_tree().captures(input) {
            let protocol = caps.get(1).map(|m| m.as_str()).unwrap_or("https");
            let hostname = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let repo_path = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let ref_name = caps.get(4).map(|m| m.as_str().to_string());
            let subpath = caps.get(5).map(|m| m.as_str().to_string());
            if hostname != "github.com" && !repo_path.is_empty() {
                let clean = repo_path.trim_end_matches(".git");
                let url = format!("{}://{}/{}.git", protocol, hostname, clean);
                return ParsedSource::GitLab {
                    url,
                    ref_name,
                    subpath,
                };
            }
        }
    }

    // Well-known URL (arbitrary HTTP(S) that isn't a known git host)
    if is_well_known_url(input) {
        return ParsedSource::WellKnown {
            url: input.to_string(),
        };
    }

    // Fallback: generic git URL
    ParsedSource::Git {
        url: input.to_string(),
    }
}

fn resolve_alias(input: &str) -> &str {
    for (alias, target) in SOURCE_ALIASES {
        if input == *alias {
            return target;
        }
    }
    input
}

fn is_local_path(input: &str) -> bool {
    input.starts_with("./")
        || input.starts_with("../")
        || input == "."
        || input == ".."
        || (input.len() >= 2 && input.chars().nth(1) == Some(':') && input.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false))
        || input.starts_with('/')
}

fn is_direct_skill_url(input: &str) -> bool {
    if !input.starts_with("http://") && !input.starts_with("https://") {
        return false;
    }
    let lower = input.to_lowercase();
    if !lower.ends_with("/skill.md") {
        return false;
    }
    if input.contains("github.com/") && !input.contains("raw.githubusercontent.com") {
        if !input.contains("/blob/") && !input.contains("/raw/") {
            return false;
        }
    }
    if input.contains("gitlab.com/") && !input.contains("/-/raw/") {
        return false;
    }
    true
}

fn is_well_known_url(input: &str) -> bool {
    if !input.starts_with("http://") && !input.starts_with("https://") {
        return false;
    }
    if let Ok(parsed) = url::Url::parse(input) {
        let host = parsed.host_str().unwrap_or("");
        for excluded in ["github.com", "gitlab.com", "huggingface.co", "raw.githubusercontent.com"] {
            if host == excluded {
                return false;
            }
        }
        if input.to_lowercase().ends_with("/skill.md") {
            return false;
        }
        if input.ends_with(".git") {
            return false;
        }
        return true;
    }
    false
}

fn regex_for_tree_path() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)").unwrap())
}

fn regex_for_at_skill() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^([^/]+)/([^/@]+)@(.+)$").unwrap())
}

fn regex_for_owner_repo() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^([^/]+)/([^/]+)(?:/(.+))?$").unwrap())
}

fn regex_for_github_url() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)github\.com/([^/]+)/([^/]+?)(?:\.git)?/?").unwrap())
}

fn regex_for_gitlab_tree() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^(https?)://([^/]+)/(.+?)/-/tree/([^/]+)(?:/(.+))?$").unwrap())
}

/// Parse skills.sh URLs: (?:https?://)?skills.sh/owner/repo or .../owner/repo/skill1/skill2
fn parse_skills_sh(input: &str) -> Option<(String, Vec<String>)> {
    let rest = input
        .strip_prefix("https://skills.sh/")
        .or_else(|| input.strip_prefix("http://skills.sh/"))
        .or_else(|| input.strip_prefix("skills.sh/"))?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return None;
    }
    let source = format!("{}/{}", parts[0], parts[1]);
    let skills: Vec<String> = parts[2..]
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| *s != "*")
        .map(String::from)
        .collect();
    Some((source, skills))
}

/// Parse colon format: owner/repo:skill1,skill2 or owner/repo:skill1:skill2
fn parse_colon_format(input: &str) -> Option<(String, Vec<String>)> {
    if input.starts_with('.') || input.starts_with('/') || input.contains("github.com") || input.contains("gitlab.com") {
        return None;
    }
    let mut split = input.splitn(2, ':');
    let source = split.next()?.trim();
    let rest = split.next()?;
    if source.is_empty() || !source.contains('/') {
        return None;
    }
    let skills: Vec<String> = rest
        .split(|c| c == ',' || c == ':')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|s| *s != "*")
        .map(String::from)
        .collect();
    Some((source.to_string(), skills))
}

/// Parse source string and extract skill filters (skillman-style). Use for config and install.
pub fn parse_source_with_skills(input: &str) -> ParsedSourceWithSkills {
    let parsed = parse_source(input);
    let skills = match &parsed {
        ParsedSource::GitHub {
            skill_filter,
            skill_filters,
            ..
        } => {
            if let Some(ref filters) = skill_filters {
                filters.clone()
            } else if let Some(ref f) = skill_filter {
                vec![f.clone()]
            } else {
                vec![]
            }
        }
        _ => vec![],
    };
    ParsedSourceWithSkills { parsed, skills }
}

/// Extract owner/repo from a parsed source for lock file and telemetry.
pub fn get_owner_repo(parsed: &ParsedSource) -> Option<String> {
    match parsed {
        ParsedSource::GitHub { url, .. } | ParsedSource::Git { url } => {
            if url.starts_with("http://") || url.starts_with("https://") {
                if let Ok(u) = url::Url::parse(url) {
                    let path = u.path().trim_start_matches('/').trim_end_matches(".git");
                    if path.contains('/') {
                        return Some(path.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    None
}
