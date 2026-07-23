use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Ready,
    Building,
    Error,
    /// The deploy command exited 0 but we could not confirm a live URL —
    /// surfaced honestly instead of guessing one (see `DeployOutput::url`).
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployOutput {
    pub provider: String,
    /// The real deployment URL, parsed from the provider CLI's own output.
    /// `None` when the URL could not be confirmed — never a guessed/fabricated
    /// value (e.g. `https://{dirname}.vercel.app`).
    pub url: Option<String>,
    pub is_preview: bool,
    pub status: DeployStatus,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_ms: Option<u64>,
}

impl DeployOutput {
    pub fn dry_run(provider: &str, is_preview: bool) -> Self {
        Self {
            provider: provider.to_string(),
            url: None,
            is_preview,
            status: DeployStatus::Unknown,
            created_at: None,
            duration_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub id: String,
    pub url: Option<String>,
    pub status: DeployStatus,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedConfig {
    pub config_file: String,
    pub is_linked: bool,
    pub project_name: Option<String>,
    pub team: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrerequisiteStatus {
    Ready,
    CliMissing { tool: String, install_hint: String },
    AuthMissing { env_var: String, login_hint: String },
}
