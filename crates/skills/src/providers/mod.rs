//! Remote skill host providers (Mintlify, HuggingFace, WellKnown).

mod huggingface;
mod mintlify;
mod wellknown;

pub use huggingface::HuggingFaceProvider;
pub use mintlify::MintlifyProvider;
pub use wellknown::WellKnownProvider;

use serde::Deserialize;
use std::collections::HashMap;

/// Parsed skill from a remote host.
#[derive(Clone, Debug)]
pub struct RemoteSkill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub install_name: String,
    pub source_url: String,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Result of matching a URL to a provider.
#[derive(Clone, Debug)]
pub struct ProviderMatch {
    pub matches: bool,
    pub source_identifier: Option<String>,
}

/// Trait for remote SKILL.md host providers.
#[async_trait::async_trait]
pub trait HostProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn match_url(&self, url: &str) -> ProviderMatch;
    async fn fetch_skill(&self, url: &str) -> Option<RemoteSkill>;
}

/// Find the first provider that matches the URL.
pub fn find_provider(url: &str) -> Option<ProviderHandle> {
    if MintlifyProvider.match_url(url).matches {
        return Some(ProviderHandle::Mintlify(MintlifyProvider));
    }
    if HuggingFaceProvider.match_url(url).matches {
        return Some(ProviderHandle::HuggingFace(HuggingFaceProvider));
    }
    if WellKnownProvider.match_url(url).matches {
        return Some(ProviderHandle::WellKnown(WellKnownProvider));
    }
    None
}

/// Type-erased provider handle.
pub enum ProviderHandle {
    Mintlify(MintlifyProvider),
    HuggingFace(HuggingFaceProvider),
    WellKnown(WellKnownProvider),
}

impl ProviderHandle {
    pub fn id(&self) -> &str {
        match self {
            ProviderHandle::Mintlify(p) => p.id(),
            ProviderHandle::HuggingFace(p) => p.id(),
            ProviderHandle::WellKnown(p) => p.id(),
        }
    }
    pub async fn fetch_skill(&self, url: &str) -> Option<RemoteSkill> {
        match self {
            ProviderHandle::Mintlify(p) => p.fetch_skill(url).await,
            ProviderHandle::HuggingFace(p) => p.fetch_skill(url).await,
            ProviderHandle::WellKnown(p) => p.fetch_skill(url).await,
        }
    }
}

fn parse_frontmatter(content: &str) -> Option<(String, String, Option<HashMap<String, serde_json::Value>>)> {
    let content = content.trim_start();
    let rest = content.strip_prefix("---")?;
    let rest = rest.trim_start_matches(['\n', '\r']);
    let end = rest.find("\n---").or_else(|| rest.find("\r\n---"))?;
    let yaml = rest[..end].trim();
    #[derive(Deserialize)]
    struct Fm {
        name: Option<String>,
        description: Option<String>,
        #[serde(flatten)]
        rest: HashMap<String, serde_json::Value>,
    }
    let fm: Fm = serde_yaml::from_str(yaml).ok()?;
    let name = fm.name?;
    let description = fm.description?;
    let metadata = if fm.rest.is_empty() {
        None
    } else {
        Some(fm.rest)
    };
    Some((name, description, metadata))
}
