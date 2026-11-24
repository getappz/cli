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

/// Outcome summary of a best-effort source backup run after deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSummary {
    pub snapshot: Option<String>,
    pub added_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
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
    /// Best-effort source backup summary produced after a successful deploy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupSummary>,
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
            backup: None,
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
    #[serde(default, deserialize_with = "deserialize_timestamp_or_rfc3339")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_current: bool,
}

/// Filters for the team-wide `GET /v0/deployments` listing.
#[derive(Debug, Clone, Default)]
pub struct DeploymentQuery {
    /// Project id or slug; `None` lists the whole team.
    pub project: Option<String>,
    pub production: bool,
    pub environment: Option<String>,
    /// Statuses to match (`ready`, `building`, `error`, `canceled`), sent as repeated params.
    pub status: Vec<String>,
    /// Only deployments created at or after this time (epoch milliseconds).
    pub since: Option<i64>,
    /// Only deployments created at or before this time (epoch milliseconds).
    pub until: Option<i64>,
    /// `key=value` pairs matched against deployment metadata.
    pub meta: Vec<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// One organization audit event from `GET /v0/activity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: String,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub actor_user_id: Option<String>,
    #[serde(default)]
    pub actor_key_id: Option<String>,
    #[serde(default)]
    pub actor_type: Option<String>,
    pub action: String,
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub created_at: Option<f64>,
}

/// One page of audit events (`{ items, nextCursor }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPage {
    pub items: Vec<ActivityEvent>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One resource row from `GET /v0/usage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub label: String,
    pub used: f64,
    pub limit: f64,
    #[serde(default)]
    pub detail: String,
}

/// Organization usage plus the paid features it is entitled to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub usage: Vec<UsageEntry>,
    pub entitlements: Vec<String>,
}

/// One page of a cursor-paginated listing (`{ items, next }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamDeployment {
    pub id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_slug: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub file_count: Option<u64>,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
    #[serde(default)]
    pub production_url: Option<String>,
    #[serde(default)]
    pub is_current: bool,
}

/// Filters for the team-wide `GET /v0/domains` listing.
#[derive(Debug, Clone, Default)]
pub struct DomainQuery {
    pub project: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// A host serving a project, as returned by `GET /v0/projects/{id}/aliases`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasInfo {
    pub host: String,
    pub deployment_id: String,
    pub version_id: String,
    /// Creation time in epoch milliseconds.
    pub created_at: i64,
    pub is_production: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamDomain {
    #[serde(flatten)]
    pub domain: ProjectDomain,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_slug: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
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

pub(crate) fn deserialize_timestamp_or_rfc3339<'de, D>(
    deserializer: D,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Visitor;

    struct TimestampOrRfc3339Visitor;

    impl<'de> Visitor<'de> for TimestampOrRfc3339Visitor {
        type Value = Option<chrono::DateTime<chrono::Utc>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a timestamp integer in milliseconds or an RFC 3339 string")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(chrono::DateTime::from_timestamp_millis(v))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(chrono::DateTime::from_timestamp_millis(v as i64))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            chrono::DateTime::parse_from_rfc3339(v)
                .map(|dt| Some(dt.with_timezone(&chrono::Utc)))
                .map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_option(TimestampOrRfc3339Visitor)
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
    #[serde(default, alias = "type")]
    pub target: String,
    #[serde(
        default,
        alias = "created_at",
        deserialize_with = "deserialize_timestamp_or_rfc3339"
    )]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, alias = "productionUrl", alias = "production_url")]
    pub production_url: Option<String>,
    #[serde(default, alias = "isCurrent", alias = "is_current")]
    pub is_current: Option<bool>,
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

