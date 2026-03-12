//! # Appz Deployer
//!
//! Universal deployment provider for static site hosting platforms.
//!
//! ## What it does
//!
//! The deployer provides a unified interface for deploying static sites to
//! any hosting platform (Vercel, Netlify, Cloudflare Pages, GitHub Pages,
//! and more). It auto-detects existing platform configurations, guides
//! users through setup, and works seamlessly in CI/CD pipelines.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  appz deploy [provider]                   │
//! │  CLI entry point — resolves provider and builds context   │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │              PlatformDetector (detect.rs)                 │
//! │  Scans for vercel.json, netlify.toml, wrangler.toml,     │
//! │  appz.json deploy targets, etc.                          │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │              DeployProvider (trait)                       │
//! │  check_prerequisites · detect_config · setup             │
//! │  deploy · deploy_preview · list · rollback               │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//!    ┌──────────┬────────┴─────────┬──────────────┐
//!    ▼          ▼                  ▼              ▼
//!  Vercel    Netlify    Cloudflare Pages    GitHub Pages
//!  Firebase  AWS S3     Azure Static        Surge
//!  Fly.io    Render     (more providers)
//! ```
//!
//! ## Module guide
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | [`DeployConfig`], [`DeployContext`], [`SetupContext`], config I/O |
//! | [`error`] | [`DeployError`] with miette diagnostics |
//! | [`provider`] | [`DeployProvider`] trait and provider registry/factory |
//! | [`output`] | [`DeployOutput`], [`DeploymentInfo`], [`DetectedPlatform`] |
//! | [`detect`] | Platform auto-detection from config files |
//! | [`providers`] | Individual provider implementations |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use deployer::{get_provider, DeployContext};
//! use sandbox::{SandboxConfig, create_sandbox};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = SandboxConfig::new("/tmp/my-project");
//! let mut sb = create_sandbox(&config)?;
//! sb.init(&config).await?;
//! let sb: Arc<dyn sandbox::SandboxProvider> = Arc::from(sb);
//!
//! let provider = get_provider("vercel")?;
//! let ctx = DeployContext::new(sb, "dist".into());
//! let output = provider.deploy(&ctx).await?;
//! println!("Deployed to: {}", output.url);
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod detect;
pub mod error;
pub mod output;
pub mod provider;
pub mod providers;

/// Quiet-aware UI helpers (internal use by providers).
pub(crate) mod ui;

// Re-export primary types for ergonomic imports.
pub use config::{
    is_ci_environment, read_deploy_config, read_deploy_config_async, write_deploy_config,
    write_deploy_config_async, CiPlatform, DeployConfig, DeployContext, DeployHooks, SetupContext,
};
pub use detect::{detect, detect_all};
pub use error::{DeployError, DeployResult};
pub use output::{
    DeployOutput, DeployStatus, DeploymentInfo, DetectedConfig, DetectedPlatform,
    DetectionConfidence, PrerequisiteStatus,
};
pub use provider::{
    available_provider_slugs, create_provider_registry, get_provider,
    get_provider_with_sandbox, DeployProvider,
};
