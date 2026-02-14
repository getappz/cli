//! Deployment output types.
//!
//! Structs representing the result of a deployment, preview, or other
//! provider operations. Used by all providers to return consistent results.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The result of a successful deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployOutput {
    /// The provider that was used (e.g. "vercel", "netlify").
    pub provider: String,

    /// The deployment URL (production or preview).
    pub url: String,

    /// Optional additional URLs (e.g. alias URLs, branch URLs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_urls: Vec<String>,

    /// Unique deployment identifier from the provider.
    pub deployment_id: Option<String>,

    /// Whether this is a preview or production deployment.
    pub is_preview: bool,

    /// Deployment status (e.g. "ready", "building", "queued").
    pub status: DeployStatus,

    /// When the deployment was created.
    pub created_at: Option<DateTime<Utc>>,

    /// How long the deployment took in milliseconds.
    pub duration_ms: Option<u64>,
}

/// Deployment status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Queued,
    Building,
    Deploying,
    Ready,
    Error,
    Cancelled,
}

impl std::fmt::Display for DeployStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployStatus::Queued => write!(f, "queued"),
            DeployStatus::Building => write!(f, "building"),
            DeployStatus::Deploying => write!(f, "deploying"),
            DeployStatus::Ready => write!(f, "ready"),
            DeployStatus::Error => write!(f, "error"),
            DeployStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Information about a past deployment (for listing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    /// Unique deployment identifier.
    pub id: String,

    /// Deployment URL.
    pub url: String,

    /// Deployment status.
    pub status: DeployStatus,

    /// When the deployment was created.
    pub created_at: Option<DateTime<Utc>>,

    /// Whether this is the current production deployment.
    pub is_current: bool,

    /// Optional commit SHA or ref that was deployed.
    pub git_ref: Option<String>,
}

/// Status of provider prerequisites (CLI tools, authentication, etc.).
#[derive(Debug, Clone)]
pub enum PrerequisiteStatus {
    /// All prerequisites are met.
    Ready,

    /// The provider CLI tool needs to be installed.
    CliMissing {
        tool: String,
        install_hint: String,
    },

    /// Authentication is required.
    AuthMissing {
        env_var: String,
        login_hint: String,
    },

    /// Multiple issues found.
    Multiple(Vec<PrerequisiteStatus>),
}

impl PrerequisiteStatus {
    /// Returns true if all prerequisites are satisfied.
    pub fn is_ready(&self) -> bool {
        matches!(self, PrerequisiteStatus::Ready)
    }
}

/// Configuration detected from existing platform files.
#[derive(Debug, Clone)]
pub struct DetectedConfig {
    /// The file that was detected (e.g. "vercel.json", "netlify.toml").
    pub config_file: String,

    /// Whether the project is already linked/connected to the platform.
    pub is_linked: bool,

    /// Optional project name extracted from the config.
    pub project_name: Option<String>,

    /// Optional team/account name extracted from the config.
    pub team: Option<String>,
}

/// Detected platform information (used by the platform detector).
#[derive(Debug, Clone)]
pub struct DetectedPlatform {
    /// Provider slug (e.g. "vercel", "netlify").
    pub slug: String,

    /// Human-readable provider name.
    pub name: String,

    /// The config file(s) that were detected.
    pub config_files: Vec<String>,

    /// Confidence level of the detection.
    pub confidence: DetectionConfidence,
}

/// How confident we are in a platform detection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectionConfidence {
    /// Found a platform-specific config file (e.g. vercel.json).
    Low,
    /// Found a linked project state file (e.g. .vercel/project.json).
    Medium,
    /// Configured in appz.json deploy targets.
    High,
}