/// Result of promoting a ready deployment to production.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteResult {
    pub provider: String,
    pub project_id: String,
    pub deployment_id: String,
    pub version_id: String,
    /// Production URL serving the promoted deployment.
    pub url: String,
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
pub struct DnsRecordChallenge {
    pub r#type: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnameRecord {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApexRecord {
    pub name: String,
    pub ip: String,
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
    #[serde(default)]
    pub id: Option<String>,
    pub domain: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, alias = "ssl_status")]
    pub ssl_status: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub challenge: Option<DnsRecordChallenge>,
    #[serde(default)]
    pub cname: Option<CnameRecord>,
    #[serde(default)]
    pub apex: Option<ApexRecord>,
    #[serde(default)]
    pub verification: Vec<DomainVerificationRecord>,
    #[serde(default, alias = "created_at")]
    pub created_at: Option<i64>,
    /// When the domain's DNS stopped resolving to Appz, if it has lapsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapsed_at: Option<String>,
    /// When a lapsed domain will be detached automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removes_at: Option<String>,
}

/// Result of `POST /v0/projects/{id}/domains/{domain}/move`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainMove {
    #[serde(default)]
    pub id: Option<String>,
    pub domain: String,
    pub project_id: String,
    pub organization_id: String,
    /// `committed` / `completed`, or `pending` when routing reconciliation
    /// (HTTP 202) had not finished after retries.
    pub status: String,
}

impl ProjectDomain {
    /// Whether traffic for this domain is currently being served.
    pub fn serving_status(&self) -> &'static str {
        if self.lapsed_at.is_some() {
            "lapsed (DNS no longer points at Appz)"
        } else if !self.verified {
            "not serving (verification pending)"
        } else if self.ssl_status.as_deref().is_some_and(|s| s != "active") {
            "verified, TLS certificate pending"
        } else {
            "serving"
        }
    }

    pub fn display_status(&self) -> &str {
        if self.verified {
            self.ssl_status.as_deref().unwrap_or("active")
        } else if let Some(s) = &self.status {
            s.as_str()
        } else {
            "pending_verification"
        }
    }

    /// Appz routing guidance for responses containing only ownership records.
    pub fn dns_setup_hint(&self) -> Option<String> {
        if self.cname.is_some()
            || self.apex.is_some()
            || self
                .verification
                .iter()
                .any(|v| !v.r#type.eq_ignore_ascii_case("TXT"))
        {
            return None;
        }
        let domain = self.domain.trim_end_matches('.');
        Some(format!(
            "If {domain} is below your DNS zone apex, set its CNAME to cname.appz.to.\n\
             If {domain} is your DNS zone apex, add www.{domain} to this project, set its CNAME to \
             cname.appz.to, and verify it. Point {domain} to your redirect service using the DNS \
             records it provides, enable TLS for {domain} there, and redirect to https://www.{domain}"
        ))
    }
}

pub type DomainVerifyResult = ProjectDomain;

#[cfg(test)]
mod dns_hint_tests {
    use super::ProjectDomain;
    use serde_json::json;

    #[test]
    fn api_targets_take_precedence() {
        for (field, records) in [
            (
                "cname",
                json!({"name":"www.example.com","target":"custom.example.net"}),
            ),
            ("apex", json!({"name":"example.com","ip":"192.0.2.1"})),
            (
                "verification",
                json!([{"type":"CNAME","domain":"www.example.com","value":"custom.example.net"}]),
            ),
        ] {
            let dom: ProjectDomain =
                serde_json::from_value(json!({"domain":"example.com", (field):records})).unwrap();
            assert!(dom.dns_setup_hint().is_none(), "{field}");
        }
    }

    #[test]
    fn missing_targets_get_subdomain_and_apex_instructions() {
        for verification in [
            json!([]),
            json!([{"type":"txt","domain":"example.co.uk","value":"proof"}]),
        ] {
            let dom: ProjectDomain = serde_json::from_value(json!({
                "domain":"example.co.uk.", "verification":verification
            }))
            .unwrap();
            let hint = dom.dns_setup_hint().unwrap();
            assert!(hint.contains(
                "If example.co.uk is below your DNS zone apex, set its CNAME to cname.appz.to"
            ));
            assert!(hint.contains("add www.example.co.uk to this project"));
            assert!(hint.contains("Point example.co.uk to your redirect service"));
            assert!(hint.contains("enable TLS for example.co.uk there"));
            assert!(hint.ends_with("redirect to https://www.example.co.uk"));
        }
    }
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
    #[serde(default, alias = "production_url")]
    pub production_url: Option<String>,
}
