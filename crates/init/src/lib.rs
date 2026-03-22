//! # Appz Init
//!
//! Project initialization from multiple sources: framework create commands,
//! git repositories, remote archives, npm templates, and local paths.
//!
//! ## What it does
//!
//! The init crate provides a unified interface for creating new projects
//! via `appz init`. It supports:
//!
//! - **Framework commands**: `appz init astro` → runs `npm create astro@latest`
//! - **Git providers**: GitHub, GitLab, Bitbucket repository archives
//! - **Remote archives**: Any `.zip`, `.tar.gz`, `.tar.xz`, `.tar.zstd` URL
//! - **npm packages**: `appz init npm:create-foo`
//! - **Local paths**: `appz init ./template`
//!
//! All operations execute within the sandbox for isolation and tool management.
//!
//! ## Architecture
//!
//! ```text
//! appz init <source>
//!     │
//!     ▼
//! detect::resolve_source(source) → InitProvider
//!     │
//!     ▼
//! provider.init(ctx) → InitOutput
//!     │
//!     ├── create_sandbox
//!     ├── download / extract / run create command
//!     └── optional: install dependencies
//! ```

pub mod blueprint_schema;
pub mod config;
pub mod detect;
pub mod error;
pub mod output;
pub mod provider;
pub mod providers;
pub mod run;
pub mod sources;
pub mod ui;

pub use config::{InitContext, InitOptions};
pub use error::{InitError, InitResult};
pub use output::InitOutput;
pub use provider::{
    available_source_slugs, create_provider_registry, get_provider, InitProvider,
};
pub use providers::framework::has_create_command;
pub use run::run;
