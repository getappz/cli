//! Check provider trait and factory.
//!
//! # Trait hierarchy
//!
//! ```text
//! CheckProvider              Core trait (dyn-compatible)
//!   ├── name / slug          Identity
//!   ├── detect               Does this checker apply to the project?
//!   ├── ensure_tool          Install checker CLI via sandbox/mise
//!   ├── check                Run the checker, return issues
//!   ├── fix                  Auto-fix safe issues
//!   └── supports_*           Capability flags
//! ```
//!
//! # Adding a new provider
//!
//! 1. Create `src/providers/<name>.rs` implementing [`CheckProvider`].
//! 2. Register it in [`create_provider_registry`].
//! 3. Detection is handled by the provider's own `detect()` method.

use std::path::Path;

use async_trait::async_trait;

use sandbox::SandboxProvider;

use crate::config::CheckContext;
use crate::error::CheckResult;
use crate::output::{CheckIssue, FixReport};

/// Core trait abstracting a code check provider.
///
/// Each linting/checking tool (Biome, tsc, Ruff, Clippy, etc.)
/// implements this trait. The trait is dyn-compatible so providers can
/// be stored as `Box<dyn CheckProvider>`.
///
/// All command execution goes through the sandbox
/// (`ctx.sandbox` / `ctx.exec()`) rather than raw process spawning.
#[async_trait]
pub trait CheckProvider: Send + Sync {
    // ------------------------------------------------------------------
    // Identity
    // ------------------------------------------------------------------

    /// Human-readable provider name (e.g. "Biome", "TypeScript Compiler").
    fn name(&self) -> &str;

    /// Slug identifier used in config and CLI (e.g. "biome", "tsc").
    fn slug(&self) -> &str;

    /// Short description of what this provider checks.
    fn description(&self) -> &str {
        ""
    }

    // ------------------------------------------------------------------
    // Detection
    // ------------------------------------------------------------------

    /// Detect whether this provider is applicable to the given project.
    ///
    /// Checks for language markers (package.json, Cargo.toml, etc.),
    /// config files (biome.json, tsconfig.json, etc.), and framework hints.
    fn detect(&self, project_dir: &Path, frameworks: &[&str]) -> bool;

    // ------------------------------------------------------------------
    // Tool management
    // ------------------------------------------------------------------

    /// The CLI tool name this provider requires (e.g. "biome", "ruff").
    fn tool_name(&self) -> &str;

    /// Ensure the provider's tool is installed via the sandbox.
    ///
    /// Default implementation runs `npm install -g <tool>` through the sandbox.
    /// Providers can override to use mise, npx, or a custom installer.
    async fn ensure_tool(&self, sandbox: &dyn SandboxProvider) -> CheckResult<()> {
        let tool = self.tool_name();
        let output = sandbox.exec(&format!("which {}", tool)).await;
        match output {
            Ok(o) if o.success() => Ok(()),
            _ => {
                // Try npm global install as default.
                let cmd = format!("npm install -g {}", tool);
                let _ = ui::status::info(&format!("Installing {} ...", tool));
                let output = sandbox.exec(&cmd).await?;
                if !output.success() {
                    return Err(crate::error::CheckerError::ToolInstallFailed {
                        tool: tool.to_string(),
                        reason: output.stderr(),
                    });
                }
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------------
    // Check operations
    // ------------------------------------------------------------------

    /// Run the check and return all found issues.
    ///
    /// Providers should use `ctx.exec()` for command execution and parse
    /// the tool's output (preferably JSON) into [`CheckIssue`] structs.
    async fn check(&self, ctx: &CheckContext) -> CheckResult<Vec<CheckIssue>>;

    /// Run auto-fix for safe fixes.
    ///
    /// Returns a report of what was fixed and what remains.
    async fn fix(&self, ctx: &CheckContext) -> CheckResult<FixReport> {
        let _ = ctx;
        Ok(FixReport::default())
    }

    // ------------------------------------------------------------------
    // Capabilities
    // ------------------------------------------------------------------

    /// Whether this provider supports `--fix` (auto-fix safe issues).
    fn supports_fix(&self) -> bool;

    /// Whether this provider supports `--format` (formatting checks).
    fn supports_format(&self) -> bool {
        false
    }

    /// Whether this provider supports `--watch` (watch mode).
    fn supports_watch(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Provider registry / factory
// ---------------------------------------------------------------------------

use crate::providers;

/// Create a registry of all available check providers.
pub fn create_provider_registry() -> Vec<Box<dyn CheckProvider>> {
    vec![
        Box::new(providers::biome::BiomeProvider),
        Box::new(providers::typescript::TypeScriptProvider),
        Box::new(providers::ruff::RuffProvider),
        Box::new(providers::clippy::ClippyProvider),
        Box::new(providers::phpstan::PHPStanProvider),
        Box::new(providers::stylelint::StylelintProvider),
        Box::new(providers::secrets::SecretScanProvider),
    ]
}

/// Look up a provider by slug from the registry.
pub fn get_provider(slug: &str) -> CheckResult<Box<dyn CheckProvider>> {
    let registry = create_provider_registry();
    for provider in registry {
        if provider.slug() == slug {
            return Ok(provider);
        }
    }
    Err(crate::error::CheckerError::ProviderNotFound {
        slug: slug.to_string(),
    })
}

/// List all available provider slugs.
pub fn available_provider_slugs() -> Vec<&'static str> {
    vec![
        "biome",
        "tsc",
        "ruff",
        "clippy",
        "phpstan",
        "stylelint",
        "secrets",
    ]
}

/// Detect which providers are applicable to the given project.
///
/// Returns providers in priority order. Uses file-existence heuristics
/// and framework hints to determine which linters should run.
pub fn detect_applicable_providers(
    project_dir: &Path,
    frameworks: &[&str],
) -> Vec<Box<dyn CheckProvider>> {
    let registry = create_provider_registry();
    registry
        .into_iter()
        .filter(|p| p.detect(project_dir, frameworks))
        .collect()
}
