//! # Appz Checker
//!
//! Universal code checker with auto-fix and AI-assisted repair.
//!
//! ## What it does
//!
//! The checker provides a unified interface for running language-specific
//! linters, type checkers, formatters, and security scanners on any project.
//! It auto-detects the project's language/framework and runs the best
//! industry-standard tools, with support for auto-fixing safe issues and
//! AI-assisted repair of complex errors.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              appz check [--fix] [--ai-fix]                │
//! │  CLI entry point — detects project, builds context        │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │              ParallelRunner (runner.rs)                    │
//! │  Detects applicable providers, runs them concurrently,    │
//! │  streams results, aggregates into CheckReport             │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │              CheckProvider (trait)                         │
//! │  detect · ensure_tool · check · fix                       │
//! │  supports_fix · supports_format · supports_watch          │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//!    ┌──────────┬────────┴─────────┬──────────────┐
//!    ▼          ▼                  ▼              ▼
//!  Biome     tsc --noEmit       Ruff         Clippy
//!  Stylelint PHPStan           gitleaks     (more)
//! ```
//!
//! ## Module guide
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | [`CheckConfig`], [`CheckContext`], config I/O |
//! | [`error`] | [`CheckerError`] with miette diagnostics |
//! | [`provider`] | [`CheckProvider`] trait and provider registry/factory |
//! | [`output`] | [`CheckIssue`], [`Severity`], [`CheckReport`], [`FixReport`] |
//! | [`runner`] | Parallel check execution engine |
//! | [`fixer`] | Auto-fix orchestrator (`--fix`) |
//! | [`ai_fixer`] | AI-assisted fix with human-in-loop (`--ai-fix`) |
//! | [`detect`] | Language/framework detection for provider selection |
//! | [`git`] | Git integration for `--changed` and `--staged` |
//! | [`cache`] | Content-hash result caching |
//! | [`init`] | Best-practice config file generation (`--init`) |
//! | [`providers`] | Individual check provider implementations |

pub mod ai_fixer;
pub mod cache;
pub mod config;
pub mod detect;
pub mod error;
pub mod fixer;
pub mod git;
pub mod init;
pub mod output;
pub mod provider;
pub mod providers;
pub mod runner;

// Re-export primary types for ergonomic imports.
pub use config::{read_check_config, read_check_config_async, AiModelConfig, CheckConfig, CheckContext};
pub use error::{CheckResult, CheckerError};
pub use output::{CheckIssue, CheckReport, FixKind, FixReport, FixSuggestion, Severity};
pub use provider::{
    available_provider_slugs, create_provider_registry, detect_applicable_providers, get_provider,
    CheckProvider,
};
pub use runner::run_checks;
