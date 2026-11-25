//! appz.dev project lifecycle (pause, resume, keep, settings) and
//! cross-organization transfers.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{parse_api_error, validate_project_ref};
use crate::error::{DeployError, DeployResult};

const PROVIDER: &str = "AppzDev";

/// Project as returned by `GET/PATCH /v0/projects/{id}` and the lifecycle
/// endpoints. Timestamps are epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub paused_at: Option<i64>,
    #[serde(default)]
    pub pause_reason: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub production_url: Option<String>,
    #[serde(default)]
    pub toolbar_mode: Option<String>,
    #[serde(default)]
    pub markdown: Option<bool>,
    #[serde(default)]
    pub deployment_protection_disabled: Option<bool>,
}

/// Fields accepted by `PATCH /v0/projects/{id}`; unset fields are not sent.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettingsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toolbar_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_protection_disabled: Option<bool>,
}

impl ProjectSettingsUpdate {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPreview {
    pub deployments: u64,
    pub domains: u64,
    pub environment_keys: u64,
    pub incremental_bytes: u64,
    pub revoked_keys: u64,
    #[serde(default)]
    pub alias_hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub id: String,
    pub project_id: String,
    pub source_organization_id: String,
    pub target_organization_id: String,
    pub status: String,
    pub expires_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<TransferPreview>,
}

fn failed(reason: String) -> DeployError {
    DeployError::Api {
        provider: PROVIDER,
        reason,
    }
}

/// `parse_api_error`, with a clear message for permission failures.
fn api_error(err: ureq::Error, op: &str) -> DeployError {
    if let ureq::Error::Status(403, resp) = err {
        let detail = resp
            .into_json::<serde_json::Value>()
            .ok()
            .and_then(|j| j.get("error").and_then(|e| e.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "forbidden".into());
        return failed(format!(
            "{op} was refused ({detail}). This needs a team owner or admin of the project's organization."
        ));
    }
    parse_api_error(err, PROVIDER, op)
}

fn send(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    op: &str,
) -> DeployResult<(u16, serde_json::Value)> {
    let mut req = ureq::request(method, url);
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let resp = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    }
    .map_err(|e| api_error(e, op))?;
    let status = resp.status();
    let json = resp
        .into_json()
        .map_err(|e| failed(format!("Invalid JSON response from {op}: {e}")))?;
    Ok((status, json))
}

fn parse<T: serde::de::DeserializeOwned>(v: serde_json::Value, what: &str) -> DeployResult<T> {
    serde_json::from_value(v).map_err(|e| failed(format!("Failed to parse {what}: {e}")))
}

pub fn get_project_state(
    api_base: &str,
    token: Option<&str>,
    id: &str,
) -> DeployResult<ProjectState> {
    validate_project_ref(id)?;
    let url = format!("{api_base}/v0/projects/{id}");
    parse(send("GET", &url, token, None, "Get project")?.1, "project")
}

/// `action` is one of `pause`, `resume`, `keep`.
pub fn project_action(
    api_base: &str,
    token: Option<&str>,
    id: &str,
    action: &str,
) -> DeployResult<ProjectState> {
    validate_project_ref(id)?;
    let url = format!("{api_base}/v0/projects/{id}/{action}");
    let op = match action {
        "pause" => "Pause project",
        "resume" => "Resume project",
        _ => "Keep project",
    };
    parse(send("POST", &url, token, None, op)?.1, "project")
}

pub fn update_project(
    api_base: &str,
    token: Option<&str>,
    id: &str,
    update: &ProjectSettingsUpdate,
) -> DeployResult<ProjectState> {
    validate_project_ref(id)?;
    if update.is_empty() {
        return Err(failed("No settings to update".into()));
    }
    if let Some(name) = &update.name
        && (name.trim().is_empty() || name.trim().len() > 100)
    {
        return Err(failed(
            "Project name must be between 1 and 100 characters".into(),
        ));
    }
    let url = format!("{api_base}/v0/projects/{id}");
    let body = serde_json::to_value(update).map_err(|e| failed(e.to_string()))?;
    parse(
        send("PATCH", &url, token, Some(body), "Update project")?.1,
        "project",
    )
}

pub fn request_transfer(
    api_base: &str,
    token: Option<&str>,
    id: &str,
    to_org: &str,
) -> DeployResult<Transfer> {
    validate_project_ref(id)?;
    let to_org = to_org.trim();
    if to_org.is_empty() || to_org.len() > 200 {
        return Err(failed(
            "Target organization must be 1 to 200 characters".into(),
        ));
    }
    let url = format!("{api_base}/v0/projects/{id}/transfer");
    let body = serde_json::json!({ "toOrganizationId": to_org });
    parse(
        send("POST", &url, token, Some(body), "Transfer project")?.1,
        "transfer",
    )
}

pub fn list_transfers(api_base: &str, token: Option<&str>) -> DeployResult<Vec<Transfer>> {
    let url = format!("{api_base}/v0/transfers");
    parse(
        send("GET", &url, token, None, "List transfers")?.1,
        "transfers",
    )
}

pub fn get_transfer(api_base: &str, token: Option<&str>, id: &str) -> DeployResult<Transfer> {
    validate_project_ref(id)?;
    let url = format!("{api_base}/v0/transfers/{id}");
    parse(
        send("GET", &url, token, None, "Get transfer")?.1,
        "transfer",
    )
}

pub fn cancel_transfer(api_base: &str, token: Option<&str>, id: &str) -> DeployResult<Transfer> {
    validate_project_ref(id)?;
    let url = format!("{api_base}/v0/transfers/{id}");
    parse(
        send("DELETE", &url, token, None, "Cancel transfer")?.1,
        "transfer",
    )
}

/// Accept a transfer. The server answers 202 while it reconciles; repeat
/// the call until it answers 200, or give up after `timeout`.
pub fn accept_transfer(
    api_base: &str,
    token: Option<&str>,
    id: &str,
    timeout: Duration,
    interval: Duration,
) -> DeployResult<Transfer> {
    validate_project_ref(id)?;
    let url = format!("{api_base}/v0/transfers/{id}/accept");
    let deadline = Instant::now() + timeout;
    loop {
        let (status, body) = send("POST", &url, token, None, "Accept transfer")?;
        let transfer: Transfer = parse(body, "transfer")?;
        if status != 202 {
            return Ok(transfer);
        }
        if Instant::now() >= deadline {
            return Err(failed(format!(
                "Transfer {id} is still being reconciled after {}s; run `appz transfers accept {id}` again to finish it",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Serves `responses` in order, one per connection, and records
    /// `"METHOD path body"` for each request.
    fn serve(responses: Vec<(&'static str, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut len = 0;
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h == "\r\n" || h.is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf).unwrap();
                let mut parts = line.split_whitespace();
                let (m, p) = (parts.next().unwrap(), parts.next().unwrap());
                log.lock()
                    .unwrap()
                    .push(format!("{m} {p} {}", String::from_utf8_lossy(&buf)));
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (addr, seen)
    }

    const PROJECT: &str = r#"{"id":"prj_abc123def456","slug":"demo","name":"Demo","createdAt":1,"pausedAt":5,"expiresAt":null,"toolbarMode":"off","markdown":false,"deploymentProtectionDisabled":false}"#;
    const TRANSFER: &str = r#"{"id":"trf_abc123def456","projectId":"prj_abc123def456","sourceOrganizationId":"org_a","targetOrganizationId":"org_b","status":"pending","expiresAt":9,"preview":{"deployments":3,"domains":1,"environmentKeys":2,"incrementalBytes":10,"revokedKeys":4,"aliasHosts":["demo.appz.to"]}}"#;

    #[test]
    fn pause_posts_and_parses_state() {
        let (base, seen) = serve(vec![("200 OK", PROJECT)]);
        let p = project_action(&base, Some("t"), "demo", "pause").unwrap();
        assert_eq!(p.paused_at, Some(5));
        assert_eq!(p.expires_at, None);
        assert_eq!(seen.lock().unwrap()[0], "POST /v0/projects/demo/pause ");
    }

    #[test]
    fn conflict_message_is_surfaced() {
        let (base, _) = serve(vec![(
            "409 Conflict",
            r#"{"error":"project is already paused"}"#,
        )]);
        let err = project_action(&base, None, "demo", "pause").unwrap_err();
        assert!(err.to_string().contains("already paused"), "{err}");
    }

    #[test]
    fn update_sends_only_set_fields_and_explains_403() {
        let (base, seen) = serve(vec![
            ("200 OK", PROJECT),
            ("403 Forbidden", r#"{"error":"admins only"}"#),
        ]);
        let upd = ProjectSettingsUpdate {
            toolbar_mode: Some("previews".into()),
            deployment_protection_disabled: Some(true),
            ..Default::default()
        };
        update_project(&base, None, "demo", &upd).unwrap();
        let sent = seen.lock().unwrap()[0].clone();
        assert!(sent.starts_with("PATCH /v0/projects/demo "), "{sent}");
        assert!(sent.contains(r#""toolbarMode":"previews""#));
        assert!(sent.contains(r#""deploymentProtectionDisabled":true"#));
        assert!(!sent.contains("markdown") && !sent.contains("name"));
        let err = update_project(&base, None, "demo", &upd).unwrap_err();
        assert!(err.to_string().contains("team owner or admin"), "{err}");
        assert!(update_project(&base, None, "demo", &ProjectSettingsUpdate::default()).is_err());
    }

    #[test]
    fn transfer_request_returns_preview() {
        let (base, seen) = serve(vec![("201 Created", TRANSFER)]);
        let t = request_transfer(&base, None, "demo", "org_b").unwrap();
        let preview = t.preview.unwrap();
        assert_eq!((preview.deployments, preview.revoked_keys), (3, 4));
        assert!(seen.lock().unwrap()[0].ends_with(r#"{"toOrganizationId":"org_b"}"#));
    }

    #[test]
    fn accept_repeats_on_202_then_times_out() {
        let (base, seen) = serve(vec![("202 Accepted", TRANSFER), ("200 OK", TRANSFER)]);
        let t = accept_transfer(
            &base,
            None,
            "trf_abc123def456",
            Duration::from_secs(5),
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(t.id, "trf_abc123def456");
        assert_eq!(seen.lock().unwrap().len(), 2);

        let (base, _) = serve(vec![("202 Accepted", TRANSFER)]);
        let err = accept_transfer(
            &base,
            None,
            "trf_abc123def456",
            Duration::ZERO,
            Duration::from_millis(1),
        )
        .unwrap_err();
        assert!(err.to_string().contains("still being reconciled"), "{err}");
    }

    #[test]
    fn list_cancel_and_bad_ids() {
        let (base, seen) = serve(vec![
            (
                "200 OK",
                r#"[{"id":"trf_1","projectId":"p","sourceOrganizationId":"a","targetOrganizationId":"b","status":"pending","expiresAt":1}]"#,
            ),
            (
                "200 OK",
                r#"{"id":"trf_1","projectId":"p","sourceOrganizationId":"a","targetOrganizationId":"b","status":"cancelled","expiresAt":1}"#,
            ),
        ]);
        assert_eq!(list_transfers(&base, None).unwrap().len(), 1);
        assert_eq!(
            cancel_transfer(&base, None, "trf_1").unwrap().status,
            "cancelled"
        );
        assert_eq!(seen.lock().unwrap()[1], "DELETE /v0/transfers/trf_1 ");
        assert!(get_transfer(&base, None, "../x").is_err());
    }
}
