//! AppzDev / Cloudflare Edge native deploy provider.
//!
//! Direct edge-native CAS deployment to the Cloudflare runtime (designflare/worker/serve).
//! Features:
//! - Multi-threaded SHA-256 Merkle tree calculation
//! - Delta-check: only missing hashes are uploaded (zero duplicate transfer)
//! - Bounded concurrency blob uploads with retries
//! - Atomic immutable deployment to R2 MANIFESTS
//! - Sub-second instant rollback via KV HOSTS pointer mutation

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::context::{DeployContext, read_markdown_config};
use crate::error::{DeployError, DeployResult};
use crate::fingerprint::prepare_upload;
use crate::markdown::{HttpMarkdownConverter, add_markdown_siblings};
use crate::output::{
    ActivityPage, DeployOutput, DeployStatus, DeploymentInfo, DeploymentInspect, DeploymentQuery,
    DetectedConfig, DomainMove, DomainQuery, DomainVerifyResult, Page, PrerequisiteStatus,
    ProjectDomain, ProjectInfo, PromoteResult, TeamDeployment, TeamDomain, UsageReport,
};
use crate::provider::DeployProvider;
use crate::redirects::{RedirectInput, RedirectList, RedirectVersionRef, RedirectVersions};
use crate::static_cas::ScannedFileEntry;
use projects::{ProjectState, Transfer};

pub struct AppzdevProvider;

mod interrupt;
pub mod projects;
pub use interrupt::enable_cli_deploy_interrupts;

/// Body of `POST /v0/deployments`. `flags` is sent only when a feature is
/// on, so deploys without opt-ins keep the exact pre-existing payload.
pub(crate) fn negotiate_payload(
    project_id: &str,
    target: &str,
    files: &[ScannedFileEntry],
    markdown: bool,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "projectId": project_id,
        "target": target,
        "files": files,
    });
    if markdown {
        payload["flags"] = serde_json::json!({ "markdown": true });
    }
    payload
}

const DEPLOY_USER_AGENT: &str = concat!("appz-cli/", env!("CARGO_PKG_VERSION"));
const DEFAULT_API_URL: &str = "https://api.appz.dev";

/// Normalizes a project id or name into a valid edge project slug:
/// - Values starting with `prj_` are preserved untouched.
/// - Otherwise: lowercase, replace non `[a-z0-9]` runs with `-`, trim `-`, cap at 40 chars.
/// - Falls back to `default-project` if empty.
pub fn slugify_project_id(raw: &str) -> String {
    if raw.starts_with("prj_") {
        return raw.to_string();
    }
    slugify(raw)
}

fn slugify(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(40));
    let mut last_dash = true; // suppress leading dash
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() {
            if out.len() >= 40 {
                break;
            }
            out.push(b.to_ascii_lowercase() as char);
            last_dash = false;
        } else if !last_dash {
            if out.len() >= 40 {
                break;
            }
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "default-project".to_string()
    } else {
        out
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredAuth {
    pub token: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
}

pub fn auth_file_path() -> PathBuf {
    if let Ok(p) = std::env::var("APPZ_VAULT_HOME_OVERRIDE") {
        return PathBuf::from(p).join(".appz").join("auth.json");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".appz")
        .join("auth.json")
}

pub fn load_stored_auth() -> Option<StoredAuth> {
    let p = auth_file_path();
    let content = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_stored_auth(auth: &StoredAuth) -> Result<(), std::io::Error> {
    use std::io::Write;
    let p = auth_file_path();
    let parent = p.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let data = serde_json::to_string_pretty(auth).map_err(std::io::Error::other)?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(data.as_bytes())?;
    tmp.as_file().sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(windows)]
    restrict_auth_file_to_current_user(tmp.path());

    tmp.persist(&p).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(windows)]
fn restrict_auth_file_to_current_user(path: &Path) {
    let Some(user) = std::env::var_os("USERNAME") else {
        return;
    };
    let mut grant = user;
    grant.push(":(F)");
    let _ = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(grant)
        .output();
}

pub fn clear_stored_auth() -> Result<(), std::io::Error> {
    let p = auth_file_path();
    if p.is_file() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

pub fn resolve_auth_token(ctx: Option<&DeployContext>) -> Option<String> {
    std::env::var("APPZ_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .or_else(|| {
            ctx.and_then(|c| c.run_env.get("APPZ_TOKEN").cloned())
                .filter(|t| !t.is_empty())
        })
        .or_else(|| load_stored_auth().map(|a| a.token))
}

/// POST the promote request, retrying transport/5xx failures with one
/// Idempotency-Key so a retry cannot promote twice.
fn post_promote(
    url: &str,
    token: Option<&str>,
    deployment_id: &str,
) -> Result<serde_json::Value, Box<ureq::Error>> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let key = format!("appz-promote-{deployment_id}-{nanos:x}");
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut req = ureq::post(url).set("Idempotency-Key", &key);
        if let Some(token) = token {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        match req.send_json(serde_json::json!({ "deploymentId": deployment_id })) {
            Ok(resp) => return resp.into_json().map_err(|e| Box::new(ureq::Error::from(e))),
            Err(e) => {
                let retryable = match &e {
                    ureq::Error::Status(code, _) => *code >= 500,
                    ureq::Error::Transport(_) => true,
                };
                if !retryable || attempt >= 3 {
                    return Err(Box::new(e));
                }
                std::thread::sleep(Duration::from_millis(300 * attempt));
            }
        }
    }
}

pub fn parse_api_error(
    err: ureq::Error,
    provider_name: &'static str,
    operation: &str,
) -> DeployError {
    match err {
        ureq::Error::Status(code, resp) => {
            if code == 401 {
                // A session still waiting on its 2FA code is not a missing login.
                if let Ok(json) = resp.into_json::<serde_json::Value>()
                    && json.get("code").and_then(|c| c.as_str()) == Some("MFA_REQUIRED")
                {
                    let reason = json.get("error").and_then(|e| e.as_str()).unwrap_or(
                        "two-factor verification required; finish verification in your browser",
                    );
                    return DeployError::Api {
                        provider: provider_name,
                        reason: reason.to_string(),
                    };
                }
                return DeployError::AuthMissing {
                    provider: provider_name,
                    env_var: "APPZ_TOKEN",
                    login_hint: "run `appz login` or set APPZ_TOKEN".to_string(),
                };
            }
            if code == 409 && operation.contains("Verify") {
                return DeployError::Api {
                    provider: provider_name,
                    reason: "DNS ownership TXT record not propagated yet. Please ensure the DNS challenge TXT record is configured and wait a few minutes before re-checking.".into(),
                };
            }
            if code == 503 && operation.contains("domain") {
                return DeployError::Api {
                    provider: provider_name,
                    reason: "Custom domains are not configured on the server.".into(),
                };
            }
            if code == 502 && operation.contains("domain") {
                return DeployError::Api {
                    provider: provider_name,
                    reason: "Cloudflare upstream failure; verification was recorded, please retry shortly.".into(),
                };
            }
            if let Ok(json) = resp.into_json::<serde_json::Value>() {
                let mut msg = String::new();
                if let Some(err_str) = json.get("error").and_then(|e| e.as_str()) {
                    msg.push_str(err_str);
                } else {
                    msg.push_str(&format!("HTTP {code}"));
                }
                if let Some(issues) = json.get("issues").and_then(|i| i.as_array()) {
                    for issue in issues {
                        let path = issue.get("path").and_then(|p| p.as_str()).unwrap_or("");
                        let message = issue.get("message").and_then(|m| m.as_str()).unwrap_or("");
                        if !path.is_empty() || !message.is_empty() {
                            msg.push_str(&format!("\n  - {path}: {message}"));
                        }
                    }
                }
                if let Some(missing) = json.get("missing").and_then(|m| m.as_array()) {
                    msg.push_str(&format!(" (missing {} blobs)", missing.len()));
                }
                DeployError::Api {
                    provider: provider_name,
                    reason: format!("{operation} failed ({code}): {msg}"),
                }
            } else {
                DeployError::Api {
                    provider: provider_name,
                    reason: format!("{operation} failed with HTTP status {code}"),
                }
            }
        }
        ureq::Error::Transport(e) => DeployError::Api {
            provider: provider_name,
            reason: format!("{operation} network error: {e}"),
        },
    }
}

pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub interval: u64,
    pub expires_in: u64,
}

pub fn request_device_code(api_base: &str) -> DeployResult<DeviceCodeResponse> {
    let url = format!("{api_base}/api/auth/device/code");
    let resp: serde_json::Value = ureq::post(&url)
        .send_json(serde_json::json!({ "client_id": "appz-cli" }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Request device code"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid device code response: {e}"),
        })?;

    Ok(DeviceCodeResponse {
        device_code: resp["device_code"].as_str().unwrap_or_default().to_string(),
        user_code: resp["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: resp["verification_uri"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        verification_uri_complete: resp["verification_uri_complete"].as_str().map(String::from),
        interval: resp["interval"].as_u64().unwrap_or(5),
        expires_in: resp["expires_in"].as_u64().unwrap_or(900),
    })
}

pub enum DevicePollResult {
    Success(String),
    Pending,
    SlowDown,
    Failed(String),
}

pub fn poll_device_token(api_base: &str, device_code: &str) -> DevicePollResult {
    let url = format!("{api_base}/api/auth/device/token");
    let res = ureq::post(&url).send_json(serde_json::json!({
        "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        "device_code": device_code,
        "client_id": "appz-cli",
    }));

    match res {
        Ok(resp) => {
            if let Some(tok) = resp.into_json::<serde_json::Value>().ok().and_then(|v| {
                v.get("access_token")
                    .and_then(|s| s.as_str())
                    .map(String::from)
            }) {
                DevicePollResult::Success(tok)
            } else {
                DevicePollResult::Failed("Missing access_token in token response".into())
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            if let Ok(v) = resp.into_json::<serde_json::Value>() {
                let err = v.get("error").and_then(|s| s.as_str()).unwrap_or("");
                match err {
                    "authorization_pending" => DevicePollResult::Pending,
                    "slow_down" => DevicePollResult::SlowDown,
                    "access_denied" => DevicePollResult::Failed("Access denied by user".into()),
                    "expired_token" => DevicePollResult::Failed("Device code expired".into()),
                    _ => DevicePollResult::Failed(format!("Device token failed ({code}): {err}")),
                }
            } else {
                DevicePollResult::Failed(format!("HTTP {code} from device token endpoint"))
            }
        }
        Err(e) => DevicePollResult::Failed(format!("Network error: {e}")),
    }
}

pub fn resolve_project_id_from_dir(dir: &Path) -> String {
    let project_json = dir.join(".appz").join("project.json");
    if let Some(id) = std::fs::read_to_string(&project_json)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| {
            v.get("projectId")
                .and_then(|s| s.as_str())
                .map(String::from)
        })
    {
        return slugify_project_id(&id);
    }
    if let Some(id) = std::env::var("APPZ_PROJECT_ID")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return slugify_project_id(&id);
    }
    // `.` / `..` / a trailing separator have no file name until resolved.
    let folder = dir
        .file_name()
        .map(|s| s.to_os_string())
        .or_else(|| {
            dir.canonicalize()
                .ok()?
                .file_name()
                .map(|s| s.to_os_string())
        })
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    folder_slug(&folder)
}

/// Turns a folder name into a project slug:
/// - transliterates non-ASCII (`Café` -> `cafe`, `रवि` -> `rvi`),
/// - drops copy/download markers (`site (1)`, `site - Copy`, `site copy 2`,
///   GitHub zip `site-main` / `site-master`),
/// - strips a domain suffix (`rksolar.co.in` -> `rksolar`),
/// - drops apostrophes (`John's Site` -> `johns-site`),
/// - then slugifies. A folder named `prj_*` is slugified like any other name.
fn folder_slug(folder: &str) -> String {
    static COPY_MARKER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?:\s*\(\d+\)|[\s_-]+(?:-\s*)?copy(?:\s*\d+)?|-(?:main|master))$")
            .expect("valid regex")
    });
    let mut name = deunicode::deunicode(folder).to_ascii_lowercase();
    loop {
        let trimmed = name.trim();
        let stripped = COPY_MARKER.replace(trimmed, "");
        if stripped.is_empty() || stripped == trimmed {
            name = trimmed.to_string();
            break;
        }
        name = stripped.into_owned();
    }
    slugify(&strip_domain_suffix(&name).replace('\'', ""))
}

/// Strips a leading `www.` and a Public Suffix List suffix from a lowercase
/// folder name that looks like a domain, so `rksolar.co.in`, `rksolar.in`
/// and `www.rksolar.com` all map to `rksolar`. Unknown endings (`next.js`)
/// and non-domain names are returned unchanged.
fn strip_domain_suffix(lower: &str) -> String {
    let s = lower.strip_prefix("www.").unwrap_or(lower);
    match psl2::lookup(s) {
        Some(d) if d.is_known() && !d.is_public_suffix() => {
            let host = d.as_str();
            host[..host.len() - d.suffix().len() - 1].to_string()
        }
        _ => s.to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudEnvVar {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub key: String,
    pub value: String,
    #[serde(default = "default_targets")]
    pub target: Vec<String>,
    #[serde(default, alias = "gitBranch", skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default, alias = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

/// Key and metadata of a cloud variable. The list endpoint never returns values.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudEnvMeta {
    pub id: String,
    pub key: String,
    pub target: Vec<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub created_at: Option<f64>,
}

/// A custom project environment (in addition to production/preview/development).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEnvironment {
    pub id: String,
    pub slug: String,
    #[serde(default)]
    pub branch_pattern: Option<String>,
}

/// Whether `slug` is a valid environment name: `^[a-z0-9](?:[a-z0-9-]{0,38}[a-z0-9])?$`.
pub fn is_valid_environment_slug(slug: &str) -> bool {
    let b = slug.as_bytes();
    let alnum = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty()
        && b.len() <= 40
        && b.iter().all(|c| alnum(c) || *c == b'-')
        && alnum(&b[0])
        && alnum(&b[b.len() - 1])
}

#[derive(serde::Serialize)]
struct EnvPushItem<'a> {
    key: &'a str,
    value: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    target: &'a [String],
}

