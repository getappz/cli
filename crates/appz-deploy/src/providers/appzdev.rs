//! AppzDev / Cloudflare Edge native deploy provider.
//!
//! Direct edge-native CAS deployment to the Cloudflare runtime (designflare/worker/serve).
//! Features:
//! - Multi-threaded SHA-256 Merkle tree calculation
//! - Delta-check: only missing hashes are uploaded (zero duplicate transfer)
//! - Bounded concurrency blob uploads with retries
//! - Atomic immutable deployment to R2 MANIFESTS
//! - Sub-second instant rollback via KV HOSTS pointer mutation

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::context::DeployContext;
use crate::error::{DeployError, DeployResult};
use crate::output::{
    DeployOutput, DeployStatus, DeploymentInfo, DeploymentInspect, DetectedConfig,
    DomainVerifyResult, PrerequisiteStatus, ProjectDomain, ProjectInfo,
};
use crate::provider::DeployProvider;
use crate::static_cas::scan_output_dir;

pub struct AppzdevProvider;

const DEFAULT_API_URL: &str = "https://api.appz.dev";

/// Normalizes a project id or name into a valid edge project slug:
/// - Values starting with `prj_` are preserved untouched.
/// - Otherwise: lowercase, replace non `[a-z0-9]` runs with `-`, trim `-`, cap at 40 chars.
/// - Falls back to `default-project` if empty.
pub fn slugify_project_id(raw: &str) -> String {
    if raw.starts_with("prj_") {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let capped = if trimmed.len() > 40 {
        &trimmed[..40]
    } else {
        trimmed
    };
    let final_slug = capped.trim_end_matches('-');
    if final_slug.is_empty() {
        "default-project".to_string()
    } else {
        final_slug.to_string()
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
    let p = auth_file_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(auth).map_err(std::io::Error::other)?;
    std::fs::write(p, data)
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

pub fn parse_api_error(
    err: ureq::Error,
    provider_name: &'static str,
    operation: &str,
) -> DeployError {
    match err {
        ureq::Error::Status(code, resp) => {
            if code == 401 {
                return DeployError::AuthMissing {
                    provider: provider_name,
                    env_var: "APPZ_TOKEN",
                    login_hint: "run `appz login` or set APPZ_TOKEN".to_string(),
                };
            }
            if code == 409 && operation.contains("Verify") {
                return DeployError::DeployFailed {
                    provider: provider_name,
                    reason: "DNS ownership TXT record not propagated yet. Please ensure the DNS challenge TXT record is configured and wait a few minutes before re-checking.".into(),
                };
            }
            if code == 503 && operation.contains("domain") {
                return DeployError::DeployFailed {
                    provider: provider_name,
                    reason: "Custom domains are not configured on the server.".into(),
                };
            }
            if code == 502 && operation.contains("domain") {
                return DeployError::DeployFailed {
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
                DeployError::DeployFailed {
                    provider: provider_name,
                    reason: format!("{operation} failed ({code}): {msg}"),
                }
            } else {
                DeployError::DeployFailed {
                    provider: provider_name,
                    reason: format!("{operation} failed with HTTP status {code}"),
                }
            }
        }
        ureq::Error::Transport(e) => DeployError::DeployFailed {
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
        .map_err(|e| DeployError::DeployFailed {
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
    let folder = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "default-project".to_string());
    slugify_project_id(&folder)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudEnvVar {
    #[serde(default)]
    pub id: Option<String>,
    pub key: String,
    pub value: String,
    #[serde(default = "default_targets")]
    pub target: Vec<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
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
            let key = k.trim().to_string();
            let mut val = v.trim().to_string();
            let is_quoted = (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''));
            if is_quoted && val.len() >= 2 {
                val = val[1..val.len() - 1].to_string();
            }
            if !key.is_empty() {
                out.push((key, val));
            }
        }
    }
    out
}

pub fn format_env_file(vars: &[(String, String)]) -> String {
    let mut lines = Vec::new();
    for (k, v) in vars {
        if v.contains(' ') || v.contains('\n') || v.contains('"') || v.is_empty() {
            let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
            lines.push(format!("{k}=\"{escaped}\""));
        } else {
            lines.push(format!("{k}={v}"));
        }
    }
    lines.join("\n") + "\n"
}

pub fn pull_cloud_env(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    target: Option<&str>,
) -> DeployResult<Vec<CloudEnvVar>> {
    let mut url = format!("{api_base}/api/v0/projects/{project_id}/env");
    if let Some(t) = target {
        url.push_str(&format!("?target={t}"));
    }
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Pull environment variables"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from pull env: {e}"),
        })?;

    let items = if let Some(arr) = resp.as_array() {
        arr
    } else if let Some(arr) = resp["env"].as_array() {
        arr
    } else {
        return Err(DeployError::DeployFailed {
            provider: "AppzDev",
            reason: "Expected array of environment variables".into(),
        });
    };

    let vars: Vec<CloudEnvVar> = serde_json::from_value(serde_json::Value::Array(items.clone()))
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Failed to parse cloud env vars: {e}"),
        })?;

    Ok(vars)
}

