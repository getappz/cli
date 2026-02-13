//! Sandbox configuration types.
//!
//! All sandbox behaviour is driven by [`SandboxConfig`] which combines a
//! project path, a provider backend, and runtime [`SandboxSettings`].
//!
//! # Building a config
//!
//! Use the builder pattern for ergonomic construction:
//!
//! ```rust
//! use sandbox::{SandboxConfig, SandboxSettings, ProviderKind};
//!
//! let config = SandboxConfig::new("/tmp/my-site")
//!     .with_provider(ProviderKind::Local)
//!     .with_settings(
//!         SandboxSettings::default()
//!             .with_tool("node", Some("22"))
//!             .with_tool("hugo", Some("0.139"))
//!             .with_env("NODE_ENV", "production")
//!             .with_dotenv(".env.local")
//!             .quiet(),   // suppress progress UI
//!     );
//! ```
//!
//! # Serialisation
//!
//! Both [`SandboxConfig`] and [`SandboxSettings`] derive `Serialize` /
//! `Deserialize` so they can be persisted to JSON or TOML config files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level sandbox configuration.
///
/// Created via [`SandboxConfig::new`] and refined with builder methods.
/// Passed to [`crate::create_sandbox`] or [`crate::provider::SandboxProvider::init`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Absolute path to the project root directory.
    pub project_path: PathBuf,
    /// Which provider backend to use.
    pub provider: ProviderKind,
    /// Additional settings for the sandbox environment.
    pub settings: SandboxSettings,
}

impl SandboxConfig {
    /// Create a new config with the given project path and default settings.
    pub fn new(project_path: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            provider: ProviderKind::Local,
            settings: SandboxSettings::default(),
        }
    }

    /// Builder: set the provider kind.
    pub fn with_provider(mut self, provider: ProviderKind) -> Self {
        self.provider = provider;
        self
    }

    /// Builder: set the sandbox settings.
    pub fn with_settings(mut self, settings: SandboxSettings) -> Self {
        self.settings = settings;
        self
    }
}

/// Settings that control sandbox behaviour.
///
/// All fields have sensible defaults via [`Default`]. Use the builder methods
/// (`with_tool`, `with_env`, `with_dotenv`, `quiet`) to customise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSettings {
    /// Automatically install mise if not present on the system. Default: true.
    pub auto_install_mise: bool,
    /// Tools to ensure are available via mise (e.g. node@22, hugo@latest).
    pub mise_tools: Vec<MiseToolSpec>,
    /// Extra environment variables injected into every command.
    pub env: HashMap<String, String>,
    /// Optional .env file path (relative to project root) to load.
    pub dotenv: Option<PathBuf>,
    /// Suppress all progress indicators and status messages. Default: false.
    pub quiet: bool,
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            auto_install_mise: true,
            mise_tools: Vec::new(),
            env: HashMap::new(),
            dotenv: None,
            quiet: false,
        }
    }
}

impl SandboxSettings {
    /// Builder: add a mise tool spec.
    pub fn with_tool(mut self, name: impl Into<String>, version: Option<impl Into<String>>) -> Self {
        self.mise_tools.push(MiseToolSpec {
            name: name.into(),
            version: version.map(|v| v.into()),
        });
        self
    }

    /// Builder: add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Builder: set the dotenv file path.
    pub fn with_dotenv(mut self, path: impl Into<PathBuf>) -> Self {
        self.dotenv = Some(path.into());
        self
    }

    /// Builder: suppress all progress indicators and status messages.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }
}

/// Specification for a tool managed by mise.
///
/// Represents a `name@version` pair. When `version` is `None`, mise uses
/// `latest`. Common tool names: `node`, `bun`, `hugo`, `python`, `go`.
///
/// ```rust
/// use sandbox::MiseToolSpec;
///
/// let node = MiseToolSpec::new("node").with_version("22");
/// assert_eq!(node.to_mise_arg(), "node@22");
///
/// let bun = MiseToolSpec::new("bun"); // latest
/// assert_eq!(bun.to_mise_arg(), "bun@latest");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiseToolSpec {
    /// Tool name (e.g. "node", "hugo", "bun").
    pub name: String,
    /// Version requirement (e.g. "22", "latest", "0.83.0"). None means latest.
    pub version: Option<String>,
}

impl MiseToolSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Format as "name@version" for mise commands.
    pub fn to_mise_arg(&self) -> String {
        match &self.version {
            Some(v) => format!("{}@{}", self.name, v),
            None => format!("{}@latest", self.name),
        }
    }
}

/// The kind of sandbox provider backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderKind {
    /// Local filesystem-based sandbox.
    Local,
    /// Docker container-based sandbox (future).
    Docker,
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::Local
    }
}
