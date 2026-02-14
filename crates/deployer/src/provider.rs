//! Deploy provider trait and factory.
//!
//! # Trait hierarchy
//!
//! ```text
//! DeployProvider              Core trait (dyn-compatible)
//!   ├── name / slug           Identity
//!   ├── check_prerequisites   Verify CLI tools and auth (via sandbox)
//!   ├── ensure_cli            Install provider CLI via sandbox/mise
//!   ├── detect_config         Auto-detect existing platform config
//!   ├── setup                 Interactive setup wizard
//!   ├── deploy / deploy_preview   Deployment operations
//!   ├── list_deployments      History
//!   └── rollback / teardown   Optional operations
//! ```
//!
//! # Adding a new provider
//!
//! 1. Create `src/providers/<name>.rs` implementing [`DeployProvider`].
//! 2. Register it in [`create_provider_registry`].
//! 3. Add detection rules in `src/detect.rs`.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use sandbox::SandboxProvider;

use crate::config::{DeployContext, SetupContext};
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeploymentInfo, DetectedConfig, PrerequisiteStatus,
};

/// Core trait abstracting a deployment provider.
///
/// Each hosting platform (Vercel, Netlify, Cloudflare Pages, etc.)
/// implements this trait. The trait is dyn-compatible so providers can
/// be stored as `Box<dyn DeployProvider>`.
///
/// All command execution goes through the sandbox
/// (`ctx.sandbox` / `ctx.exec()`) rather than raw process spawning.
#[async_trait]
pub trait DeployProvider: Send + Sync {
    // ------------------------------------------------------------------
    // Identity
    // ------------------------------------------------------------------

    /// Human-readable provider name (e.g. "Vercel", "Netlify").
    fn name(&self) -> &str;

    /// Slug identifier used in config and CLI (e.g. "vercel", "netlify").
    fn slug(&self) -> &str;

    /// Short description of the provider.
    fn description(&self) -> &str {
        ""
    }

    // ------------------------------------------------------------------
    // Prerequisites
    // ------------------------------------------------------------------

    /// Check if the provider's CLI tool is installed and auth is available.
    ///
    /// Uses the sandbox for command execution (`which <tool>`, `<tool> whoami`, etc.)
    /// rather than raw `which::which` or `tokio::process::Command`.
    async fn check_prerequisites(
        &self,
        sandbox: &dyn SandboxProvider,
    ) -> DeployResult<PrerequisiteStatus>;

    /// The CLI tool name this provider requires (e.g. "vercel", "netlify", "wrangler").
    fn cli_tool(&self) -> &str;

    /// The environment variable name for the auth token.
    fn auth_env_var(&self) -> &str;

    /// Ensure the provider's CLI tool is installed via the sandbox.
    ///
    /// Default implementation runs `npm install -g <cli_tool>` through the sandbox.
    /// Providers can override to use mise, npx, or a custom installer.
    async fn ensure_cli(&self, sandbox: &dyn SandboxProvider) -> DeployResult<()> {
        let tool = self.cli_tool();
        let cmd = format!("npm install -g {}", tool);
        let _ = ui::status::info(&format!("Installing {} CLI...", tool));
        let output = sandbox.exec(&cmd).await?;
        if !output.success() {
            return Err(DeployError::CommandFailed {
                command: cmd,
                reason: crate::providers::helpers::combined_output(&output),
            });
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Detection and setup
    // ------------------------------------------------------------------

    /// Detect if this provider is already configured in the project.
    ///
    /// Checks for platform-specific config files (vercel.json, netlify.toml, etc.).
    async fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>>;

    /// Run the interactive setup wizard for this provider.
    ///
    /// Should guide the user through authentication, project linking, and
    /// generate the necessary configuration. Uses `ctx.exec()` / `ctx.exec_interactive()`
    /// for all command execution.
    async fn setup(
        &self,
        ctx: &mut SetupContext,
    ) -> DeployResult<serde_json::Value>;

    // ------------------------------------------------------------------
    // Deployment operations
    // ------------------------------------------------------------------

    /// Deploy to production.
    async fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;

    /// Deploy a preview/staging build.
    async fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput>;

    // ------------------------------------------------------------------
    // History and management
    // ------------------------------------------------------------------

    /// List recent deployments.
    async fn list_deployments(
        &self,
        ctx: &DeployContext,
    ) -> DeployResult<Vec<DeploymentInfo>> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name().to_string(),
            operation: "list_deployments".into(),
        })
    }

    /// Rollback to a previous deployment.
    async fn rollback(
        &self,
        ctx: &DeployContext,
        deployment_id: &str,
    ) -> DeployResult<DeployOutput> {
        let _ = (ctx, deployment_id);
        Err(DeployError::Unsupported {
            provider: self.name().to_string(),
            operation: "rollback".into(),
        })
    }

    /// Tear down / remove the project from this provider.
    async fn teardown(&self, ctx: &DeployContext) -> DeployResult<()> {
        let _ = ctx;
        Err(DeployError::Unsupported {
            provider: self.name().to_string(),
            operation: "teardown".into(),
        })
    }

    // ------------------------------------------------------------------
    // Capabilities
    // ------------------------------------------------------------------

    /// Whether this provider supports environment variable management.
    fn supports_env_vars(&self) -> bool {
        false
    }

    /// Whether this provider supports custom domain management.
    fn supports_custom_domains(&self) -> bool {
        false
    }

    /// Whether this provider supports deployment rollback.
    fn supports_rollback(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Provider registry / factory
// ---------------------------------------------------------------------------

use crate::providers;

/// Create a registry of all available deploy providers.
pub fn create_provider_registry() -> Vec<Box<dyn DeployProvider>> {
    vec![
        Box::new(providers::vercel::VercelProvider),
        Box::new(providers::netlify::NetlifyProvider),
        Box::new(providers::cloudflare_pages::CloudflarePagesProvider),
        Box::new(providers::github_pages::GitHubPagesProvider),
        Box::new(providers::firebase::FirebaseProvider),
        Box::new(providers::aws_s3::AwsS3Provider),
        Box::new(providers::azure_static::AzureStaticProvider),
        Box::new(providers::surge::SurgeProvider),
        Box::new(providers::fly::FlyProvider),
        Box::new(providers::render::RenderProvider),
    ]
}

/// Look up a provider by slug from the registry.
pub fn get_provider(slug: &str) -> DeployResult<Box<dyn DeployProvider>> {
    let registry = create_provider_registry();
    for provider in registry {
        if provider.slug() == slug {
            return Ok(provider);
        }
    }
    Err(DeployError::ProviderNotFound {
        slug: slug.to_string(),
    })
}

/// Look up a provider by slug and return it along with a sandbox reference.
pub fn get_provider_with_sandbox(
    slug: &str,
    sandbox: Arc<dyn SandboxProvider>,
) -> DeployResult<(Box<dyn DeployProvider>, Arc<dyn SandboxProvider>)> {
    let provider = get_provider(slug)?;
    Ok((provider, sandbox))
}

/// List all available provider slugs.
pub fn available_provider_slugs() -> Vec<&'static str> {
    vec![
        "vercel",
        "netlify",
        "cloudflare-pages",
        "github-pages",
        "firebase",
        "aws-s3",
        "azure-static",
        "surge",
        "fly",
        "render",
    ]
}