fn default_targets() -> Vec<String> {
    vec!["production".to_string(), "preview".to_string()]
}

pub fn parse_env_file(content: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let line_clean = trimmed.trim_start_matches("export ").trim();
        if let Some((k, v)) = line_clean.split_once('=') {
            let key = k.trim();
            let mut val = v.trim();
            let is_quoted = (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''));
            if is_quoted && val.len() >= 2 {
                val = &val[1..val.len() - 1];
            }
            if !key.is_empty() {
                out.push((key.to_string(), val.to_string()));
            }
        }
    }
    out
}

fn is_valid_env_key(key: &str) -> bool {
    !key.is_empty()
        && !key
            .chars()
            .any(|c| c == '=' || c == '\r' || c == '\n' || c == '\0' || c.is_ascii_whitespace())
}

pub fn format_env_file(vars: &[(String, String)]) -> String {
    let cap = vars.iter().map(|(k, v)| k.len() + v.len() + 4).sum();
    let mut out = String::with_capacity(cap);
    for (k, v) in vars {
        if !is_valid_env_key(k) {
            continue;
        }
        out.push_str(k);
        out.push('=');
        if v.contains([' ', '\t', '\n', '\r', '"']) || v.is_empty() {
            out.push('"');
            for c in v.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    _ => out.push(c),
                }
            }
            out.push('"');
        } else {
            out.push_str(v);
        }
        out.push('\n');
    }
    out
}

fn project_env_url(api_base: &str, project_id: &str, path: &str, target: Option<&str>) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let encoded_project = utf8_percent_encode(project_id, NON_ALPHANUMERIC);
    let mut url = format!("{api_base}/v0/projects/{encoded_project}{path}");
    if let Some(t) = target {
        let encoded_target = utf8_percent_encode(t, NON_ALPHANUMERIC);
        url.push_str(&format!("?target={encoded_target}"));
    }
    url
}

fn get_env_array<T: serde::de::DeserializeOwned>(
    url: &str,
    token: Option<&str>,
    operation: &str,
) -> DeployResult<Vec<T>> {
    let mut req = ureq::get(url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", operation))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from {operation}: {e}"),
        })?;
    let items = if let Some(arr) = resp.as_array() {
        arr
    } else if let Some(arr) = resp["env"].as_array() {
        arr
    } else {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: "Expected array in response".into(),
        });
    };
    serde_json::from_value(serde_json::Value::Array(items.clone())).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse response: {e}"),
    })
}

/// Keys and metadata for `target` (`GET /env`); never returns values.
pub fn list_cloud_env(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    target: Option<&str>,
) -> DeployResult<Vec<CloudEnvMeta>> {
    let url = project_env_url(api_base, project_id, "/env", target);
    get_env_array(&url, token, "List environment variables")
}

/// Custom environments of a project.
pub fn list_project_environments(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
) -> DeployResult<Vec<ProjectEnvironment>> {
    let url = project_env_url(api_base, project_id, "/environments", None);
    get_env_array(&url, token, "List environments")
}

pub fn add_project_environment(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    slug: &str,
    branch_pattern: Option<&str>,
) -> DeployResult<ProjectEnvironment> {
    if !is_valid_environment_slug(slug) {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: format!(
                "Invalid environment name {slug:?}: use lowercase letters, digits and '-' (max 40)"
            ),
        });
    }
    let url = project_env_url(api_base, project_id, "/environments", None);
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    req.send_json(serde_json::json!({ "slug": slug, "branchPattern": branch_pattern }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Create environment"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from create environment: {e}"),
        })
}

pub fn remove_project_environment(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    slug: &str,
) -> DeployResult<()> {
    if !is_valid_environment_slug(slug) {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid environment name {slug:?}"),
        });
    }
    let url = project_env_url(api_base, project_id, &format!("/environments/{slug}"), None);
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    req.call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Remove environment"))?;
    Ok(())
}

/// Decrypted variables for `target` (`GET /env/values`, needs the `env:decrypt` scope).
pub fn pull_cloud_env(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    target: Option<&str>,
) -> DeployResult<Vec<CloudEnvVar>> {
    let url = project_env_url(api_base, project_id, "/env/values", target);
    let vars: Vec<CloudEnvVar> = get_env_array(&url, token, "Pull environment variables")?;

    for var in &vars {
        if !is_valid_env_key(&var.key) {
            return Err(DeployError::Api {
                provider: "AppzDev",
                reason: format!("Invalid environment variable key received: {:?}", var.key),
            });
        }
    }

    Ok(vars)
}

pub fn push_cloud_env(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    vars: &[CloudEnvVar],
) -> DeployResult<usize> {
    let url = format!("{api_base}/v0/projects/{project_id}/env");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let payload_items: Vec<EnvPushItem> = vars
        .iter()
        .map(|v| EnvPushItem {
            key: &v.key,
            value: &v.value,
            target: &v.target,
        })
        .collect();

    let resp: serde_json::Value = req
        .send_json(serde_json::json!({ "env": payload_items }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Push environment variables"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from push env: {e}"),
        })?;

    let count = resp
        .get("count")
        .and_then(|c| c.as_u64())
        .map(|c| c as usize)
        .unwrap_or(vars.len());

    Ok(count)
}

pub fn delete_cloud_env_var(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    key: &str,
) -> DeployResult<bool> {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    let key = utf8_percent_encode(key, NON_ALPHANUMERIC);
    let url = project_env_url(api_base, project_id, &format!("/env/{key}"), None);
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let _ = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Delete environment variable"))?;

    Ok(true)
}

/// Trim/lowercase a hostname and reject anything that would alter the URL path
/// when interpolated into `/domains/{domain}` (slashes, queries, whitespace, ...).
fn normalize_domain(domain: &str) -> DeployResult<String> {
    let d = domain.trim().trim_end_matches('.');
    if d.is_empty() {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid domain name: {domain:?}"),
        });
    }
    let mut out = String::with_capacity(d.len());
    for c in d.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '*' {
            out.push(c.to_ascii_lowercase());
        } else {
            return Err(DeployError::Api {
                provider: "AppzDev",
                reason: format!("Invalid domain name: {domain:?}"),
            });
        }
    }
    Ok(out)
}

pub fn add_project_domain(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<ProjectDomain> {
    let domain = normalize_domain(domain)?;
    let domain = domain.as_str();
    let url = format!("{api_base}/v0/projects/{project_id}/domains");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({ "domain": domain }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Add custom domain"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from add domain: {e}"),
        })?;

    serde_json::from_value(resp).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse added domain record: {e}"),
    })
}

pub fn list_project_domains(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
) -> DeployResult<Vec<ProjectDomain>> {
    let url = format!("{api_base}/v0/projects/{project_id}/domains");
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "List custom domains"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from list domains: {e}"),
        })?;

    let items = if resp.is_array() {
        resp
    } else if let Some(arr) = resp.get("domains").filter(|v| v.is_array()) {
        arr.clone()
    } else {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: "Expected array of domain records".into(),
        });
    };

    serde_json::from_value(items).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse domain list: {e}"),
    })
}

/// GET `{api_base}{path}` with query params, decoded as a `{ items, next }` page.
fn get_page<T: serde::de::DeserializeOwned>(
    api_base: &str,
    path: &str,
    token: Option<&str>,
    params: &[(&str, String)],
    operation: &str,
) -> DeployResult<Page<T>> {
    let mut req = ureq::get(&format!("{api_base}{path}"));
    for (k, v) in params {
        req = req.query(k, v);
    }
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    req.call()
        .map_err(|e| parse_api_error(e, "AppzDev", operation))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid response from {operation}: {e}"),
        })
}

/// Query-string pairs for `GET /v0/deployments`.
fn deployment_params(query: &DeploymentQuery) -> DeployResult<Vec<(String, String)>> {
    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(p) = &query.project {
        params.push(("projectId".into(), p.clone()));
    }
    if query.production {
        params.push(("type".into(), "production".into()));
    }
    if let Some(e) = &query.environment {
        params.push(("environment".into(), e.clone()));
    }
    for s in &query.status {
        params.push(("status".into(), s.clone()));
    }
    if let Some(t) = query.since {
        params.push(("since".into(), t.to_string()));
    }
    if let Some(t) = query.until {
        params.push(("until".into(), t.to_string()));
    }
    for kv in &query.meta {
        let (k, v) = kv.split_once('=').ok_or_else(|| DeployError::Api {
            provider: "AppzDev",
            reason: format!("invalid --meta '{kv}': expected key=value"),
        })?;
        params.push((format!("meta.{k}"), v.to_string()));
    }
    if let Some(l) = query.limit {
        params.push(("limit".into(), l.to_string()));
    }
    if let Some(c) = &query.cursor {
        params.push(("cursor".into(), c.clone()));
    }
    Ok(params)
}

pub fn list_team_deployments(
    api_base: &str,
    token: Option<&str>,
    query: &DeploymentQuery,
) -> DeployResult<Page<TeamDeployment>> {
    let params = deployment_params(query)?;
    let params: Vec<(&str, String)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();
    get_page(
        api_base,
        "/v0/deployments",
        token,
        &params,
        "List deployments",
    )
}

/// GET `/v0/activity`. A 403 means the caller is neither an owner session nor
/// an org key with `activity:read`, so say that instead of a bare status.
pub fn list_activity(
    api_base: &str,
    token: Option<&str>,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> DeployResult<ActivityPage> {
    let mut req = ureq::get(&format!("{api_base}/v0/activity"));
    if let Some(l) = limit {
        req = req.query("limit", &l.to_string());
    }
    if let Some(c) = cursor {
        req = req.query("cursor", c);
    }
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    req.call()
        .map_err(|e| match e {
            ureq::Error::Status(403, _) => DeployError::Api {
                provider: "AppzDev",
                reason: "Activity needs an organization owner session or an API key with the `activity:read` scope".into(),
            },
            e => parse_api_error(e, "AppzDev", "List activity"),
        })?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid response from List activity: {e}"),
        })
}

/// GET `/v0/usage` and `/v0/entitlements`, combined.
pub fn get_usage_report(api_base: &str, token: Option<&str>) -> DeployResult<UsageReport> {
    fn get<T: serde::de::DeserializeOwned>(
        api_base: &str,
        path: &str,
        token: Option<&str>,
        operation: &str,
    ) -> DeployResult<T> {
        let mut req = ureq::get(&format!("{api_base}{path}"));
        if let Some(tok) = token {
            req = req.set("Authorization", &format!("Bearer {tok}"));
        }
        req.call()
            .map_err(|e| parse_api_error(e, "AppzDev", operation))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: "AppzDev",
                reason: format!("Invalid response from {operation}: {e}"),
            })
    }
    #[derive(serde::Deserialize)]
    struct Entitlements {
        entitlements: Vec<String>,
    }
    Ok(UsageReport {
        usage: get(api_base, "/v0/usage", token, "Get usage")?,
        entitlements: get::<Entitlements>(api_base, "/v0/entitlements", token, "Get entitlements")?
            .entitlements,
    })
}

pub fn list_team_domains(
    api_base: &str,
    token: Option<&str>,
    query: &DomainQuery,
) -> DeployResult<Page<TeamDomain>> {
    let mut params: Vec<(&str, String)> = Vec::new();
    if let Some(p) = &query.project {
        params.push(("projectId", p.clone()));
    }
    if let Some(l) = query.limit {
        params.push(("limit", l.to_string()));
    }
    if let Some(c) = &query.cursor {
        params.push(("cursor", c.clone()));
    }
    get_page(api_base, "/v0/domains", token, &params, "List domains")
}

/// Find one domain in the team-wide listing (there is no single-domain GET).
pub fn inspect_team_domain(
    api_base: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<Option<TeamDomain>> {
    let domain = normalize_domain(domain)?;
    let mut cursor: Option<String> = None;
    loop {
        let page = list_team_domains(
            api_base,
            token,
            &DomainQuery {
                project: None,
                limit: Some(100),
                cursor: cursor.clone(),
            },
        )?;
        if let Some(found) = page.items.into_iter().find(|t| {
            t.domain
                .domain
                .trim_end_matches('.')
                .eq_ignore_ascii_case(&domain)
        }) {
            return Ok(Some(found));
        }
        match page.next {
            Some(next) if cursor.as_deref() != Some(next.as_str()) => cursor = Some(next),
            _ => return Ok(None),
        }
    }
}

/// Move a domain attachment to `project_id` in organization `org`.
/// Retries a 202 (routing reconciliation pending) with the same
/// Idempotency-Key, as the API asks, before reporting `pending`.
pub fn move_project_domain(
    api_base: &str,
    token: Option<&str>,
    from_project: &str,
    domain: &str,
    org: &str,
    project_id: &str,
) -> DeployResult<DomainMove> {
    let domain = normalize_domain(domain)?;
    let url = format!("{api_base}/v0/projects/{from_project}/domains/{domain}/move");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let seed = format!("{domain}:{org}:{project_id}:{nanos}:{}", std::process::id());
    let key = format!(
        "appz-move-{}",
        &blake3::hash(seed.as_bytes()).to_hex()[..32]
    );
    let body = serde_json::json!({ "organizationId": org, "projectId": project_id });
    let mut last = None;
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(if cfg!(test) { 10 } else { 1500 }));
        }
        let mut req = ureq::post(&url).set("Idempotency-Key", &key);
        if let Some(tok) = token {
            req = req.set("Authorization", &format!("Bearer {tok}"));
        }
        let resp = match req.send_json(body.clone()) {
            Ok(r) => r,
            Err(ureq::Error::Status(403, _)) => {
                return Err(DeployError::Api {
                    provider: "AppzDev",
                    reason: format!(
                        "not allowed to move {domain}: you must be an owner of both the current and the destination organization ({org})"
                    ),
                });
            }
            Err(e) => return Err(parse_api_error(e, "AppzDev", "Move custom domain")),
        };
        let accepted = resp.status() == 202;
        let parsed: DomainMove = resp.into_json().map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid response from move domain: {e}"),
        })?;
        if !accepted {
            return Ok(parsed);
        }
        last = Some(parsed);
    }
    let mut out = last.expect("loop ran at least once");
    out.status = "pending".into();
    Ok(out)
}