pub fn push_cloud_env(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    vars: &[CloudEnvVar],
) -> DeployResult<usize> {
    let url = format!("{api_base}/api/v0/projects/{project_id}/env");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({ "env": vars }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Push environment variables"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
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
    let url = format!("{api_base}/api/v0/projects/{project_id}/env/{key}");
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
    let d = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let ok = !d.is_empty()
        && d.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '*');
    if !ok {
        return Err(DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid domain name: {domain:?}"),
        });
    }
    Ok(d)
}

pub fn add_project_domain(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<ProjectDomain> {
    let domain = normalize_domain(domain)?;
    let domain = domain.as_str();
    let url = format!("{api_base}/api/v0/projects/{project_id}/domains");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({ "domain": domain }))
        .map_err(|e| parse_api_error(e, "AppzDev", "Add custom domain"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from add domain: {e}"),
        })?;

    serde_json::from_value(resp).map_err(|e| DeployError::DeployFailed {
        provider: "AppzDev",
        reason: format!("Failed to parse added domain record: {e}"),
    })
}

pub fn list_project_domains(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
) -> DeployResult<Vec<ProjectDomain>> {
    let url = format!("{api_base}/api/v0/projects/{project_id}/domains");
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "List custom domains"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from list domains: {e}"),
        })?;

    let items = if resp.is_array() {
        resp
    } else if let Some(arr) = resp.get("domains").filter(|v| v.is_array()) {
        arr.clone()
    } else {
        return Err(DeployError::DeployFailed {
            provider: "AppzDev",
            reason: "Expected array of domain records".into(),
        });
    };

    serde_json::from_value(items).map_err(|e| DeployError::DeployFailed {
        provider: "AppzDev",
        reason: format!("Failed to parse domain list: {e}"),
    })
}

pub fn verify_project_domain(
    api_base: &str,
    project_id: &str,
    token: Option<&str>,
    domain: &str,
) -> DeployResult<DomainVerifyResult> {
    let domain = normalize_domain(domain)?;
    let domain = domain.as_str();
    let url = format!("{api_base}/api/v0/projects/{project_id}/domains/{domain}/verify");
    let mut req = ureq::post(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .send_json(serde_json::json!({}))
        .map_err(|e| parse_api_error(e, "AppzDev", "Verify custom domain"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from verify domain: {e}"),
        })?;

    serde_json::from_value(resp).map_err(|e| DeployError::DeployFailed {
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
    let url = format!("{api_base}/api/v0/projects/{project_id}/domains/{domain}");
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let _ = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Remove custom domain"))?;

    Ok(true)
}

pub fn list_projects(api_base: &str, token: Option<&str>) -> DeployResult<Vec<ProjectInfo>> {
    let url = format!("{api_base}/api/v0/projects");
    let mut req = ureq::get(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp: serde_json::Value = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "List projects"))?
        .into_json()
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from list projects: {e}"),
        })?;

    let items = if resp.is_array() {
        resp
    } else if let Some(arr) = resp.get("projects").filter(|v| v.is_array()) {
        arr.clone()
    } else {
        return Err(DeployError::DeployFailed {
            provider: "AppzDev",
            reason: "Expected array of projects".into(),
        });
    };

    serde_json::from_value(items).map_err(|e| DeployError::DeployFailed {
        provider: "AppzDev",
        reason: format!("Failed to parse project list: {e}"),
    })
}

pub fn add_project(api_base: &str, token: Option<&str>, name: &str) -> DeployResult<ProjectInfo> {
    let url = format!("{api_base}/api/v0/projects");
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
        .map_err(|e| DeployError::DeployFailed {
            provider: "AppzDev",
            reason: format!("Invalid JSON response from add project: {e}"),
        })?;

    let val = if resp.get("project").is_some() {
        resp["project"].clone()
    } else {
        resp
    };

    serde_json::from_value(val).map_err(|e| DeployError::DeployFailed {
        provider: "AppzDev",
        reason: format!("Failed to parse created project: {e}"),
    })
}

