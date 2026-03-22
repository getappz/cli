//! Init provider trait and factory.
//!
//! Each source type (framework, git, remote archive, npm, local) implements
//! InitProvider. The detect module routes `appz init <arg>` to the correct provider.

use async_trait::async_trait;

use crate::config::InitContext;
use crate::error::{InitError, InitResult};
use crate::output::InitOutput;

/// Core trait abstracting an init source.
///
/// Each source type implements this trait. All command execution and file I/O
/// goes through the sandbox in InitContext.
#[async_trait]
pub trait InitProvider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &str;

    /// Slug identifier used for detection (e.g. "framework", "git", "npm").
    fn slug(&self) -> &str;

    /// Initialize a project from this source.
    ///
    /// The sandbox is already created at the target path. The provider should
    /// populate it (download, extract, run create command, etc.) and optionally
    /// run install.
    async fn init(&self, ctx: &InitContext) -> InitResult<InitOutput>;
}

// ---------------------------------------------------------------------------
// Provider registry / factory
// ---------------------------------------------------------------------------

use crate::providers;

/// Create a registry of all available init providers.
pub fn create_provider_registry() -> Vec<Box<dyn InitProvider>> {
    vec![
        Box::new(providers::blueprint::BlueprintProvider),
        Box::new(providers::wordpress::WordPressProvider),
        Box::new(providers::framework::FrameworkProvider),
        Box::new(providers::git::GitProvider),
        Box::new(providers::remote_archive::RemoteArchiveProvider),
        Box::new(providers::npm::NpmProvider),
        Box::new(providers::local::LocalProvider),
    ]
}

/// Look up a provider by slug.
pub fn get_provider(slug: &str) -> InitResult<Box<dyn InitProvider>> {
    let registry = create_provider_registry();
    for provider in registry {
        if provider.slug() == slug {
            return Ok(provider);
        }
    }
    Err(InitError::SourceNotFound(slug.to_string()))
}

/// List all available init source slugs.
pub fn available_source_slugs() -> Vec<&'static str> {
    vec!["blueprint", "wordpress", "framework", "git", "remote-archive", "npm", "local"]
}
