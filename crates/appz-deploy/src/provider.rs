use std::path::Path;
use std::time::Duration;

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{DeployOutput, DeploymentInfo, DetectedConfig, PrerequisiteStatus};

/// Timeout for the actual deploy invocation (platform CLIs can legitimately
/// take minutes to build+upload).
pub const DEPLOY_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Timeout for quick status/list queries.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// A hosting-platform deploy provider. Every operation shells out to the
/// platform's OWN CLI (vercel/netlify/wrangler/...) — appz never talks to a
/// platform's API directly; it's a unified front-end, not a competing cloud.
///
/// Synchronous by design: deploy CLIs are one-shot child processes, not
/// long-lived connections, so there's no need for an async runtime here
/// (the upstream version this was ported from wrapped every method in
/// `async_trait` for no operational benefit).
pub trait DeployProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn slug(&self) -> &'static str;
    fn cli_tool(&self) -> &'static str;
    /// npm package to `npm install -g` when the CLI is missing — not always
    /// the same as `cli_tool()` (e.g. Netlify's binary is `netlify` but the
    /// package is `netlify-cli`).
    fn install_package(&self) -> &'static str;
    fn auth_env_var(&self) -> &'static str;

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus>;
    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>>;

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;
    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "list_deployments",
        })
    }

    fn rollback(&self, ctx: &DeployContext, deployment_id: &str) -> DeployResult<DeployOutput> {
        let _ = (ctx, deployment_id);
        Err(DeployError::Unsupported {
            provider: self.name(),
            operation: "rollback",
        })
    }
}

pub fn create_provider_registry() -> Vec<Box<dyn DeployProvider>> {
    vec![
        Box::new(crate::providers::vercel::VercelProvider),
        Box::new(crate::providers::netlify::NetlifyProvider),
    ]
}

pub fn get_provider(slug: &str) -> DeployResult<Box<dyn DeployProvider>> {
    create_provider_registry()
        .into_iter()
        .find(|p| p.slug() == slug)
        .ok_or_else(|| DeployError::ProviderNotFound {
            slug: slug.to_string(),
        })
}

pub fn available_provider_slugs() -> Vec<&'static str> {
    create_provider_registry()
        .iter()
        .map(|p| p.slug())
        .collect()
}
