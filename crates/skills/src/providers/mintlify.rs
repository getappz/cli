//! Mintlify-hosted skills provider.
//!
//! URLs ending in /skill.md with frontmatter metadata.mintlify-proj.

use super::{parse_frontmatter, HostProvider, ProviderMatch, RemoteSkill};

pub struct MintlifyProvider;

#[async_trait::async_trait]
impl HostProvider for MintlifyProvider {
    fn id(&self) -> &str {
        "mintlify"
    }

    fn display_name(&self) -> &str {
        "Mintlify"
    }

    fn match_url(&self, url: &str) -> ProviderMatch {
        if !url.starts_with("http://") && !url.starts_with("https://") {
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
        if url.contains("github.com") || url.contains("gitlab.com") || url.contains("huggingface.co") {
            return ProviderMatch {
                matches: false,
                source_identifier: None,
            };
        }
        ProviderMatch {
            matches: true,
            source_identifier: Some("mintlify/com".to_string()),
        }
    }

    async fn fetch_skill(&self, url: &str) -> Option<RemoteSkill> {
        let client = reqwest::Client::new();
        let res = client
            .get(url)
            .send()
            .await
            .ok()?;
        if !res.status().is_success() {
            return None;
        }
        let content = res.text().await.ok()?;
        let (name, description, metadata) = parse_frontmatter(&content)?;
        let metadata_map = metadata.as_ref()?;
        let meta_obj = metadata_map.get("metadata").and_then(|v| v.as_object())?;
        let mintlify_proj = meta_obj.get("mintlify-proj").and_then(|v| v.as_str())?;
        Some(RemoteSkill {
            name,
            description,
            content,
            install_name: mintlify_proj.to_string(),
            source_url: url.to_string(),
            metadata,
        })
    }
}