pub fn verify_project_domain(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<DomainVerifyResult> {
    let domain = normalize_domain(domain)?;
    let domain = domain.as_str();
    let url = format!("{api_base}/v0/projects/{project_id}/domains/{domain}/verify");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({}))
        .map_err(|e| parse_api_error(e, "AppzDev", "Verify custom domain"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from verify domain: {e}"),
        })?;

    serde_json::from_value(resp).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse domain verification result: {e}"),
    })
}

pub fn remove_project_domain(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<bool> {
    let domain = normalize_domain(domain)?;
    let domain = domain.as_str();
    let url = format!("{api_base}/v0/projects/{project_id}/domains/{domain}");
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let _ = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Remove custom domain"))?;

    Ok(true)
}

fn redirects_url(api_base: &str, project: &str, tail: &str) -> DeployResult<String> {
    validate_project_ref(project)?;
    Ok(format!("{api_base}/v0/projects/{project}/redirects{tail}"))
}

fn redirect_call<T: serde::de::DeserializeOwned>(
    mut req: ureq::Request,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    operation: &str,
) -> DeployResult<T> {
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    }
    .map_err(|e| parse_api_error(e, "AppzDev", operation))?;
    resp.into_json().map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Invalid response from {operation}: {e}"),
    })
}

pub fn list_project_redirects(
    api_base: &str,
    project: &str,
    token: Option<&str>,
    version: Option<u64>,
) -> DeployResult<RedirectList> {
    let mut req = ureq::get(&redirects_url(api_base, project, "")?);
    if let Some(v) = version {
        req = req.query("version", &v.to_string());
    }
    redirect_call(req, token, None, "List redirects")
}

pub fn add_project_redirect(
    api_base: &str,
    project: &str,
    token: Option<&str>,
    rule: &RedirectInput,
) -> DeployResult<RedirectList> {
    let req = ureq::post(&redirects_url(api_base, project, "")?);
    redirect_call(req, token, Some(serde_json::json!(rule)), "Add redirect")
}

pub fn remove_project_redirect(
    api_base: &str,
    project: &str,
    token: Option<&str>,
    id: &str,
) -> DeployResult<RedirectList> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: "Expected a redirect ID".into(),
        });
    }
    let req = ureq::delete(&redirects_url(api_base, project, &format!("/{id}"))?);
    redirect_call(req, token, None, "Remove redirect")
}

pub fn import_project_redirects(
    api_base: &str,
    project: &str,
    token: Option<&str>,
    rules: &[RedirectInput],
) -> DeployResult<RedirectList> {
    let req = ureq::post(&redirects_url(api_base, project, "/import")?);
    redirect_call(
        req,
        token,
        Some(serde_json::json!(rules)),
        "Import redirects",
    )
}

pub fn list_project_redirect_versions(
    api_base: &str,
    project: &str,
    token: Option<&str>,
) -> DeployResult<RedirectVersions> {
    let req = ureq::get(&redirects_url(api_base, project, "/versions")?);
    redirect_call(req, token, None, "List redirect versions")
}

pub fn update_project_redirect_version(
    api_base: &str,
    project: &str,
    token: Option<&str>,
    version: u64,
    action: &str,
) -> DeployResult<RedirectVersionRef> {
    let (action, operation) = if action == "restore" {
        ("restore", "Restore redirect version")
    } else {
        ("promote", "Promote redirect version")
    };
    let req = ureq::post(&redirects_url(
        api_base,
        project,
        &format!("/versions/{version}/{action}"),
    )?);
    redirect_call(req, token, Some(serde_json::json!({})), operation)
}

pub fn list_project_aliases(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
) -> DeployResult<Vec<crate::AliasInfo>> {
    validate_project_ref(project_id)?;
    let mut req = ureq::get(&format!("{api_base}/v0/projects/{project_id}/aliases"));
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    req.call()
        .map_err(|e| parse_api_error(e, "AppzDev", "List aliases"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid aliases response: {e}"),
        })
}

pub fn remove_project_alias(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    host: &str,
) -> DeployResult<()> {
    validate_project_ref(project_id)?;
    let host = normalize_domain(host)?;
    let mut req = ureq::delete(&format!(
        "{api_base}/v0/projects/{project_id}/aliases/{host}"
    ));
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    match req.call() {
        Ok(_) => Ok(()),
        // The server refuses to remove a project's production host.
        Err(ureq::Error::Status(409, resp)) => {
            let detail = resp
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|j| j.get("error").and_then(|e| e.as_str().map(String::from)))
                .unwrap_or_else(|| "conflict".into());
            Err(DeployError::Api {
                provider: "AppzDev",
                reason: format!("{host} cannot be removed: {detail}"),
            })
        }
        Err(e) => Err(parse_api_error(e, "AppzDev", "Remove alias")),
    }
}

pub fn list_projects(api_base: &str, token: Option<&str>) -> DeployResult<Vec<ProjectInfo>> {
    let url = format!("{api_base}/v0/projects");
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "List projects"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from list projects: {e}"),
        })?;

    let items = if resp.is_array() {
        resp
    } else if let Some(arr) = resp.get("projects").filter(|v| v.is_array()) {
        arr.clone()
    } else {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: "Expected array of projects".into(),
        });
    };

    serde_json::from_value(items).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse project list: {e}"),
    })
}

pub fn add_project(api_base: &str, token: Option<&str>, name: &str) -> DeployResult<ProjectInfo> {
    let url = format!("{api_base}/v0/projects");
    let slug = slugify_project_id(name);
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({
            "name": name,
            "slug": slug,
        }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Create project"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from add project: {e}"),
        })?;

    let val = if resp.get("project").is_some() {
        resp["project"].clone()
    } else {
        resp
    };

    serde_json::from_value(val).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse created project: {e}"),
    })
}

/// Resolve a deployment ID or live URL to a `dpl_` ID.
fn deployment_id_from_ref(id_or_url: &str) -> DeployResult<String> {
    let trimmed = id_or_url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let id = trimmed
        .split('/')
        .next()
        .unwrap_or(trimmed)
        .split('.')
        .next()
        .unwrap_or(trimmed);
    if id.starts_with("dpl_") && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Ok(id.to_string());
    }
    Err(DeployError::Api {
        provider: "AppzDev",
        reason: format!(
            "`{id_or_url}` is not a deployment ID or URL. To delete a project, use `appz projects remove <name>`"
        ),
    })
}

/// `DELETE /v0/deployments/{id}`. The server answers 409 while an alias points
/// at the deployment; surface that with the way out.
pub fn remove_deployment(
    api_base: &str,
    token: Option<&str>,
    id_or_url: &str,
) -> DeployResult<String> {
    let id = deployment_id_from_ref(id_or_url)?;
    let mut req = ureq::delete(&format!("{api_base}/v0/deployments/{id}"));
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    match req.call() {
        Ok(_) => Ok(id),
        Err(ureq::Error::Status(409, resp)) => {
            let detail = resp
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|j| j.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| "an alias points at it".to_string());
            Err(DeployError::Api {
                provider: "AppzDev",
                reason: format!(
                    "{id} was not removed: {detail}. Remove the alias first with `appz alias rm <alias>`, then retry"
                ),
            })
        }
        Err(e) => Err(parse_api_error(e, "AppzDev", "Remove deployment")),
    }
}

fn validate_project_ref(id_or_slug: &str) -> DeployResult<()> {
    if id_or_slug.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id_or_slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Ok(());
    }
    Err(DeployError::Api {
        provider: "AppzDev",
        reason: "Expected a project slug or ID".into(),
    })
}

pub fn remove_project(api_base: &str, token: Option<&str>, id_or_slug: &str) -> DeployResult<bool> {
    validate_project_ref(id_or_slug)?;
    let url = format!("{api_base}/v0/projects/{id_or_slug}");
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let _ = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Remove project"))?;

    Ok(true)
}

pub fn get_project(
    api_base: &str,
    token: Option<&str>,
    id_or_slug: &str,
) -> DeployResult<ProjectInfo> {
    let url = format!("{api_base}/v0/projects/{id_or_slug}");
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Get project"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from get project: {e}"),
        })?;

    let val = if resp.get("project").is_some() {
        resp["project"].clone()
    } else {
        resp
    };

    serde_json::from_value(val).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse project details: {e}"),
    })
}

