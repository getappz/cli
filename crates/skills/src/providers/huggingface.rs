//! HuggingFace Spaces skills provider.
//!
//! URLs: huggingface.co/spaces/owner/repo/blob|raw/main/SKILL.md

use super::{parse_frontmatter, HostProvider, ProviderMatch, RemoteSkill};
use std::sync::OnceLock;

pub struct HuggingFaceProvider;

const HOST: &str = "huggingface.co";

#[async_trait::async_trait]
impl HostProvider for HuggingFaceProvider {
    fn id(&self) -> &str {
        "huggingface"
    }

    fn display_name(&self) -> &str {
        "HuggingFace"
    }

    fn match_url(&self, url: &str) -> ProviderMatch {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ProviderMatch {
                matches: false,
                source_identifier: None,
            };
        }
        if let Ok(parsed) = url::Url::parse(url) {
            if parsed.host_str() != Some(HOST) {
                return ProviderMatch {
                    matches: false,
                    source_identifier: None,
                };
            }
        } else {
            return ProviderMatch {
                matches: false,
                source_identifier: None,
            };
        }
        if !url.to_lowercase().ends_with("/skill.md") {
            return ProviderMatch {
                matches: false,
                source_identifier: None,
            };
        }
        if !url.contains("/spaces/") {
            return ProviderMatch {
                matches: false,
                source_identifier: None,
            };
        }
        let source_id = parse_hf_url(url).map(|(o, r)| format!("huggingface/{}/{}", o, r));
        ProviderMatch {
            matches: true,
            source_identifier: source_id,
        }
    }

    async fn fetch_skill(&self, url: &str) -> Option<RemoteSkill> {
        let raw_url = url.replace("/blob/", "/raw/");
        let client = reqwest::Client::new();
        let res = client.get(&raw_url).send().await.ok()?;
        if !res.status().is_success() {
            return None;
        }
        let content = res.text().await.ok()?;
        let (name, description, metadata) = parse_frontmatter(&content)?;
        let (_owner, repo) = parse_hf_url(url)?;
        let install_name = metadata
            .as_ref()
            .and_then(|m| m.get("metadata"))
            .and_then(|m| m.as_object())
            .and_then(|m| m.get("install-name"))
            .and_then(|v| v.as_str())
            .unwrap_or(repo);
        Some(RemoteSkill {
            name,
            description,
            content,
            install_name: install_name.to_string(),
            source_url: url.to_string(),
            metadata,
        })
    }
}

fn regex_hf_spaces() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"/spaces/([^/]+)/([^/]+)").unwrap())
}

fn parse_hf_url(url: &str) -> Option<(&str, &str)> {
    let caps = regex_hf_spaces().captures(url)?;
    let owner = caps.get(1)?.as_str();
    let repo = caps.get(2)?.as_str();
    Some((owner, repo))
}

