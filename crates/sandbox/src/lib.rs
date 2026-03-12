//! # Appz Sandbox
//!
//! Scoped execution environment for local static website development, build,
//! and deployment.
//!
//! ## What it does
//!
//! The sandbox confines **all** filesystem operations and command execution to a
//! single project root directory. It uses [mise](https://mise.jdx.dev/) to
//! manage tool versions (Node, Hugo, Bun, etc.) so every project gets a
//! reproducible, isolated toolchain without polluting the host system.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    create_sandbox()                      │
//! │  Entry point — builds a provider from SandboxConfig      │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │               SandboxProvider (trait)                     │
//! │  init · teardown · fs · exec · exec_interactive           │
//! │  ensure_tool · exec_with_tool · exec_all                  │
//! │                                                           │
//! │  SandboxProviderExt (auto-impl extension trait)           │
//! │  write_files_progress · read_files_progress               │
//! │  remove_files_progress · copy_progress                    │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼──────────────────────────────────┐
//! │               LocalProvider (impl)                       │
//! │  Host-native execution via command crate + MiseManager    │
//! │  Spinners and status messages via ui crate                │
//! └──┬───────────────┬───────────────┬──────────────────────┘
//!    │               │               │
//!    ▼               ▼               ▼
//!  ScopedFs       MiseManager     ui::{status, progress}
//!  Path-safe      Tool install    Spinners, progress bars,
//!  file I/O       & exec via      status messages
//!  + batch ops    mise CLI        (quiet-aware)
//! ```
//!
//! ## Module guide
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | [`SandboxConfig`], [`SandboxSettings`], [`MiseToolSpec`], [`ProviderKind`] |
//! | [`error`] | [`SandboxError`] with miette diagnostics and actionable help messages |
//! | [`provider`] | [`SandboxProvider`] trait + [`SandboxProviderExt`] batch-with-progress |
//! | [`local`] | [`LocalProvider`](local::LocalProvider) — host filesystem implementation |
//! | [`scoped_fs`] | [`ScopedFs`] — path-safe I/O with parallel batch operations |
//! | [`mise`] | [`MiseManager`](mise::MiseManager) — mise tool version manager wrapper |
//! | [`json_ops`] | Read / write / deep-merge JSON files through [`ScopedFs`] |
//! | [`toml_ops`] | Read / write TOML files through [`ScopedFs`] |
//!
//! ## Security model
//!
//! [`ScopedFs`] is the security boundary. Every path is resolved relative to
//! the sandbox root and validated **before** any I/O occurs:
//!
//! - Absolute paths are rejected immediately.
//! - `..` traversal that escapes the root is rejected.
//! - Symlinks are resolved via `canonicalize` and checked against the root.
//! - All errors produce [`SandboxError::PathEscape`] with a help message.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = SandboxConfig::new("/tmp/my-project")
//!     .with_settings(
//!         SandboxSettings::default()
//!             .with_tool("node", Some("22"))
//!             .with_tool("bun", None::<String>),
//!     );
//!
//! let sandbox = create_sandbox(config).await?;
//!
//! // Scoped file operations
//! sandbox.fs().write_string("hello.txt", "world")?;
//!
//! // Execute commands through mise
//! let output = sandbox.exec("node --version").await?;
//! println!("{}", output.stdout_trimmed());
//! # Ok(())
//! # }
//! ```
//!
//! ## Batch operations with progress
//!
//! For large file counts, use the [`SandboxProviderExt`] methods which show a
//! progress bar and parallelise work via rayon:
//!
//! ```rust,no_run
//! use sandbox::{create_sandbox, SandboxConfig, SandboxProviderExt};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let sandbox = create_sandbox(SandboxConfig::new("/tmp/site")).await?;
//!
//! // Write 1000 files in parallel with a progress bar
//! let items: Vec<(String, String)> = (0..1000)
//!     .map(|i| (format!("pages/page_{}.html", i), format!("<h1>Page {}</h1>", i)))
//!     .collect();
//! sandbox.write_files_progress(&items, "Generating pages")?;
//!
//! // Run build + lint concurrently
//! let results = sandbox.exec_all(&["npm run build", "npm run lint"]).await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Quiet mode
//!
//! Suppress all spinners, progress bars, and status messages (useful for CI or
//! tests) by setting [`SandboxSettings::quiet`]:
//!
//! ```rust,no_run
//! # use sandbox::SandboxSettings;
//! let settings = SandboxSettings::default().quiet();
//! ```

pub mod config;
pub mod error;
pub mod json_ops;
pub mod local;
pub mod mise;
pub mod provider;
pub mod scoped_fs;
pub mod toml_ops;

// Re-export primary types for ergonomic imports.
pub use config::{MiseToolSpec, ProviderKind, SandboxConfig, SandboxSettings};
pub use error::{SandboxError, SandboxResult};
pub use provider::{CommandOutput, SandboxProvider, SandboxProviderExt};
pub use scoped_fs::ScopedFs;

/// Create and initialise a sandbox from the given configuration.
///
/// Returns a boxed [`SandboxProvider`] ready for use. Currently only
/// [`ProviderKind::Local`] is supported.
///
/// Progress indicators and status messages are displayed unless
/// [`SandboxSettings::quiet`] is set.
pub async fn create_sandbox(
    config: SandboxConfig,
) -> SandboxResult<Box<dyn SandboxProvider>> {
    let quiet = config.settings.quiet;
    let provider_label = match config.provider {
        ProviderKind::Local => "local",
        ProviderKind::Docker => "docker",
    };

    if !quiet {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title(&format!(
            "Initialising {} sandbox",
            provider_label
        ));
    }

    let result: SandboxResult<Box<dyn SandboxProvider>> = match config.provider {
        ProviderKind::Local => {
            let mut provider = local::LocalProvider::new();
            provider.init(&config).await?;
            Ok(Box::new(provider))
        }
        ProviderKind::Docker => Err(SandboxError::Other(
            "Docker provider is not yet supported".to_string(),
        )),
    };

    if !quiet {
        match &result {
            Ok(_) => {
                let _ = ui::layout::blank_line();
            }
            Err(e) => {
                let _ = ui::status::error(&format!("Sandbox initialisation failed: {}", e));
            }
        }
    }

    result
}