pub fn remove_project(api_base: &str, token: Option<&str>, id_or_slug: &str) -> DeployResult<bool> {
    let url = format!("{api_base}/api/v0/projects/{id_or_slug}");
    let mut req = ureq::delete(&url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let _ = req
        .call()
        .map_err(|e| parse_api_error(e, "AppzDev", "Remove project"))?;

    Ok(true)
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
        let scanned = scan_output_dir(&output_path).map_err(|e| DeployError::DeployFailed {
            provider: self.name(),
            reason: e,
        })?;

        if scanned.files.is_empty() {
            return Err(DeployError::DeployFailed {
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

        let payload = serde_json::json!({
            "projectId": project_id,
            "target": target,
            "files": scanned.files,
        });

        // Step 1: POST /api/v0/deployments - Handshake & missing blob diff
        let create_url = format!("{}/api/v0/deployments", api_base);
        let mut req = ureq::post(&create_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(payload)
            .map_err(|e| parse_api_error(e, self.name(), "Initiate deployment"))?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON response from {create_url}: {e}"),
            })?;

        let deployment_id = resp["deploymentId"]
            .as_str()
            .or_else(|| resp["id"].as_str())
            .ok_or_else(|| DeployError::DeployFailed {
                provider: self.name(),
                reason: "Missing deploymentId in response".into(),
            })?;

        let missing = resp["missing"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Step 2: Upload missing blobs with bounded concurrency (pool of 8) and retry
        if !missing.is_empty() {
            let missing_blobs: Vec<(&str, &[u8])> = missing
                .iter()
                .filter_map(|h| {
                    scanned
                        .blobs_by_hash
                        .get(h)
                        .map(|b| (h.as_str(), b.data.as_slice()))
                })
                .collect();

            let worker_count = 8.min(missing_blobs.len()).max(1);
            let next_idx = AtomicUsize::new(0);
            let upload_error = Mutex::new(None::<String>);

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
                            let (hash, data) = missing_blobs[idx];

                            let mut last_err = None;
                            for attempt in 0..3 {
                                if attempt > 0 {
                                    std::thread::sleep(Duration::from_millis(200 * (1 << attempt)));
                                }
                                let upload_url = format!(
                                    "{}/api/v0/deployments/{}/files/{}",
                                    api_base, deployment_id, hash
                                );
                                let mut up_req = ureq::put(&upload_url)
                                    .set("Content-Type", "application/octet-stream");
                                if let Some(token) = &token_opt {
                                    up_req =
                                        up_req.set("Authorization", &format!("Bearer {token}"));
                                }
                                match up_req.send_bytes(data) {
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
                return Err(DeployError::DeployFailed {
                    provider: self.name(),
                    reason: err,
                });
            }
        }

        // Step 3: Commit deployment if blobs were uploaded; when missing is empty the deployment is already published
        let final_url = if !missing.is_empty() {
            let commit_url = format!("{}/api/v0/deployments/{}/commit", api_base, deployment_id);
            let mut commit_req = ureq::post(&commit_url);
            if let Some(token) = &token_opt {
                commit_req = commit_req.set("Authorization", &format!("Bearer {token}"));
            }
            let commit_resp: serde_json::Value = commit_req
                .send_json(serde_json::json!({}))
                .map_err(|e| parse_api_error(e, self.name(), "Commit deployment"))?
                .into_json()
                .map_err(|e| DeployError::DeployFailed {
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
            unchanged: missing.is_empty(),
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
            let content =
                std::fs::read_to_string(&project_json).map_err(|e| DeployError::DeployFailed {
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
        std::fs::create_dir_all(&appz_dir).map_err(|e| DeployError::DeployFailed {
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
        .map_err(|e| DeployError::DeployFailed {
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
        let rollback_url = format!("{}/api/v0/projects/{}/rollback", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::post(&rollback_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "versionId": version_id }))
            .map_err(|e| parse_api_error(e, self.name(), "Rollback deployment"))?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
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
        })
    }

    fn alias(&self, ctx: &DeployContext, domain: &str) -> DeployResult<String> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let alias_url = format!("{}/api/v0/projects/{}/alias", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::post(&alias_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .send_json(serde_json::json!({ "domain": domain }))
            .map_err(|e| parse_api_error(e, self.name(), "Alias domain"))?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid alias response: {e}"),
            })?;

        let bound_domain = resp["domain"].as_str().unwrap_or(domain);
        Ok(format!("https://{bound_domain}"))
    }

    fn list_deployments(&self, ctx: &DeployContext) -> DeployResult<Vec<DeploymentInfo>> {
        let project_id = self.resolve_project_id(ctx);
        let api_base = self.api_base(ctx);
        let list_url = format!("{}/api/v0/projects/{}/deployments", api_base, project_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::get(&list_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| parse_api_error(e, self.name(), "List deployments"))?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON from list deployments: {e}"),
            })?;

        let items = resp.as_array().ok_or_else(|| DeployError::DeployFailed {
            provider: self.name(),
            reason: "Expected JSON array of deployments".into(),
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

        let inspect_url = format!("{}/api/v0/deployments/{}", api_base, dpl_id);
        let token_opt = resolve_auth_token(Some(ctx));

        let mut req = ureq::get(&inspect_url);
        if let Some(token) = &token_opt {
            req = req.set("Authorization", &format!("Bearer {token}"));
        }

        let resp: serde_json::Value = req
            .call()
            .map_err(|e| parse_api_error(e, self.name(), "Inspect deployment"))?
            .into_json()
            .map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Invalid JSON from inspect deployment: {e}"),
            })?;

        let inspect: DeploymentInspect =
            serde_json::from_value(resp).map_err(|e| DeployError::DeployFailed {
                provider: self.name(),
                reason: format!("Failed to parse deployment inspect: {e}"),
            })?;

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

    fn add_project(&self, ctx: &DeployContext, name: &str) -> DeployResult<ProjectInfo> {
        let api_base = self.api_base(ctx);
        let token = resolve_auth_token(Some(ctx));
        add_project(&api_base, token.as_deref(), name)
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
    }

    #[test]
    fn test_link_uses_stored_org_when_present() {
        let dir = tempdir().unwrap();
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
                            loop {
                                let mut line = String::new();
                                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                                    break;
                                }
                                let lower = line.to_lowercase();
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

                            let (status, resp_body) = if method == "POST"
                                && path == "/api/v0/deployments"
                            {
                                (
                                    "200 OK",
                                    r#"{"deploymentId":"dpl_mock_1","id":"dpl_mock_1","url":"https://mock.appz.page","status":"ready","missing":[]}"#,
                                )
                            } else if method == "GET"
                                && path.starts_with("/api/v0/deployments/dpl_mock_1")
                            {
                                (
                                    "200 OK",
                                    r#"{"id":"dpl_mock_1","projectId":"prj_mock","versionId":"ver_mock_1","status":"ready","target":"production","hosts":["mock.appz.page"],"fileCount":2,"totalBytes":128,"files":[{"path":"/index.html","size":64,"hash":"h1"},{"path":"/app.js","size":64,"hash":"h2"}]}"#,
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
                            } else if method == "GET" && path.contains("/env") {
                                (
                                    "200 OK",
                                    r#"[{"id":"env_1","key":"MOCK_VAR","value":"test_val_123","target":["production"],"gitBranch":null,"createdAt":1762732800}]"#,
                                )
                            } else if method == "POST" && path.contains("/env") {
                                ("200 OK", r#"{"status":"synced","count":1}"#)
                            } else if method == "DELETE" && path.contains("/env/") {
                                ("204 No Content", "")
                            } else if method == "GET" && path == "/api/v0/projects" {
                                (
                                    "200 OK",
                                    r#"[{"id":"prj_mock","name":"mock-app","slug":"mock-app","organizationId":"org_mock","createdAt":1762732800}]"#,
                                )
                            } else if method == "POST" && path == "/api/v0/projects" {
                                (
                                    "200 OK",
                                    r#"{"id":"prj_new_mock","name":"new-app","slug":"new-app","organizationId":"org_mock","createdAt":1762732800}"#,
                                )
                            } else if method == "DELETE" && path.starts_with("/api/v0/projects/") {
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
    fn test_appzdev_mock_end_to_end_flow() {
        let server = MockServer::start();
        let dir = tempdir().unwrap();

        // Scaffold a mock project with dist output
        let dist = dir.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "<h1>Hello Appz</h1>").unwrap();
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

        let added_proj = provider
            .add_project(&ctx, "new-app")
            .expect("add project must succeed");
        assert_eq!(added_proj.name, "new-app");
        assert_eq!(added_proj.slug, "new-app");

        let removed_proj = provider
            .remove_project(&ctx, "mock-app")
            .expect("remove project must succeed");
        assert!(removed_proj);
    }
}