pub fn rename_project(
    api_base: &str,
    token: Option<&str>,
    id_or_slug: &str,
    new_name: &str,
) -> DeployResult<ProjectInfo> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return Err(DeployError::Api {
            provider: "AppzDev",
            reason: "Project name must be between 1 and 100 characters".into(),
        });
    }
    let url = format!("{api_base}/v0/projects/{id_or_slug}");
    let mut req = ureq::patch(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({
            "name": trimmed,
        }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Rename project"))?
        .into_json()
        .map_err(|e| DeployError::Api {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from rename project: {e}"),
        })?;

    let val = if resp.get("project").is_some() {
        resp["project"].clone()
    } else {
        resp
    };

    serde_json::from_value(val).map_err(|e| DeployError::Api {
        provider: "AppzDev",
        reason: format!("Failed to parse renamed project: {e}"),
    })
}

impl AppzdevProvider {
    fn api_base(&self, ctx: &DeployContext) -> String {
        ctx.run_env
            .get("APPZ_API_URL")
            .filter(|u| !u.is_empty())
            .cloned()
            .or_else(|| std::env::var("APPZ_API_URL").ok().filter(|u| !u.is_empty()))
            .unwrap_or_else(|| DEFAULT_API_URL.to_string())
    }

    fn resolve_project_id(&self, ctx: &DeployContext) -> String {
        ctx.run_env
            .get("APPZ_PROJECT_ID")
            .filter(|id| !id.is_empty())
            .map(|id| slugify_project_id(id))
            .unwrap_or_else(|| resolve_project_id_from_dir(&ctx.project_dir))
    }

    fn deploy_internal(&self, ctx: &DeployContext, is_preview: bool) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        if ctx.dry_run {
            return Ok(DeployOutput::dry_run(self.slug(), is_preview));
        }

        let output_path = ctx.output_path();
        let (mut scanned, _staged_blobs) =
            prepare_upload(&output_path).map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: e,
            })?;

        if scanned.files.is_empty() {
            return Err(DeployError::Api {
                provider: self.name(),
                reason: format!(
                    "No static files found in output directory: {}",
                    output_path.display()
                ),
            });
        }

        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let token_opt = resolve_auth_token(Some(ctx));
        let target = if is_preview { "preview" } else { "production" };

        // Opt-in "Markdown for Agents": add a `.md` sibling per `.html`, converted
        // by the hosted API. The guard owns the generated files, so it must
        // outlive the uploads. Conversion problems only cost a page its
        // sibling: they warn, they never fail the deploy.
        let markdown_enabled = read_markdown_config(&ctx.project_dir)?.enabled;
        let _markdown_siblings = if markdown_enabled {
            // The project's primary host is not known to this provider yet, so
            // links in the siblings stay relative.
            let converter = HttpMarkdownConverter::new(&api_base, token_opt.as_deref());
            match add_markdown_siblings(&output_path, &mut scanned, None, &converter) {
                Ok(guard) => {
                    if let Some(warning) = guard.warning()
                        && !ctx.json_output
                    {
                        eprintln!("warning: {warning}");
                    }
                    Some(guard)
                }
                Err(e) => {
                    if !ctx.json_output {
                        eprintln!("warning: Markdown for Agents: skipped ({e})");
                    }
                    None
                }
            }
        } else {
            None
        };

        let payload = negotiate_payload(&project_id, target, &scanned.files, markdown_enabled);

        // Step 1: POST /v0/deployments - Handshake & missing blob diff
        let create_url = format!("{}/v0/deployments", api_base);
        let mut req = ureq::post(&create_url).set("User-Agent", DEPLOY_USER_AGENT);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let negotiating = interrupt::negotiating().map_err(|e| DeployError::Api {
            provider: self.name(),
            reason: format!("could not install deploy interrupt handler: {e}"),
        })?;
        if negotiating.is_some() {
            req = req.timeout(Duration::from_secs(60));
        }
        let resp_result = req.send_json(payload);
        let (resp, deployment_id, missing, inline_publish_failed) = match resp_result {
            Ok(r) => {
                let v: serde_json::Value = r.into_json().map_err(|e| DeployError::Api {
                    provider: self.name(),
                    reason: format!("Invalid JSON response from {create_url}: {e}"),
                })?;
                let dpl_id = v["deploymentId"]
                    .as_str()
                    .or_else(|| v["id"].as_str())
                    .ok_or_else(|| DeployError::Api {
                        provider: self.name(),
                        reason: "Missing deploymentId in response".into(),
                    })?
                    .to_string();
                let missing = v["missing"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                (v, dpl_id, missing, false)
            }
            Err(ureq::Error::Status(500, response)) => {
                let err_text = response.into_string().unwrap_or_default();
                if let Ok(body) = serde_json::from_str::<serde_json::Value>(&err_text) {
                    if let Some(dpl_id) = body
                        .get("deploymentId")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                    {
                        (body, dpl_id, Vec::new(), true)
                    } else {
                        return Err(DeployError::Api {
                            provider: self.name(),
                            reason: format!("Initiate deployment failed (500): {err_text}"),
                        });
                    }
                } else {
                    return Err(DeployError::Api {
                        provider: self.name(),
                        reason: format!("Initiate deployment failed (500): {err_text}"),
                    });
                }
            }
            Err(e) => return Err(parse_api_error(e, self.name(), "Initiate deployment")),
        };

        let _pending_upload = negotiating.and_then(|slot| {
            (!missing.is_empty() || inline_publish_failed).then(|| {
                interrupt::PendingUpload::track(
                    slot,
                    &api_base,
                    &deployment_id,
                    token_opt.as_deref(),
                )
            })
        });
        interrupt::check_interrupted();

        // Step 2: Upload missing blobs with bounded concurrency (pool of 8) and retry
        if !missing.is_empty() {
            // A hash we never sent can't be uploaded; committing without it
            // would publish an incomplete site, so stop here instead.
            let missing_blobs: Vec<(&str, &std::path::Path)> = missing
                .iter()
                .map(|h| {
                    scanned
                        .blobs_by_hash
                        .get(h)
                        .map(|b| (h.as_str(), b.path.as_path()))
                        .ok_or_else(|| DeployError::Api {
                            provider: self.name(),
                            reason: format!("server asked for unknown file content {h}"),
                        })
                })
                .collect::<Result<_, _>>()?;

            let worker_count = 8.min(missing_blobs.len()).max(1);
            let next_idx = AtomicUsize::new(0);
            let upload_error = Mutex::new(None::<String>);
            let agent = ureq::builder()
                .user_agent(DEPLOY_USER_AGENT)
                .timeout_read(Duration::from_secs(30))
                .timeout_write(Duration::from_secs(30))
                .max_idle_connections(32)
                .max_idle_connections_per_host(16)
                .build();
            let auth_header = token_opt.as_deref().map(|t| format!("Bearer {t}"));

            std::thread::scope(|s| {
                for _ in 0..worker_count {
                    s.spawn(|| {
                        loop {
                            let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                            if idx >= missing_blobs.len() {
                                break;
                            }
                            if upload_error.lock().unwrap().is_some() {
                                break;
                            }
                            interrupt::check_interrupted();
                            let (hash, path) = missing_blobs[idx];
                            // Read only now, so at most one file per worker is
                            // in memory instead of the whole output directory.
                            // Cap read to MAX_FILE_SIZE + 1 bytes to prevent memory exhaustion
                            // if a file grows after initial scan.
                            let mut file = match std::fs::File::open(path) {
                                Ok(f) => f,
                                Err(e) => {
                                    let mut lock = upload_error.lock().unwrap();
                                    if lock.is_none() {
                                        *lock =
                                            Some(format!("cannot open {}: {e}", path.display()));
                                    }
                                    break;
                                }
                            };
                            let mut data = Vec::new();
                            if let Err(e) = (&mut file)
                                .take((crate::static_cas::MAX_FILE_SIZE + 1) as u64)
                                .read_to_end(&mut data)
                            {
                                let mut lock = upload_error.lock().unwrap();
                                if lock.is_none() {
                                    *lock =
                                        Some(format!("cannot read {}: {e}", path.display()));
                                }
                                break;
                            }
                            if data.len() > crate::static_cas::MAX_FILE_SIZE {
                                let mut lock = upload_error.lock().unwrap();
                                if lock.is_none() {
                                    *lock = Some(format!(
                                        "file {} exceeds maximum allowed size of {} bytes",
                                        path.display(),
                                        crate::static_cas::MAX_FILE_SIZE
                                    ));
                                }
                                break;
                            }

                            let read_hash = crate::static_cas::blob_id(&data);
                            if read_hash != hash {
                                let mut lock = upload_error.lock().unwrap();
                                if lock.is_none() {
                                    *lock = Some(format!(
                                        "file {} changed during deploy: expected hash {hash}, got {read_hash}",
                                        path.display()
                                    ));
                                }
                                break;
                            }

                            let upload_url = format!(
                                "{}/v0/deployments/{}/files/{}",
                                api_base, deployment_id, hash
                            );

                            let mut last_err = None;
                            for attempt in 0..3 {
                                if attempt > 0 {
                                    std::thread::sleep(Duration::from_millis(200 * (1 << attempt)));
                                }
                                interrupt::check_interrupted();
                                let mut up_req = agent
                                    .put(&upload_url)
                                    .set("Content-Type", "application/octet-stream");
                                if let Some(auth) = &auth_header {
                                    up_req = up_req.set("Authorization", auth);
                                }
                                match up_req.send_bytes(&data) {
                                    Ok(_) => {
                                        last_err = None;
                                        break;
                                    }
                                    Err(ureq::Error::Status(code, _))
                                        if code == 429 || code >= 500 =>
                                    {
                                        last_err =
                                            Some(format!("HTTP {code} uploading blob {hash}"));
                                        continue;
                                    }
                                    // A dropped connection or timeout is as
                                    // transient as a 5xx: retry it too.
                                    Err(ureq::Error::Transport(t)) => {
                                        last_err = Some(format!("uploading blob {hash}: {t}"));
                                        continue;
                                    }
                                    Err(e) => {
                                        let parsed = parse_api_error(e, "AppzDev", "Upload blob");
                                        last_err = Some(parsed.to_string());
                                        break;
                                    }
                                }
                            }
                            if let Some(err) = last_err {
                                let mut lock = upload_error.lock().unwrap();
                                if lock.is_none() {
                                    *lock = Some(err);
                                }
                                break;
                            }
                        }
                    });
                }
            });

            if let Some(err) = upload_error.into_inner().unwrap() {
                return Err(DeployError::Api {
                    provider: self.name(),
                    reason: err,
                });
            }
        }

        // Step 3: Commit deployment if blobs were uploaded, or if inline publish failed on 500
        let final_url = if !missing.is_empty() || inline_publish_failed {
            interrupt::check_interrupted();
            let commit_url = format!("{}/v0/deployments/{}/commit", api_base, deployment_id);
            let mut commit_req = ureq::post(&commit_url).set("User-Agent", DEPLOY_USER_AGENT);
            if let Some(token) = &token_opt {
                commit_req = commit_req.set("Authorization", &format!("Bearer {token}"));
            }
            let commit_resp: serde_json::Value = commit_req
                .send_json(serde_json::json!({}))
                .map_err(|e| parse_api_error(e, self.name(), "Commit deployment"))?
                .into_json()
                .map_err(|e| DeployError::Api {
                    provider: self.name(),
                    reason: format!("Invalid commit response: {e}"),
                })?;

            if !is_preview && commit_resp["productionUrl"].is_string() {
                commit_resp["productionUrl"].as_str().map(String::from)
            } else {
                commit_resp["url"].as_str().map(String::from)
            }
        } else if !is_preview && resp["productionUrl"].is_string() {
            resp["productionUrl"].as_str().map(String::from)
        } else {
            resp["url"].as_str().map(String::from)
        };

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: final_url,
            is_preview,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            // A new deployment was created and published even when every
            // blob was already stored, so this is never "nothing redeployed".
            unchanged: false,
            backup: None,
        })
    }
}

