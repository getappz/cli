//! Well-known skills provider (RFC 8615).
//!
//! Fetches from /.well-known/skills/index.json.

use super::{parse_frontmatter, HostProvider, ProviderMatch, RemoteSkill};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
pub struct WellKnownSkillEntry {
    pub name: String,
    pub description: String,
    pub files: Vec<String>,
}

#[derive(Deserialize)]
struct WellKnownIndex {
    skills: Vec<WellKnownSkillEntry>,
}

pub struct WellKnownProvider;

const WELL_KNOWN_PATH: &str = ".well-known/skills";
const INDEX_FILE: &str = "index.json";

#[async_trait::async_trait]
impl HostProvider for WellKnownProvider {
    fn id(&self) -> &str {
        "well-known"
    }

    fn display_name(&self) -> &str {
        "Well-Known Skills"
    }

    fn match_url(&self, url: &str) -> ProviderMatch {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ProviderMatch { matches: false, source_identifier: None };
        }
        if let Ok(parsed) = url::Url::parse(url) {
            let host = parsed.host_str().unwrap_or("");
            for excluded in ["github.com", "gitlab.com", "huggingface.co"] {
                if host == excluded {
                    return ProviderMatch { matches: false, source_identifier: None };
                }
            }
            if url.to_lowercase().ends_with("/skill.md") {
                return ProviderMatch { matches: false, source_identifier: None };
            }
            if url.ends_with(".git") {
                return ProviderMatch { matches: false, source_identifier: None };
            }
            let base_path = parsed.path().trim_end_matches('/');
            let _path_relative = format!("{}/{}", base_path, WELL_KNOWN_PATH);
            let source_id = format!("wellknown/{}", host);
            return ProviderMatch {
                matches: true,
                source_identifier: Some(source_id),
            };
        }
        ProviderMatch { matches: false, source_identifier: None }
    }

    async fn fetch_skill(&self, url: &str) -> Option<RemoteSkill> {
        let (index, base_url) = self.fetch_index(url).await?;
        let parsed = url::Url::parse(url).ok()?;
        let path = parsed.path();

        let skill_name = if let Some(m) = regex_well_known_skill_path().captures(path) {
            m.get(1).map(|x| x.as_str().to_string())
        } else if index.skills.len() == 1 {
            Some(index.skills[0].name.clone())
        } else {
            None
        }?;

        let entry = index.skills.iter().find(|s| s.name == skill_name)?;
        self.fetch_skill_by_entry(&base_url, entry).await
    }
}

fn regex_well_known_skill_path() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"/.well-known/skills/([^/]+)/?$").unwrap())
}

impl WellKnownProvider {
    async fn fetch_index(&self, base_url: &str) -> Option<(WellKnownIndex, String)> {
        let parsed = url::Url::parse(base_url).ok()?;
        let base_path = parsed.path().trim_end_matches('/');
        let base = format!("{}://{}", parsed.scheme(), parsed.host_str()?);

        let urls_to_try = [
            (format!("{}{}/{}/{}", base, base_path, WELL_KNOWN_PATH, INDEX_FILE), format!("{}{}", base, base_path)),
            (format!("{}/{}/{}", base, WELL_KNOWN_PATH, INDEX_FILE), base.clone()),
        ];

        for (index_url, resolved_base) in urls_to_try {
            if let Ok(res) = reqwest::Client::new().get(&index_url).send().await {
                if res.status().is_success() {
                    if let Ok(index) = res.json::<WellKnownIndex>().await {
                        if index.skills.iter().all(|e| valid_entry(e)) {
                            return Some((index, resolved_base));
                        }
                    }
                }
            }
        }
        None
    }

    async fn fetch_skill_by_entry(&self, base_url: &str, entry: &WellKnownSkillEntry) -> Option<RemoteSkill> {
        let skill_base = format!("{}/{}/{}", base_url.trim_end_matches('/'), WELL_KNOWN_PATH, entry.name);
        let mut content = String::new();
        for file in &entry.files {
            let file_url = format!("{}/{}", skill_base.trim_end_matches('/'), file.trim_start_matches('/'));
            if let Ok(res) = reqwest::Client::new().get(&file_url).send().await {
                if res.status().is_success() {
                    if let Ok(text) = res.text().await {
                        if file.eq_ignore_ascii_case("skill.md") {
                            content = text;
                            break;
                        }
                    }
                }
            }
        }
        if content.is_empty() {
            return None;
        }
        let (name, description, metadata) = parse_frontmatter(&content)?;
        Some(RemoteSkill {
            name,
            description,
            content,
            install_name: entry.name.clone(),
            source_url: format!("{}/skill.md", skill_base),
            metadata,
        })
    }
}

fn valid_entry(entry: &WellKnownSkillEntry) -> bool {
    if entry.name.is_empty() || entry.description.is_empty() || entry.files.is_empty() {
        return false;
    }
    for f in &entry.files {
        if f.starts_with('/') || f.starts_with('\\') || f.contains("..") {
            return false;
        }
    }
    entry.files.iter().any(|f| f.eq_ignore_ascii_case("skill.md"))
}

