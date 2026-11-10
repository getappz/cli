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

/// Result of the provider CLI's "am I signed in" probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Authenticated,
    NotAuthenticated,
    /// The probe could not run or timed out; proceed and let the CLI report.
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
    /// Sign-in state; only filled by `--dry-run`, as a readiness report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthState>,
    /// Platform config files appz created (or, on `--dry-run`, would create)
    /// because the project had none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup_files: Vec<String>,
    /// `--dry-run` only: the platform CLI is not installed yet (a real run
    /// installs it through mise).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cli_missing: bool,
    /// `--dry-run` only: no platform project is linked yet (a real run
    /// creates or links one, with consent).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link_needed: bool,
    /// Nothing changed since the last deploy of this target, so nothing was
    /// (or, on `--dry-run`, would be) deployed; `url` is the previous one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged: bool,
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
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub id: String,
    #[serde(default)]
    pub version_id: Option<String>,
    pub url: Option<String>,
    pub status: DeployStatus,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectFileEntry {
    pub path: String,
    pub size: u64,
    pub hash: String,
    #[serde(default, alias = "content_type")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentInspect {
    pub id: String,
    #[serde(alias = "project_id")]
    pub project_id: String,
    #[serde(alias = "version_id")]
    pub version_id: String,
    pub status: DeployStatus,
    #[serde(default)]
    pub target: String,
    #[serde(default, alias = "created_at")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default, alias = "file_count")]
    pub file_count: usize,
    #[serde(default, alias = "total_bytes")]
    pub total_bytes: u64,
    #[serde(default, alias = "manifest_key")]
    pub manifest_key: Option<String>,
    #[serde(default)]
    pub redirects: Vec<String>,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub files: Option<Vec<InspectFileEntry>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainVerificationRecord {
    pub r#type: String,
    pub domain: String,
    pub value: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDomain {
    pub id: String,
    pub domain: String,
    pub status: String,
    #[serde(default, alias = "ssl_status")]
    pub ssl_status: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub verification: Vec<DomainVerificationRecord>,
    #[serde(default, alias = "created_at")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainVerifyResult {
    pub domain: String,
    pub status: String,
    #[serde(default, alias = "ssl_status")]
    pub ssl_status: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub verification: Vec<DomainVerificationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(
        default,
        alias = "organization_id",
        alias = "org_id",
        alias = "team_id"
    )]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default, alias = "created_at")]
    pub created_at: Option<i64>,
    #[serde(default, alias = "updated_at")]
    pub updated_at: Option<i64>,
}