impl DeployProvider for AppzdevProvider {
    fn name(&self) -> &'static str {
        "AppzDev"
    }

    fn slug(&self) -> &'static str {
        "appzdev"
    }

    fn cli_tool(&self) -> &'static str {
        "appz"
    }

    fn install_package(&self) -> &'static str {
        "@getappz/cli"
    }

    fn auth_env_var(&self) -> &'static str {
        "APPZ_TOKEN"
    }

    fn whoami_args(&self) -> &'static [&'static str] {
        &["whoami"]
    }

    fn detect_config(&self, project_dir: &Path) -> DeployResult<Option<DetectedConfig>> {
        let appz_dir = project_dir.join(".appz");
        let project_json = appz_dir.join("project.json");
        if project_json.is_file() {
            let content = std::fs::read_to_string(&project_json).map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: e.to_string(),
            })?;
            let (name, team) = serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .map(|v| {
                    (
                        v.get("projectId")
                            .or_else(|| v.get("projectName"))
                            .and_then(|s| s.as_str())
                            .map(String::from),
                        v.get("orgId").and_then(|s| s.as_str()).map(String::from),
                    )
                })
                .unwrap_or((None, None));
            return Ok(Some(DetectedConfig {
                config_file: ".appz/project.json".into(),
                is_linked: true,
                project_name: name,
                team,
            }));
        }

        if project_dir.join("appz.jsonc").is_file() {
            return Ok(Some(DetectedConfig {
                config_file: "appz.jsonc".into(),
                is_linked: false,
                project_name: None,
                team: None,
            }));
        }

        if project_dir.join("appz.json").is_file() {
            return Ok(Some(DetectedConfig {
                config_file: "appz.json".into(),
                is_linked: false,
                project_name: None,
                team: None,
            }));
        }

        Ok(None)
    }

    fn check_prerequisites(&self) -> DeployResult<PrerequisiteStatus> {
        Ok(PrerequisiteStatus::Ready)
    }

    fn is_linked(&self, ctx: &DeployContext) -> bool {
        ctx.project_dir.join(".appz").join("project.json").is_file()
            || ctx.run_env.contains_key("APPZ_PROJECT_ID")
            || std::env::var("APPZ_PROJECT_ID").is_ok()
    }

    fn link(&self, ctx: &DeployContext, _interactive: bool) -> DeployResult<()> {
        let appz_dir = ctx.project_dir.join(".appz");
        std::fs::create_dir_all(&appz_dir).map_err(|e| DeployError::Api {
            provider: self.name(),
            reason: format!("Failed to create .appz directory: {e}"),
        })?;

        let project_id = self.resolve_project_id(ctx);
        let project_json = appz_dir.join("project.json");

        let stored_org = load_stored_auth().and_then(|a| a.organization_id);
        let existing_org = if project_json.is_file() {
            std::fs::read_to_string(&project_json)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| v.get("orgId").and_then(|s| s.as_str()).map(String::from))
        } else {
            None
        };

        let org_id = existing_org.or(stored_org);
        let content = if let Some(org) = org_id {
            serde_json::json!({
                "projectId": project_id,
                "orgId": org,
            })
        } else {
            serde_json::json!({
                "projectId": project_id,
            })
        };

        std::fs::write(
            &project_json,
            serde_json::to_string_pretty(&content).unwrap(),
        )
        .map_err(|e| DeployError::Api {
            provider: self.name(),
            reason: format!("Failed to write .appz/project.json: {e}"),
        })?;

        Ok(())
    }

    fn deploy(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, false)
    }

    fn deploy_preview(&self, ctx: &DeployContext) -> DeployResult<DeployOutput> {
        self.deploy_internal(ctx, true)
    }

    fn rollback(&self, ctx: &DeployContext, version_id: &str) -> DeployResult<DeployOutput> {
        let start = Instant::now();
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let rollback_url = format!("{}/v0/projects/{}/rollback", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::post(&rollback_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "versionId": version_id }))
            .map_err(|e| parse_api_error(e, self.name(), "Rollback deployment"))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Invalid rollback response: {e}"),
            })?;

        let host = resp["host"].as_str().unwrap_or(&project_id);
        let url = format!("https://{}", host);

        Ok(DeployOutput {
            provider: self.slug().to_string(),
            url: Some(url),
            is_preview: false,
            status: DeployStatus::Ready,
            created_at: Some(chrono::Utc::now()),
            duration_ms: Some(start.elapsed().as_millis() as u64),
            auth: None,
            setup_files: Vec::new(),
            cli_missing: false,
            link_needed: false,
            unchanged: false,
            backup: None,
        })
    }

    fn alias(&self, ctx: &DeployContext, domain: &str) -> DeployResult<String> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let alias_url = format!("{}/v0/projects/{}/alias", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::post(&alias_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "domain": domain }))
            .map_err(|e| parse_api_error(e, self.name(), "Alias domain"))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Invalid alias response: {e}"),
            })?;

        let bound_domain = resp["domain"].as_str().unwrap_or(domain);
        Ok(format!("https://{bound_domain}"))
    }

    fn list_aliases(&self, ctx: &DeployContext) -> DeployResult<Vec<crate::AliasInfo>> {
        let project_id = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        list_project_aliases(&self.api_base(ctx), &project_id, token.as_deref())
    }

    fn remove_alias(&self, ctx: &DeployContext, host: &str) -> DeployResult<()> {
        let project_id = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        remove_project_alias(&self.api_base(ctx), &project_id, token.as_deref(), host)
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let list_url = format!("{}/v0/projects/{}/deployments", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::get(&list_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| parse_api_error(e, self.name(), "List deployments"))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Invalid JSON from list deployments: {e}"),
            })?;

        // The API returns `{items, next}`; older servers returned a bare array.
        let items = resp
            .as_array()
            .or_else(|| resp.get("items").and_then(|v| v.as_array()))
            .ok_or_else(|| DeployError::Api {
                provider: self.name(),
                reason: "Expected deployments as `{items}` or a JSON array".into(),
            })?;

        let mut out = Vec::new();
        for item in items {
            let id = item["id"].as_str().unwrap_or_default().to_string();
            let version_id = item["versionId"].as_str().map(String::from);
            let url = item["url"].as_str().map(String::from);
            let status_str = item["status"].as_str().unwrap_or("ready");
            let status = match status_str {
                "ready" => DeployStatus::Ready,
                "building" => DeployStatus::Building,
                "error" => DeployStatus::Error,
                _ => DeployStatus::Unknown,
            };
            let created_at = item["createdAt"]
                .as_i64()
                .and_then(chrono::DateTime::from_timestamp_millis);
            let is_current = item["isCurrent"].as_bool().unwrap_or(false);

            out.push(DeploymentInfo {
                id,
                version_id,
                url,
                status,
                created_at,
                is_current,
            });
        }

        Ok(out)
    }

    fn list_team_deployments(
        &self,
        ctx: &DeployContext,
        query: &DeploymentQuery,
    ) -> DeployResult<Page<TeamDeployment>> {
        let token = resolve_auth_token(Some(ctx));
        list_team_deployments(&self.api_base(ctx), token.as_deref(), query)
    }

    fn promote_deployment(
        &self,
        ctx: &DeployContext,
        id_or_url: &str,
        confirm: &dyn Fn(&DeploymentInspect) -> bool,
    ) -> DeployResult<Option<PromoteResult>> {
        let deployment = self.inspect_deployment(ctx, id_or_url)?;
        if deployment.status != DeployStatus::Ready {
            return Err(DeployError::Api {
                provider: self.name(),
                reason: format!(
                    "deployment {} is {:?}; only ready deployments can be promoted",
                    deployment.id, deployment.status
                )
                .to_lowercase(),
            });
        }
        if !confirm(&deployment) {
            return Ok(None);
        }
        // Promote inside the deployment's own project, whatever dir we run in.
        let project_id = if deployment.project_id.is_empty() {
            self.resolve_project_id(ctx)
        } else {
            deployment.project_id.clone()
        };
        let url = format!("{}/v0/projects/{}/promote", self.api_base(ctx), project_id);
        let token = resolve_auth_token(Some(ctx));
        let resp = post_promote(&url, token.as_deref(), &deployment.id)
            .map_err(|e| parse_api_error(*e, self.name(), "Promote deployment"))?;
        Ok(Some(PromoteResult {
            provider: self.slug().to_string(),
            project_id: resp["projectId"].as_str().unwrap_or(&project_id).into(),
            deployment_id: resp["deploymentId"]
                .as_str()
                .unwrap_or(&deployment.id)
                .into(),
            version_id: resp["versionId"].as_str().unwrap_or_default().into(),
            url: format!("https://{}", resp["host"].as_str().unwrap_or_default()),
        }))
    }

    fn inspect_deployment(
        &self,
        ctx: &DeployContext,
        id_or_url: &str,
    ) -> DeployResult<DeploymentInspect> {
        let api_base = self.api_base(ctx);
        let trimmed = id_or_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let dpl_id = trimmed
            .split('/')
            .next()
            .unwrap_or(trimmed)
            .split('.')
            .next()
            .unwrap_or(trimmed);

        let inspect_url = format!("{}/v0/deployments/{}", api_base, dpl_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::get(&inspect_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| parse_api_error(e, self.name(), "Inspect deployment"))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Invalid JSON from inspect deployment: {e}"),
            })?;

        let mut inspect: DeploymentInspect =
            serde_json::from_value(resp.clone()).map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Failed to parse deployment inspect: {e}"),
            })?;

        if inspect.hosts.is_empty() {
            if let Some(url_str) = resp.get("url").and_then(|u| u.as_str()) {
                let h = url_str
                    .trim_start_matches("https://")
                    .trim_start_matches("http://");
                inspect.hosts.push(h.to_string());
            }
            if let Some(prod_url) = resp.get("productionUrl").and_then(|u| u.as_str()) {
                let h = prod_url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://");
                inspect.hosts.push(h.to_string());
            }
        }

        Ok(inspect)
    }

    fn add_domain(&self, ctx: &DeployContext, domain: &str) -> DeployResult<ProjectDomain> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        add_project_domain(&api_base, &project_id, token.as_deref(), domain)
    }

    fn list_domains(&self, ctx: &DeployContext) -> DeployResult<Vec<ProjectDomain>> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        list_project_domains(&api_base, &project_id, token.as_deref())
    }

    fn list_team_domains(
        &self,
        ctx: &DeployContext,
        query: &DomainQuery,
    ) -> DeployResult<Page<TeamDomain>> {
        let token = resolve_auth_token(Some(ctx));
        list_team_domains(&self.api_base(ctx), token.as_deref(), query)
    }

    fn inspect_domain(
        &self,
        ctx: &DeployContext,
        domain: &str,
    ) -> DeployResult<Option<TeamDomain>> {
        let token = resolve_auth_token(Some(ctx));
        inspect_team_domain(&self.api_base(ctx), token.as_deref(), domain)
    }

    fn move_domain(
        &self,
        ctx: &DeployContext,
        domain: &str,
        to_org: &str,
        project: &str,
    ) -> DeployResult<DomainMove> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        let current =
            inspect_team_domain(&api_base, token.as_deref(), domain)?.ok_or_else(|| {
                DeployError::Api {
                    provider: "AppzDev",
                    reason: format!("{domain} is not attached to any project"),
                }
            })?;
        let from = current
            .project_id
            .unwrap_or_else(|| self.resolve_project_id(ctx));
        move_project_domain(&api_base, token.as_deref(), &from, domain, to_org, project)
    }

    fn verify_domain(&self, ctx: &DeployContext, domain: &str) -> DeployResult<DomainVerifyResult> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        verify_project_domain(&api_base, &project_id, token.as_deref(), domain)
    }

    fn remove_domain(&self, ctx: &DeployContext, domain: &str) -> DeployResult<bool> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        remove_project_domain(&api_base, &project_id, token.as_deref(), domain)
    }

    fn list_projects(&self, ctx: &DeployContext) -> DeployResult<Vec<ProjectInfo>> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        list_projects(&api_base, token.as_deref())
    }

    fn get_project(&self, ctx: &DeployContext, id_or_slug: &str) -> DeployResult<ProjectInfo> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        get_project(&api_base, token.as_deref(), id_or_slug)
    }

    fn add_project(&self, ctx: &DeployContext, name: &str) -> DeployResult<ProjectInfo> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        add_project(&api_base, token.as_deref(), name)
    }

    fn rename_project(
        &self,
        ctx: &DeployContext,
        id_or_slug: &str,
        new_name: &str,
    ) -> DeployResult<ProjectInfo> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        rename_project(&api_base, token.as_deref(), id_or_slug, new_name)
    }

    fn list_activity(
        &self,
        ctx: &DeployContext,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> DeployResult<ActivityPage> {
        let token = resolve_auth_token(Some(ctx));
        list_activity(&self.api_base(ctx), token.as_deref(), limit, cursor)
    }

    fn usage_report(&self, ctx: &DeployContext) -> DeployResult<UsageReport> {
        let token = resolve_auth_token(Some(ctx));
        get_usage_report(&self.api_base(ctx), token.as_deref())
    }

    fn list_webhooks(
        &self,
        ctx: &DeployContext,
        query: &crate::webhooks::WebhookQuery,
    ) -> DeployResult<Page<crate::webhooks::Webhook>> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::list_webhooks(&self.api_base(ctx), tok.as_deref(), query)
    }

    fn add_webhook(
        &self,
        ctx: &DeployContext,
        url: &str,
        events: &[String],
        project: Option<&str>,
    ) -> DeployResult<crate::webhooks::Webhook> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::add_webhook(&self.api_base(ctx), tok.as_deref(), url, events, project)
    }

    fn get_webhook(&self, ctx: &DeployContext, id: &str) -> DeployResult<crate::webhooks::Webhook> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::get_webhook(&self.api_base(ctx), tok.as_deref(), id)
    }

    fn remove_webhook(&self, ctx: &DeployContext, id: &str) -> DeployResult<bool> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::remove_webhook(&self.api_base(ctx), tok.as_deref(), id)
    }

    fn list_webhook_deliveries(
        &self,
        ctx: &DeployContext,
        id: &str,
        query: &crate::webhooks::WebhookQuery,
    ) -> DeployResult<Page<crate::webhooks::WebhookDelivery>> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::list_webhook_deliveries(&self.api_base(ctx), tok.as_deref(), id, query)
    }

    fn test_webhook(
        &self,
        ctx: &DeployContext,
        id: &str,
    ) -> DeployResult<crate::webhooks::WebhookDelivery> {
        let tok = resolve_auth_token(Some(ctx));
        crate::webhooks::test_webhook(&self.api_base(ctx), tok.as_deref(), id)
    }

    fn list_redirects(
        &self,
        ctx: &DeployContext,
        version: Option<u64>,
    ) -> DeployResult<RedirectList> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        list_project_redirects(&self.api_base(ctx), &project, token.as_deref(), version)
    }

    fn add_redirect(
        &self,
        ctx: &DeployContext,
        rule: &RedirectInput,
    ) -> DeployResult<RedirectList> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        add_project_redirect(&self.api_base(ctx), &project, token.as_deref(), rule)
    }

    fn remove_redirect(&self, ctx: &DeployContext, id: &str) -> DeployResult<RedirectList> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        remove_project_redirect(&self.api_base(ctx), &project, token.as_deref(), id)
    }

    fn import_redirects(
        &self,
        ctx: &DeployContext,
        rules: &[RedirectInput],
    ) -> DeployResult<RedirectList> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        import_project_redirects(&self.api_base(ctx), &project, token.as_deref(), rules)
    }

    fn list_redirect_versions(&self, ctx: &DeployContext) -> DeployResult<RedirectVersions> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        list_project_redirect_versions(&self.api_base(ctx), &project, token.as_deref())
    }

    fn update_redirect_version(
        &self,
        ctx: &DeployContext,
        version: u64,
        action: &str,
    ) -> DeployResult<RedirectVersionRef> {
        let project = self.resolve_project_id(ctx);
        let token = resolve_auth_token(Some(ctx));
        let base = self.api_base(ctx);
        update_project_redirect_version(&base, &project, token.as_deref(), version, action)
    }

    fn remove_deployment(&self, ctx: &DeployContext, id_or_url: &str) -> DeployResult<String> {
        remove_deployment(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id_or_url,
        )
    }

    fn restore_project(&self, ctx: &DeployContext, id_or_slug: &str) -> DeployResult<ProjectInfo> {
        let base = self.api_base(ctx);
        validate_project_ref(id_or_slug)?;
        let mut req = ureq::post(&format!("{base}/v0/projects/{id_or_slug}/restore"));
        if let Some(token) = resolve_auth_token(Some(ctx)) {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }
        req.call()
            .map_err(|e| parse_api_error(e, self.name(), "Restore project"))?
            .into_json()
            .map_err(|e| DeployError::Api {
                provider: self.name(),
                reason: format!("Invalid project in restore response: {e}"),
            })
    }

    fn project_state(&self, ctx: &DeployContext, id: &str) -> DeployResult<ProjectState> {
        projects::get_project_state(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
        )
    }

    fn pause_project(&self, ctx: &DeployContext, id: &str) -> DeployResult<ProjectState> {
        projects::project_action(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            "pause",
        )
    }

    fn resume_project(&self, ctx: &DeployContext, id: &str) -> DeployResult<ProjectState> {
        projects::project_action(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            "resume",
        )
    }

    fn keep_project(&self, ctx: &DeployContext, id: &str) -> DeployResult<ProjectState> {
        projects::project_action(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            "keep",
        )
    }

    fn update_project(
        &self,
        ctx: &DeployContext,
        id: &str,
        update: &projects::ProjectSettingsUpdate,
    ) -> DeployResult<ProjectState> {
        projects::update_project(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            update,
        )
    }

    fn request_transfer(
        &self,
        ctx: &DeployContext,
        id: &str,
        to_org: &str,
    ) -> DeployResult<Transfer> {
        projects::request_transfer(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            to_org,
        )
    }

    fn list_transfers(&self, ctx: &DeployContext) -> DeployResult<Vec<Transfer>> {
        projects::list_transfers(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
        )
    }

    fn get_transfer(&self, ctx: &DeployContext, id: &str) -> DeployResult<Transfer> {
        projects::get_transfer(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
        )
    }

    fn accept_transfer(
        &self,
        ctx: &DeployContext,
        id: &str,
        timeout: Duration,
    ) -> DeployResult<Transfer> {
        projects::accept_transfer(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
            timeout,
            Duration::from_secs(2),
        )
    }

    fn cancel_transfer(&self, ctx: &DeployContext, id: &str) -> DeployResult<Transfer> {
        projects::cancel_transfer(
            &self.api_base(ctx),
            resolve_auth_token(Some(ctx)).as_deref(),
            id,
        )
    }

    fn remove_project(&self, ctx: &DeployContext, id_or_slug: &str) -> DeployResult<bool> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        remove_project(&api_base, token.as_deref(), id_or_slug)
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    /// The tests below point `APPZ_VAULT_HOME_OVERRIDE` (process-wide) at
    /// their own temp dir; they must not run at the same time.
    static VAULT_HOME_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
    use tempfile::tempdir;

    #[test]
    fn test_slugify_project_id() {
        assert_eq!(slugify_project_id("prj_1234567890"), "prj_1234567890");
        assert_eq!(slugify_project_id("My Awesome Site"), "my-awesome-site");
        assert_eq!(slugify_project_id("---foo---bar---"), "foo-bar");
        assert_eq!(slugify_project_id("Hello, World! 123"), "hello-world-123");
        assert_eq!(slugify_project_id("$$$"), "default-project");
        let long_name = "a".repeat(60);
        let slug = slugify_project_id(&long_name);
        assert_eq!(slug.len(), 40);
        assert_eq!(slug, "a".repeat(40));
    }

    #[test]
    fn test_folder_slug() {
        for (input, want) in [
            // domain suffixes
            ("rksolar.co.in", "rksolar"),
            ("rksolar.in", "rksolar"),
            ("rksolar.com", "rksolar"),
            ("RKSolar.COM", "rksolar"),
            ("www.rksolar.com", "rksolar"),
            ("shop.rksolar.com.au", "shop-rksolar"),
            ("myapp.vercel.app", "myapp"),
            ("com", "com"),
            ("next.js", "next-js"),
            (".com", "com"),
            ("site.", "site"),
            // copy / download markers
            ("rksolar.com (1)", "rksolar"),
            ("my site - Copy", "my-site"),
            ("my site - Copy (2)", "my-site"),
            ("my site copy 2", "my-site"),
            ("rksolar.com-main", "rksolar"),
            ("portfolio-master", "portfolio"),
            ("main", "main"),
            ("copy", "copy"),
            // unicode, punctuation, empties
            ("Café Münch", "cafe-munch"),
            ("John's Site", "johns-site"),
            ("my_site", "my-site"),
            ("  spaced  ", "spaced"),
            ("🚀🚀", "rocket-rocket"),
            ("!!!", "default-project"),
            ("", "default-project"),
            ("prj_My Folder", "prj-my-folder"),
        ] {
            assert_eq!(folder_slug(input), want, "input: {input:?}");
        }
    }

    #[test]
    fn test_resolve_project_id_from_dot_dir() {
        let dir = tempdir().unwrap();
        let folder = dir.path().join("rksolar.co.in");
        std::fs::create_dir_all(folder.join("sub")).unwrap();
        assert_eq!(
            resolve_project_id_from_dir(&folder.join("sub").join("..")),
            "rksolar"
        );
    }

    #[test]
    fn test_resolve_project_id_from_file() {
        let dir = tempdir().unwrap();
        let appz_dir = dir.path().join(".appz");
        std::fs::create_dir_all(&appz_dir).unwrap();
        std::fs::write(
            appz_dir.join("project.json"),
            r#"{"projectId": "prj_file123", "orgId": "org_abc"}"#,
        )
        .unwrap();

        let ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        let provider = AppzdevProvider;
        assert_eq!(provider.resolve_project_id(&ctx), "prj_file123");
        assert!(provider.is_linked(&ctx));

        let config = provider.detect_config(dir.path()).unwrap().unwrap();
        assert_eq!(config.project_name.as_deref(), Some("prj_file123"));
        assert_eq!(config.team.as_deref(), Some("org_abc"));
    }

    #[test]
    fn test_resolve_project_id_normalizes_folder_name() {
        let dir = tempdir().unwrap();
        let folder = dir.path().join("My Test Project!");
        std::fs::create_dir_all(&folder).unwrap();

        let ctx = DeployContext::new(folder, "dist".to_string());
        let provider = AppzdevProvider;
        assert_eq!(provider.resolve_project_id(&ctx), "my-test-project");
    }

    #[test]
    fn test_link_omits_placeholder_org() {
        let dir = tempdir().unwrap();
        let _env = VAULT_HOME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let old_override = std::env::var("APPZ_VAULT_HOME_OVERRIDE").ok();
        unsafe {
            std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", dir.path());
        }
        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "prj_custom_1".to_string());

        let provider = AppzdevProvider;
        provider.link(&ctx, false).unwrap();

        let project_json = dir.path().join(".appz").join("project.json");
        assert!(project_json.is_file());

        let raw = std::fs::read_to_string(&project_json).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["projectId"], "prj_custom_1");
        assert!(val.get("orgId").is_none());

        if let Some(orig) = old_override {
            unsafe {
                std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("APPZ_VAULT_HOME_OVERRIDE");
            }
        }
    }

    #[test]
    fn test_link_uses_stored_org_when_present() {
        let dir = tempdir().unwrap();
        let _env = VAULT_HOME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let old_override = std::env::var("APPZ_VAULT_HOME_OVERRIDE").ok();
        unsafe {
            std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", dir.path());
        }

        save_stored_auth(&StoredAuth {
            token: "tok".to_string(),
            user_id: None,
            email: None,
            organization_id: Some("org_from_session".to_string()),
        })
        .unwrap();

        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "prj_stored_org".to_string());

        let provider = AppzdevProvider;
        provider.link(&ctx, false).unwrap();

        let project_json = dir.path().join(".appz").join("project.json");
        let raw = std::fs::read_to_string(&project_json).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["projectId"], "prj_stored_org");
        assert_eq!(val["orgId"], "org_from_session");

        if let Some(orig) = old_override {
            unsafe {
                std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("APPZ_VAULT_HOME_OVERRIDE");
            }
        }
    }

    #[test]
    fn test_stored_auth_roundtrip() {
        let dir = tempdir().unwrap();
        let _env = VAULT_HOME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let old_override = std::env::var("APPZ_VAULT_HOME_OVERRIDE").ok();
        unsafe {
            std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", dir.path());
        }

        let auth = StoredAuth {
            token: "test_token_xyz".to_string(),
            user_id: Some("usr_123".to_string()),
            email: Some("dev@appz.to".to_string()),
            organization_id: Some("org_999".to_string()),
        };

        save_stored_auth(&auth).unwrap();
        let loaded = load_stored_auth().unwrap();
        assert_eq!(loaded.token, "test_token_xyz");
        assert_eq!(loaded.email.as_deref(), Some("dev@appz.to"));
        assert_eq!(loaded.organization_id.as_deref(), Some("org_999"));

        clear_stored_auth().unwrap();
        assert!(load_stored_auth().is_none());

        if let Some(orig) = old_override {
            unsafe {
                std::env::set_var("APPZ_VAULT_HOME_OVERRIDE", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("APPZ_VAULT_HOME_OVERRIDE");
            }
        }
    }

    #[test]
    fn environment_slug_validation() {
        for ok in ["a", "staging", "qa-2", "0a"] {
            assert!(is_valid_environment_slug(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "A", "a_b", "a b", &"a".repeat(41)] {
            assert!(!is_valid_environment_slug(bad), "{bad}");
        }
    }

    #[test]
    fn test_parse_and_format_env_file() {
        let content = r#"
# Comment line
DATABASE_URL="postgres://user:pass@localhost:5432/db"
export API_KEY=sk_test_12345
EMPTY_VAL=""
QUOTED='single quoted'
SPACED=hello world
"#;
        let vars = parse_env_file(content);
        assert_eq!(vars.len(), 5);
        assert_eq!(
            vars[0],
            (
                "DATABASE_URL".to_string(),
                "postgres://user:pass@localhost:5432/db".to_string()
            )
        );
        assert_eq!(
            vars[1],
            ("API_KEY".to_string(), "sk_test_12345".to_string())
        );
        assert_eq!(vars[2], ("EMPTY_VAL".to_string(), "".to_string()));
        assert_eq!(vars[3], ("QUOTED".to_string(), "single quoted".to_string()));
        assert_eq!(vars[4], ("SPACED".to_string(), "hello world".to_string()));

        let formatted = format_env_file(&vars);
        assert!(formatted.contains("DATABASE_URL=postgres://user:pass@localhost:5432/db"));
        assert!(formatted.contains("API_KEY=sk_test_12345"));
        assert!(formatted.contains("SPACED=\"hello world\""));

        // Newline injection in values is escaped and invalid keys are dropped.
        let injected = vec![
            ("SAFE".to_string(), "val\nEVIL=1\r\n".to_string()),
            ("BAD=KEY".to_string(), "x".to_string()),
            ("BAD\nKEY".to_string(), "x".to_string()),
        ];
        let safe = format_env_file(&injected);
        assert_eq!(safe, "SAFE=\"val\\nEVIL=1\\r\\n\"\n");
    }

    struct MockServer {
        addr: String,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl MockServer {
        fn start() -> Self {
            use std::io::{BufRead, BufReader, Read, Write};
            use std::net::TcpListener;
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, Ordering};

            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = format!("http://{}", listener.local_addr().unwrap());
            let shutdown = Arc::new(AtomicBool::new(false));
            let shutdown_clone = Arc::clone(&shutdown);

            listener.set_nonblocking(true).unwrap();

            let handle = std::thread::spawn(move || {
                while !shutdown_clone.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let mut reader = BufReader::new(&mut stream);
                            let mut request_line = String::new();
                            if reader.read_line(&mut request_line).is_err()
                                || request_line.is_empty()
                            {
                                continue;
                            }

                            let mut content_length = 0;
                            let mut user_agent = String::new();
                            loop {
                                let mut line = String::new();
                                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                                    break;
                                }
                                let lower = line.to_lowercase();
                                if lower.starts_with("user-agent:") {
                                    user_agent = line["user-agent:".len()..].trim().to_owned();
                                }
                                if let Some(val) = lower.strip_prefix("content-length:") {
                                    content_length = val.trim().parse::<usize>().unwrap_or(0);
                                }
                            }

                            let mut body = vec![0u8; content_length];
                            if content_length > 0 {
                                let _ = reader.read_exact(&mut body);
                            }

                            let parts: Vec<&str> = request_line.split_whitespace().collect();
                            let method = parts.first().copied().unwrap_or("");
                            let path = parts.get(1).copied().unwrap_or("");

                            if matches!((method, path), ("POST", "/v0/deployments"))
                                || (method == "PUT" && path.contains("/files/"))
                                || (method == "POST" && path.ends_with("/commit"))
                            {
                                assert_eq!(
                                    user_agent,
                                    format!("appz-cli/{}", env!("CARGO_PKG_VERSION"))
                                );
                            }
                            let handshake;
                            let (status, resp_body) = if method == "POST"
                                && path == "/v0/deployments"
                            {
                                let manifest: serde_json::Value =
                                    serde_json::from_slice(&body).unwrap();
                                let files = manifest["files"].as_array().unwrap();
                                let js = files.iter().find(|f| f["path"] == "/app.js").unwrap();
                                let alias = format!(
                                    "/app-{}.js",
                                    &js["hash"].as_str().unwrap().strip_prefix("b3:").unwrap()
                                        [..10]
                                );
                                assert!(
                                    files
                                        .iter()
                                        .any(|f| f["path"] == alias && f["immutable"] == true)
                                );
                                let html = format!(r#"<script src="{alias}"></script>"#);
                                let hash = crate::static_cas::blob_id(html.as_bytes());
                                assert!(
                                    files
                                        .iter()
                                        .any(|f| f["path"] == "/index.html" && f["hash"] == hash)
                                );
                                handshake = serde_json::json!({"deploymentId":"dpl_mock_1","missing":[hash]}).to_string();
                                ("200 OK", handshake.as_str())
                            } else if method == "PUT" && path.contains("/files/") {
                                assert_eq!(
                                    path.rsplit('/').next().unwrap(),
                                    crate::static_cas::blob_id(&body)
                                );
                                ("200 OK", "{}")
                            } else if method == "POST" && path.ends_with("/commit") {
                                ("200 OK", r#"{"url":"https://mock.appz.page"}"#)
                            } else if method == "GET"
                                && path.starts_with("/v0/deployments/dpl_mock_1")
                            {
                                (
                                    "200 OK",
                                    r#"{"id":"dpl_mock_1","projectId":"prj_mock","versionId":"ver_mock_1","status":"ready","target":"production","hosts":["mock.appz.page"],"fileCount":2,"totalBytes":128,"files":[{"path":"/index.html","size":64,"hash":"h1"},{"path":"/app.js","size":64,"hash":"h2"}]}"#,
                                )
                            } else if method == "GET"
                                && (path == "/v0/deployments"
                                    || path.starts_with("/v0/deployments?"))
                            {
                                if path.contains("projectId=prj_mock") {
                                    (
                                        "200 OK",
                                        r#"{"items":[{"id":"dpl_mock_1","projectId":"prj_mock","projectSlug":"mock-app","projectName":"mock-app","url":"https://mock.appz.page","status":"ready","type":"production","createdAt":1762732800000,"isCurrent":true}],"next":"cur_2"}"#,
                                    )
                                } else {
                                    ("200 OK", r#"{"items":[],"next":null}"#)
                                }
                            } else if method == "GET"
                                && (path == "/v0/domains" || path.starts_with("/v0/domains?"))
                            {
                                (
                                    "200 OK",
                                    r#"{"items":[{"id":"dom_1","domain":"custom.example.com","verified":true,"sslStatus":"active","projectId":"prj_mock","projectSlug":"mock-app","projectName":"mock-app","createdAt":1762732800000}],"next":null}"#,
                                )
                            } else if method == "GET" && path.contains("/deployments") {
                                (
                                    "200 OK",
                                    r#"[{"id":"dpl_mock_1","url":"https://mock.appz.page","status":"ready","createdAt":"2026-11-10T00:00:00Z"}]"#,
                                )
                            } else if method == "POST" && path.contains("/rollback") {
                                ("200 OK", r#"{"success":true,"versionId":"ver_mock_1"}"#)
                            } else if method == "POST" && path.contains("/alias") {
                                ("200 OK", r#"{"success":true,"domain":"custom.com"}"#)
                            } else if method == "POST" && path.ends_with("/move") {
                                if path.contains("/forbidden.example.com/") {
                                    ("403 Forbidden", r#"{"error":"forbidden"}"#)
                                } else {
                                    (
                                        "200 OK",
                                        r#"{"id":"dom_1","domain":"custom.example.com","projectId":"prj_dest","organizationId":"org_dest","status":"completed"}"#,
                                    )
                                }
                            } else if method == "POST" && path.ends_with("/verify") {
                                (
                                    "200 OK",
                                    r#"{"id":"dom_1","domain":"custom.example.com","sslStatus":"active","verified":true,"cname":{"name":"custom.example.com","target":"cname.appz.to"}}"#,
                                )
                            } else if method == "POST" && path.ends_with("/domains") {
                                (
                                    "200 OK",
                                    r#"{"id":"dom_1","domain":"custom.example.com","sslStatus":"pending","verified":false,"challenge":{"type":"TXT","name":"_appz-challenge.custom.example.com","value":"appz-verify=mock_token"},"cname":{"name":"custom.example.com","target":"cname.appz.to"},"apex":{"name":"custom.example.com","ip":"15.207.29.70"}}"#,
                                )
                            } else if method == "GET" && path.ends_with("/domains") {
                                (
                                    "200 OK",
                                    r#"[{"id":"dom_1","domain":"custom.example.com","sslStatus":"active","verified":true,"cname":{"name":"custom.example.com","target":"cname.appz.to"}}]"#,
                                )
                            } else if method == "DELETE" && path.contains("/domains/") {
                                (
                                    "200 OK",
                                    r#"{"status":"deleted","domain":"custom.example.com"}"#,
                                )
                            } else if method == "GET" && path.contains("/environments") {
                                (
                                    "200 OK",
                                    r#"[{"id":"pen_abcdefghijkl","slug":"staging","branchPattern":"release/*"}]"#,
                                )
                            } else if method == "POST" && path.contains("/environments") {
                                (
                                    "201 Created",
                                    r#"{"id":"pen_abcdefghijkl","slug":"staging","branchPattern":null}"#,
                                )
                            } else if method == "DELETE" && path.contains("/environments/") {
                                ("204 No Content", "")
                            } else if method == "GET" && path.contains("/env/values") {
                                (
                                    "200 OK",
                                    r#"[{"id":"env_1","key":"MOCK_VAR","value":"test_val_123","target":["production"],"gitBranch":null,"createdAt":1762732800}]"#,
                                )
                            } else if method == "GET" && path.contains("/env") {
                                (
                                    "200 OK",
                                    r#"[{"id":"env_1","key":"MOCK_VAR","target":["production"],"gitBranch":"main","createdAt":1762732800}]"#,
                                )
                            } else if method == "POST" && path.contains("/env") {
                                ("200 OK", r#"{"status":"synced","count":1}"#)
                            } else if method == "DELETE" && path.contains("/env/") {
                                ("204 No Content", "")
                            } else if path.contains("/redirects") {
                                let rule_body = r#"{"version":5,"rules":[{"id":"rdr_1","source":"/old","destination":"/new","status":308,"position":0}]}"#;
                                match (method, path) {
                                    ("GET", p) if p.ends_with("/redirects/versions") => (
                                        "200 OK",
                                        r#"{"liveVersion":4,"stagedVersion":5,"versions":[{"version":5,"createdAt":1762732900},{"version":4,"createdAt":1762732800}]}"#,
                                    ),
                                    ("GET", p) if p.ends_with("/redirects?version=4") => {
                                        ("200 OK", r#"{"version":4,"rules":[]}"#)
                                    }
                                    ("GET", _) | ("POST", _) | ("DELETE", _)
                                        if path.contains("/versions/") =>
                                    {
                                        ("200 OK", r#"{"version":5}"#)
                                    }
                                    ("POST", p) if p.ends_with("/redirects/import") => {
                                        let rules: serde_json::Value =
                                            serde_json::from_slice(&body).unwrap();
                                        assert_eq!(rules[0]["status"], 301);
                                        ("200 OK", r#"{"version":6,"rules":[]}"#)
                                    }
                                    ("POST", _) => {
                                        let rule_in: serde_json::Value =
                                            serde_json::from_slice(&body).unwrap();
                                        assert_eq!(rule_in["source"], "/old");
                                        ("200 OK", rule_body)
                                    }
                                    ("DELETE", p) if p.ends_with("/redirects/rdr_missing") => {
                                        ("404 Not Found", r#"{"error":"redirect not found"}"#)
                                    }
                                    ("DELETE", _) => ("200 OK", r#"{"version":7,"rules":[]}"#),
                                    _ => ("200 OK", rule_body),
                                }
                            } else if method == "GET" && path == "/v0/projects" {
                                (
                                    "200 OK",
                                    r#"[{"id":"prj_mock","name":"mock-app","slug":"mock-app","organizationId":"org_mock","createdAt":1762732800}]"#,
                                )
                            } else if method == "GET" && path.ends_with("/aliases") {
                                (
                                    "200 OK",
                                    r#"[{"host":"app.example.com","deploymentId":"dpl_abc123def456","versionId":"ver_abc123def456","createdAt":1762732800000,"isProduction":true},{"host":"beta.example.com","deploymentId":"dpl_abc123def456","versionId":"ver_abc123def456","createdAt":1762732900000,"isProduction":false}]"#,
                                )
                            } else if method == "DELETE"
                                && path.ends_with("/aliases/beta.example.com")
                            {
                                ("204 No Content", "")
                            } else if method == "DELETE" && path.contains("/aliases/") {
                                (
                                    "409 Conflict",
                                    r#"{"error":"the production host cannot be removed"}"#,
                                )
                            } else if method == "GET"
                                && path.starts_with("/v0/projects/")
                                && !path.contains("/env")
                                && !path.contains("/domains")
                                && !path.contains("/deployments")
                            {
                                (
                                    "200 OK",
                                    r#"{"id":"prj_mock","name":"mock-app","slug":"mock-app","organizationId":"org_mock","createdAt":1762732800}"#,
                                )
                            } else if method == "GET" && path.starts_with("/v0/activity") {
                                (
                                    "200 OK",
                                    r#"{"items":[{"id":"evt_1","organizationId":"org_1","actorUserId":"usr_1","actorKeyId":null,"actorType":"user","action":"project.create","resourceType":"project","resourceId":"prj_mock","ip":null,"createdAt":1762732800000}],"nextCursor":"cur_9"}"#,
                                )
                            } else if method == "GET" && path == "/v0/usage" {
                                (
                                    "200 OK",
                                    r#"[{"label":"Projects","used":9,"limit":10,"detail":"per org"}]"#,
                                )
                            } else if method == "GET" && path == "/v0/entitlements" {
                                ("200 OK", r#"{"entitlements":["custom-domains"]}"#)
                            } else if method == "POST" && path == "/v0/projects/mock-app/restore" {
                                (
                                    "200 OK",
                                    r#"{"id":"prj_mock","name":"renamed-app","slug":"mock-app","createdAt":1762732800000,"pausedAt":null,"expiresAt":null}"#,
                                )
                            } else if method == "POST" && path == "/v0/projects/missing/restore" {
                                (
                                    "404 Not Found",
                                    r#"{"error":"no restorable project with that name"}"#,
                                )
                            } else if method == "PATCH" && path.starts_with("/v0/projects/") {
                                (
                                    "200 OK",
                                    r#"{"id":"prj_mock","name":"renamed-app","slug":"mock-app","organizationId":"org_mock","createdAt":1762732800}"#,
                                )
                            } else if method == "POST" && path == "/v0/projects" {
                                (
                                    "200 OK",
                                    r#"{"id":"prj_new_mock","name":"new-app","slug":"new-app","organizationId":"org_mock","createdAt":1762732800}"#,
                                )
                            } else if method == "DELETE" && path == "/v0/deployments/dpl_mock_1" {
                                ("204 No Content", "")
                            } else if method == "DELETE" && path == "/v0/deployments/dpl_aliased" {
                                (
                                    "409 Conflict",
                                    r#"{"error":"deployment is aliased by app.example.com"}"#,
                                )
                            } else if method == "DELETE" && path.starts_with("/v0/projects/") {
                                ("200 OK", r#"{"status":"deleted","id":"prj_mock"}"#)
                            } else {
                                ("404 Not Found", r#"{"error":"Not found"}"#)
                            };

                            let response = format!(
                                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                status,
                                resp_body.len(),
                                resp_body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                addr,
                shutdown,
                handle: Some(handle),
            }
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    #[test]
    fn deployment_params_encode_filters() {
        let p = deployment_params(&DeploymentQuery {
            production: true,
            environment: Some("staging".into()),
            status: vec!["ready".into(), "error".into()],
            since: Some(10),
            until: Some(20),
            meta: vec!["a=b=c".into()],
            ..Default::default()
        })
        .unwrap();
        let pairs: Vec<(&str, &str)> = p.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(
            pairs,
            [
                ("type", "production"),
                ("environment", "staging"),
                ("status", "ready"),
                ("status", "error"),
                ("since", "10"),
                ("until", "20"),
                ("meta.a", "b=c"),
            ]
        );
        assert!(
            deployment_params(&DeploymentQuery {
                meta: vec!["nokv".into()],
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn test_appzdev_mock_end_to_end_flow() {
        let server = MockServer::start();
        let dir = tempdir().unwrap();

        // Scaffold a mock project with dist output
        let dist = dir.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(
            dist.join("index.html"),
            r#"<script src="/app.js"></script>"#,
        )
        .unwrap();
        std::fs::write(dist.join("app.js"), "console.log('test');").unwrap();

        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env
            .insert("APPZ_API_URL".to_string(), server.addr.clone());
        ctx.run_env
            .insert("APPZ_TOKEN".to_string(), "mock_token_123".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "prj_mock".to_string());

        let provider = AppzdevProvider;

        // 1. Deploy
        let deploy_out = provider.deploy(&ctx).expect("deploy must succeed");
        assert_eq!(deploy_out.provider, "appzdev");
        assert_eq!(deploy_out.url.as_deref(), Some("https://mock.appz.page"));
        assert_eq!(deploy_out.status, DeployStatus::Ready);

        // 2. Inspect
        let inspect_out = provider
            .inspect_deployment(&ctx, "dpl_mock_1")
            .expect("inspect must succeed");
        assert_eq!(inspect_out.id, "dpl_mock_1");
        assert_eq!(inspect_out.project_id, "prj_mock");
        assert_eq!(inspect_out.version_id, "ver_mock_1");
        assert_eq!(inspect_out.file_count, 2);
        assert_eq!(inspect_out.files.as_ref().unwrap().len(), 2);

        // 3. List
        let list_out = provider.list_deployments(&ctx).expect("list must succeed");
        assert_eq!(list_out.len(), 1);
        assert_eq!(list_out[0].id, "dpl_mock_1");

        // 3b. Team-wide pages
        let page = provider
            .list_team_deployments(
                &ctx,
                &DeploymentQuery {
                    project: Some("prj_mock".into()),
                    production: true,
                    meta: vec!["k=v".into()],
                    limit: Some(5),
                    ..Default::default()
                },
            )
            .expect("team deployments must succeed");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].project_slug.as_deref(), Some("mock-app"));
        assert_eq!(page.next.as_deref(), Some("cur_2"));
        let all = provider
            .list_team_deployments(&ctx, &DeploymentQuery::default())
            .expect("team-wide deployments must succeed");
        assert!(all.items.is_empty() && all.next.is_none());
        assert!(
            provider
                .list_team_deployments(
                    &ctx,
                    &DeploymentQuery {
                        meta: vec!["nokv".into()],
                        ..Default::default()
                    },
                )
                .is_err()
        );
        let doms = provider
            .list_team_domains(&ctx, &DomainQuery::default())
            .expect("team domains must succeed");
        assert_eq!(doms.items[0].domain.domain, "custom.example.com");
        assert_eq!(doms.items[0].project_slug.as_deref(), Some("mock-app"));
        assert_eq!(doms.items[0].domain.created_at, Some(1762732800000));

        let found = provider
            .inspect_domain(&ctx, "Custom.Example.com")
            .unwrap()
            .expect("domain must be found");
        assert_eq!(found.project_slug.as_deref(), Some("mock-app"));
        assert!(
            provider
                .inspect_domain(&ctx, "nope.example.com")
                .unwrap()
                .is_none()
        );
        assert_eq!(found.domain.serving_status(), "serving");
        let moved = provider
            .move_domain(&ctx, "custom.example.com", "org_dest", "prj_dest")
            .unwrap();
        assert_eq!(moved.status, "completed");
        assert_eq!(moved.project_id, "prj_dest");
        let err = move_project_domain(
            &provider.api_base(&ctx),
            None,
            "prj_mock",
            "forbidden.example.com",
            "org_dest",
            "prj_dest",
        )
        .unwrap_err();
        assert!(err.to_string().contains("owner of both"), "{err}");

        // 4. Rollback
        provider
            .rollback(&ctx, "ver_mock_1")
            .expect("rollback must succeed");

        // 5. Alias
        let alias_out = provider
            .alias(&ctx, "custom.com")
            .expect("alias must succeed");
        assert!(alias_out.contains("custom.com"));

        // 6. Env pull
        let pulled_vars = pull_cloud_env(
            &server.addr,
            "prj_mock",
            Some("mock_token_123"),
            Some("production"),
        )
        .expect("env pull must succeed");
        assert_eq!(pulled_vars.len(), 1);
        assert_eq!(pulled_vars[0].key, "MOCK_VAR");
        assert_eq!(pulled_vars[0].value, "test_val_123");

        let meta = list_cloud_env(&server.addr, "prj_mock", Some("mock_token_123"), None)
            .expect("env list must succeed");
        assert_eq!(meta[0].key, "MOCK_VAR");
        assert_eq!(meta[0].git_branch.as_deref(), Some("main"));
        assert!(!serde_json::to_string(&meta).unwrap().contains("test_val"));

        // 6b. Custom environments
        let envs = list_project_environments(&server.addr, "prj_mock", Some("mock_token_123"))
            .expect("list environments must succeed");
        assert_eq!(envs[0].slug, "staging");
        assert_eq!(envs[0].branch_pattern.as_deref(), Some("release/*"));
        let created = add_project_environment(
            &server.addr,
            "prj_mock",
            Some("mock_token_123"),
            "staging",
            None,
        )
        .expect("create environment must succeed");
        assert_eq!(created.id, "pen_abcdefghijkl");
        assert!(add_project_environment(&server.addr, "prj_mock", None, "Bad Slug", None).is_err());
        remove_project_environment(&server.addr, "prj_mock", Some("mock_token_123"), "staging")
            .expect("remove environment must succeed");

        // 7. Env push
        let push_count = push_cloud_env(
            &server.addr,
            "prj_mock",
            Some("mock_token_123"),
            &[CloudEnvVar {
                id: None,
                key: "NEW_VAR".to_string(),
                value: "new_val".to_string(),
                target: vec!["production".to_string()],
                git_branch: None,
                created_at: None,
            }],
        )
        .expect("env push must succeed");
        assert_eq!(push_count, 1);

        // 8. Custom Domains (DF-008, live API schema)
        let added_domain = provider
            .add_domain(&ctx, "custom.example.com")
            .expect("add domain must succeed");
        assert_eq!(added_domain.domain, "custom.example.com");
        assert_eq!(added_domain.display_status(), "pending_verification");
        assert!(!added_domain.verified);
        assert!(added_domain.challenge.is_some());
        assert_eq!(added_domain.challenge.as_ref().unwrap().r#type, "TXT");
        assert!(added_domain.cname.is_some());
        assert_eq!(added_domain.cname.as_ref().unwrap().target, "cname.appz.to");
        assert!(added_domain.apex.is_some());

        let dom_list = provider
            .list_domains(&ctx)
            .expect("list domains must succeed");
        assert_eq!(dom_list.len(), 1);
        assert_eq!(dom_list[0].domain, "custom.example.com");
        assert_eq!(dom_list[0].display_status(), "active");

        let verify_res = provider
            .verify_domain(&ctx, "custom.example.com")
            .expect("verify domain must succeed");
        assert_eq!(verify_res.display_status(), "active");
        assert!(verify_res.verified);

        let removed = provider
            .remove_domain(&ctx, "custom.example.com")
            .expect("remove domain must succeed");
        assert!(removed);

        let deleted_env =
            delete_cloud_env_var(&server.addr, "prj_mock", Some("mock_token_123"), "MOCK_VAR")
                .expect("delete env var must succeed");
        assert!(deleted_env);

        // 9. Projects
        let projects = provider
            .list_projects(&ctx)
            .expect("list projects must succeed");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "mock-app");
        assert_eq!(projects[0].slug, "mock-app");

        let got_proj = provider
            .get_project(&ctx, "mock-app")
            .expect("get project must succeed");
        assert_eq!(got_proj.name, "mock-app");
        assert_eq!(got_proj.id, "prj_mock");

        let renamed_proj = provider
            .rename_project(&ctx, "mock-app", "renamed-app")
            .expect("rename project must succeed");
        assert_eq!(renamed_proj.name, "renamed-app");

        let added_proj = provider
            .add_project(&ctx, "new-app")
            .expect("add project must succeed");
        assert_eq!(added_proj.name, "new-app");
        assert_eq!(added_proj.slug, "new-app");

        let removed_proj = provider
            .remove_project(&ctx, "mock-app")
            .expect("remove project must succeed");
        assert!(removed_proj);
        let activity = provider.list_activity(&ctx, Some(5), None).unwrap();
        assert_eq!(activity.items[0].action, "project.create");
        assert_eq!(activity.next_cursor.as_deref(), Some("cur_9"));
        let report = provider.usage_report(&ctx).unwrap();
        assert_eq!(report.usage[0].used, 9.0);
        assert_eq!(report.entitlements, ["custom-domains"]);
        assert_eq!(
            provider
                .remove_deployment(&ctx, "https://dpl_mock_1.appz.app/")
                .unwrap(),
            "dpl_mock_1"
        );
        let err = provider
            .remove_deployment(&ctx, "dpl_aliased")
            .unwrap_err()
            .to_string();
        assert!(err.contains("aliased by app.example.com") && err.contains("appz alias rm"));
        let err = provider
            .remove_deployment(&ctx, "dpl_gone")
            .unwrap_err()
            .to_string();
        assert!(err.contains("404"));
        let err = provider
            .remove_deployment(&ctx, "my-app")
            .unwrap_err()
            .to_string();
        assert!(err.contains("appz projects remove"));
        let restored = provider.restore_project(&ctx, "mock-app").unwrap();
        assert_eq!(restored.id, got_proj.id);
        assert_eq!(restored.slug, got_proj.slug);
        let aliases = provider.list_aliases(&ctx).unwrap();
        assert_eq!(aliases.len(), 2);
        assert!(aliases[0].is_production && !aliases[1].is_production);
        assert_eq!(aliases[1].deployment_id, "dpl_abc123def456");
        provider.remove_alias(&ctx, "Beta.Example.com").unwrap();
        let err = provider.remove_alias(&ctx, "app.example.com").unwrap_err();
        assert!(
            err.to_string()
                .contains("production host cannot be removed")
        );
        let err = provider.restore_project(&ctx, "missing").unwrap_err();
        assert!(err.to_string().contains("no restorable project"));

        // 10. Redirects (staged versions)
        let rule = RedirectInput {
            source: "/old".into(),
            destination: "/new".into(),
            status: 308,
        };
        let added = provider.add_redirect(&ctx, &rule).unwrap();
        assert_eq!(added.version, Some(5));
        assert_eq!(added.rules[0].id, "rdr_1");
        let listed = provider.list_redirects(&ctx, None).unwrap();
        assert_eq!(listed.rules.len(), 1);
        assert!(
            provider
                .list_redirects(&ctx, Some(4))
                .unwrap()
                .rules
                .is_empty()
        );
        let imported = provider
            .import_redirects(
                &ctx,
                &[RedirectInput {
                    status: 301,
                    ..rule
                }],
            )
            .unwrap();
        assert_eq!(imported.version, Some(6));
        assert_eq!(
            provider.remove_redirect(&ctx, "rdr_1").unwrap().version,
            Some(7)
        );
        let err = provider.remove_redirect(&ctx, "rdr_missing").unwrap_err();
        assert!(err.to_string().contains("redirect not found"));
        assert!(provider.remove_redirect(&ctx, "../x").is_err());
        let versions = provider.list_redirect_versions(&ctx).unwrap();
        assert_eq!(
            (versions.live_version, versions.staged_version),
            (Some(4), Some(5))
        );
        assert_eq!(versions.versions.len(), 2);
        assert_eq!(
            provider
                .update_redirect_version(&ctx, 5, "promote")
                .unwrap()
                .version,
            5
        );
        assert_eq!(
            provider
                .update_redirect_version(&ctx, 3, "restore")
                .unwrap()
                .version,
            5
        );
    }

    #[test]
    fn move_domain_retries_202_with_the_same_idempotency_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut keys = Vec::new();
            for status in ["202 Accepted", "200 OK"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let key = req
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .starts_with("idempotency-key:")
                            .then(|| l.split_once(':').unwrap().1.trim().to_string())
                    })
                    .unwrap_or_default();
                keys.push(key);
                let body = format!(
                    r#"{{"id":"dom_1","domain":"a.example.com","projectId":"prj_dest","organizationId":"org_dest","status":"{}"}}"#,
                    if status.starts_with("202") {
                        "committed"
                    } else {
                        "completed"
                    }
                );
                let resp = format!(
                    "HTTP/1.1 {status}
Content-Type: application/json
Content-Length: {}
Connection: close

{body}",
                    body.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
                // Drain the rest of the request so closing doesn't RST the
                // response (macOS reports that as EINVAL on the client).
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let _ = stream.read_to_end(&mut Vec::new());
            }
            keys
        });
        let out = move_project_domain(
            &addr,
            None,
            "prj_src",
            "a.example.com",
            "org_dest",
            "prj_dest",
        )
        .unwrap();
        assert_eq!(out.status, "completed");
        let keys = server.join().unwrap();
        assert!(keys[0].starts_with("appz-move-"), "{keys:?}");
        assert_eq!(keys[0], keys[1]);
    }

    #[test]
    fn test_inline_publish_failure_fallback_commits() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();

        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = std::thread::spawn(move || {
            use std::io::{Read, Write};
            while !shutdown_clone.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 4096];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let req_str = String::from_utf8_lossy(&buf[..n]);
                        let first_line = req_str.lines().next().unwrap_or("");
                        let parts: Vec<&str> = first_line.split_whitespace().collect();
                        if parts.len() < 2 {
                            continue;
                        }
                        let method = parts[0];
                        let path = parts[1];

                        let (status, resp_body) = if method == "POST" && path == "/v0/deployments" {
                            (
                                "500 Internal Server Error",
                                r#"{"error":"publish failed; retry with POST /deployments/:id/commit","deploymentId":"dpl_inline_retry_1"}"#,
                            )
                        } else if method == "POST"
                            && path == "/v0/deployments/dpl_inline_retry_1/commit"
                        {
                            (
                                "200 OK",
                                r#"{"deploymentId":"dpl_inline_retry_1","versionId":"ver_retry_1","status":"ready","url":"https://mock-retry.appz.to"}"#,
                            )
                        } else {
                            ("404 Not Found", r#"{"error":"Not found"}"#)
                        };

                        let response = format!(
                            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status,
                            resp_body.len(),
                            resp_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        let dir = tempdir().unwrap();
        let dist = dir.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "<h1>Retry Test</h1>").unwrap();

        let mut ctx = DeployContext::new(dir.path().to_path_buf(), "dist".to_string());
        ctx.run_env.insert("APPZ_API_URL".to_string(), addr);
        ctx.run_env
            .insert("APPZ_TOKEN".to_string(), "mock_tok".to_string());
        ctx.run_env
            .insert("APPZ_PROJECT_ID".to_string(), "retry-site".to_string());

        let provider = AppzdevProvider;
        let out = provider
            .deploy(&ctx)
            .expect("deploy must succeed via fallback commit");
        assert_eq!(out.status, DeployStatus::Ready);
        assert_eq!(out.url.as_deref(), Some("https://mock-retry.appz.to"));

        shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
    }

    fn status_error(code: u16, body: &str) -> ureq::Error {
        ureq::Error::Status(code, ureq::Response::new(code, "status", body).unwrap())
    }

    #[test]
    fn mfa_required_401_reports_the_server_message() {
        let err = status_error(
            401,
            r#"{"error":"two-factor verification required","code":"MFA_REQUIRED"}"#,
        );
        match parse_api_error(err, "AppzDev", "List projects") {
            DeployError::Api { reason, .. } => {
                assert_eq!(reason, "two-factor verification required");
            }
            other => panic!("expected Api, got {other:?}"),
        }
        let err = status_error(401, r#"{"code":"MFA_REQUIRED"}"#);
        match parse_api_error(err, "AppzDev", "List projects") {
            DeployError::Api { reason, .. } => assert!(reason.contains("browser")),
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn plain_401_still_asks_for_login() {
        for body in [r#"{"error":"unauthorized"}"#, "not json"] {
            let err = parse_api_error(status_error(401, body), "AppzDev", "List projects");
            assert!(matches!(err, DeployError::AuthMissing { .. }), "{err:?}");
        }
    }
}

#[cfg(test)]
mod promote_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    /// Serves `responses` in order for successive requests; records raw requests.
    fn serve(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut s, _)) = listener.accept() else {
                    return;
                };
                // Read the whole request (headers and body) before replying.
                let mut raw = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    raw.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&raw).to_lowercase();
                    let Some(head_end) = text.find("\r\n\r\n") else {
                        if n == 0 { break } else { continue }
                    };
                    let len = text
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if n == 0 || raw.len() >= head_end + 4 + len {
                        break;
                    }
                }
                let n = raw.len();
                let buf = raw;
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (addr, seen)
    }

    fn ctx(addr: String) -> DeployContext {
        let mut ctx = DeployContext::new(std::env::temp_dir(), "dist".into());
        ctx.run_env.insert("APPZ_API_URL".into(), addr);
        ctx.run_env.insert("APPZ_TOKEN".into(), "tok".into());
        ctx
    }

    fn inspect_body(status: &str) -> (u16, String) {
        (
            200,
            format!(
                r#"{{"id":"dpl_abcdefghijkl","projectId":"prj_abcdefghijkl","versionId":"ver_abcdefghijkl","status":"{status}","type":"preview"}}"#
            ),
        )
    }

    const PROMOTED: &str = r#"{"status":"promoted","projectId":"prj_abcdefghijkl","deploymentId":"dpl_abcdefghijkl","versionId":"ver_abcdefghijkl","host":"site.appz.to"}"#;

    #[test]
    fn promote_posts_deployment_id_with_idempotency_key() {
        let (addr, seen) = serve(vec![inspect_body("ready"), (200, PROMOTED.into())]);
        let out = AppzdevProvider
            .promote_deployment(&ctx(addr), "https://dpl_abcdefghijkl.appz.to", &|_| true)
            .unwrap()
            .unwrap();
        assert_eq!(out.url, "https://site.appz.to");
        assert_eq!(out.version_id, "ver_abcdefghijkl");
        let reqs = seen.lock().unwrap();
        assert!(reqs[1].starts_with("POST /v0/projects/prj_abcdefghijkl/promote"));
        assert!(
            reqs[1]
                .to_lowercase()
                .contains("idempotency-key: appz-promote-dpl_abcdefghijkl-")
        );
        assert!(reqs[1].contains(r#"{"deploymentId":"dpl_abcdefghijkl"}"#));
    }

    #[test]
    fn promote_declined_makes_no_post() {
        let (addr, seen) = serve(vec![inspect_body("ready")]);
        let out = AppzdevProvider
            .promote_deployment(&ctx(addr), "dpl_abcdefghijkl", &|_| false)
            .unwrap();
        assert!(out.is_none());
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn promote_rejects_deployments_that_are_not_ready() {
        let (addr, seen) = serve(vec![inspect_body("building")]);
        let err = AppzdevProvider
            .promote_deployment(&ctx(addr), "dpl_abcdefghijkl", &|_| true)
            .unwrap_err();
        assert!(err.to_string().contains("only ready deployments"), "{err}");
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn promote_retries_server_errors_with_the_same_key() {
        let (addr, seen) = serve(vec![
            inspect_body("ready"),
            (503, r#"{"error":"busy"}"#.into()),
            (200, PROMOTED.into()),
        ]);
        AppzdevProvider
            .promote_deployment(&ctx(addr), "dpl_abcdefghijkl", &|_| true)
            .unwrap()
            .unwrap();
        let reqs = seen.lock().unwrap();
        let key = |r: &str| {
            r.lines()
                .find(|l| l.to_lowercase().starts_with("idempotency-key:"))
                .map(str::to_owned)
        };
        assert!(key(&reqs[1]).is_some());
        assert_eq!(key(&reqs[1]), key(&reqs[2]));
    }

    #[test]
    fn promote_does_not_retry_client_errors() {
        let (addr, seen) = serve(vec![
            inspect_body("ready"),
            (409, r#"{"error":"conflict"}"#.into()),
        ]);
        assert!(
            AppzdevProvider
                .promote_deployment(&ctx(addr), "dpl_abcdefghijkl", &|_| true)
                .is_err()
        );
        assert_eq!(seen.lock().unwrap().len(), 2);
    }
}
