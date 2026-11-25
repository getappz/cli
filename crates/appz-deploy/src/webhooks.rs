//! appz.dev webhook endpoints (`/v0/webhooks`).

use serde::{Deserialize, Serialize};

use crate::error::{DeployError, DeployResult};
use crate::output::Page;
use crate::providers::appzdev::parse_api_error;

/// Event names accepted by `POST /v0/webhooks`.
pub const WEBHOOK_EVENTS: [&str; 8] = [
    "deployment.created",
    "deployment.ready",
    "deployment.error",
    "deployment.canceled",
    "deployment.promoted",
    "project.created",
    "project.deleted",
    "project.restored",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub disabled_at: Option<i64>,
    pub created_at: i64,
    /// Signing secret; present only in the response to `add`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

// Hand-written so a stray `{:?}` can never print the signing secret.
impl std::fmt::Debug for Webhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Webhook")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("events", &self.events)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDelivery {
    pub id: String,
    pub event_id: String,
    pub event: String,
    pub attempt: u32,
    /// `pending`, `ok` or `failed`.
    pub status: String,
    #[serde(default)]
    pub response_status: Option<u16>,
    #[serde(default)]
    pub next_attempt_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct WebhookQuery {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPage<T> {
    items: Vec<T>,
    #[serde(default)]
    next_cursor: Option<String>,
}

fn fail(reason: impl Into<String>) -> DeployError {
    DeployError::Api {
        provider: "AppzDev",
        reason: reason.into(),
    }
}

fn has_prefixed_id(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix).is_some_and(|rest| {
        rest.len() == 12
            && rest
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
    })
}

/// Webhook IDs look like `whk_` plus 12 lowercase alphanumerics.
pub fn validate_webhook_id(id: &str) -> DeployResult<()> {
    if has_prefixed_id(id, "whk_") {
        Ok(())
    } else {
        Err(fail(format!(
            "invalid webhook id '{id}': expected whk_ followed by 12 lowercase letters or digits"
        )))
    }
}

/// Validate `add` input before it reaches the API.
pub fn validate_new_webhook(
    url: &str,
    events: &[String],
    project: Option<&str>,
) -> DeployResult<()> {
    let parsed = url::Url::parse(url).map_err(|e| fail(format!("invalid webhook URL: {e}")))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(fail("webhook URL must be an https:// URL"));
    }
    if url.len() > 2048 {
        return Err(fail("webhook URL is longer than 2048 characters"));
    }
    if events.is_empty() {
        return Err(fail(format!(
            "at least one --event is required; choose from: {}",
            WEBHOOK_EVENTS.join(", ")
        )));
    }
    if let Some(bad) = events
        .iter()
        .find(|e| !WEBHOOK_EVENTS.contains(&e.as_str()))
    {
        return Err(fail(format!(
            "unknown event '{bad}'; choose from: {}",
            WEBHOOK_EVENTS.join(", ")
        )));
    }
    if let Some(p) = project
        && !has_prefixed_id(p, "prj_")
    {
        return Err(fail(format!(
            "invalid project id '{p}': --project takes a prj_ id"
        )));
    }
    Ok(())
}

fn send(
    method: &str,
    api_base: &str,
    path: &str,
    token: Option<&str>,
    query: &[(&str, String)],
    body: Option<serde_json::Value>,
    operation: &str,
) -> DeployResult<ureq::Response> {
    let mut req = ureq::request(method, &format!("{api_base}{path}"));
    for (k, v) in query {
        req = req.query(k, v);
    }
    if let Some(tok) = token {
        req = req.set("Authorization", &format!("Bearer {tok}"));
    }
    let res = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    res.map_err(|e| parse_api_error(e, "AppzDev", operation))
}

fn json<T: serde::de::DeserializeOwned>(res: ureq::Response, operation: &str) -> DeployResult<T> {
    res.into_json()
        .map_err(|e| fail(format!("Invalid response from {operation}: {e}")))
}

fn page_params(query: &WebhookQuery) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    if let Some(l) = query.limit {
        params.push(("limit", l.to_string()));
    }
    if let Some(c) = &query.cursor {
        params.push(("cursor", c.clone()));
    }
    params
}

fn into_page<T>(raw: RawPage<T>) -> Page<T> {
    Page {
        items: raw.items,
        next: raw.next_cursor,
    }
}

pub fn list_webhooks(
    api_base: &str,
    token: Option<&str>,
    query: &WebhookQuery,
) -> DeployResult<Page<Webhook>> {
    let op = "List webhooks";
    let params = page_params(query);
    let res = send("GET", api_base, "/v0/webhooks", token, &params, None, op)?;
    Ok(into_page(json::<RawPage<Webhook>>(res, op)?))
}

pub fn add_webhook(
    api_base: &str,
    token: Option<&str>,
    url: &str,
    events: &[String],
    project: Option<&str>,
) -> DeployResult<Webhook> {
    validate_new_webhook(url, events, project)?;
    let mut body = serde_json::json!({ "url": url, "events": events });
    if let Some(p) = project {
        body["projectId"] = p.into();
    }
    let op = "Add webhook";
    let res = send("POST", api_base, "/v0/webhooks", token, &[], Some(body), op)?;
    json(res, op)
}

pub fn get_webhook(api_base: &str, token: Option<&str>, id: &str) -> DeployResult<Webhook> {
    validate_webhook_id(id)?;
    let op = "Inspect webhook";
    let path = format!("/v0/webhooks/{id}");
    json(send("GET", api_base, &path, token, &[], None, op)?, op)
}

pub fn remove_webhook(api_base: &str, token: Option<&str>, id: &str) -> DeployResult<bool> {
    validate_webhook_id(id)?;
    let path = format!("/v0/webhooks/{id}");
    send(
        "DELETE",
        api_base,
        &path,
        token,
        &[],
        None,
        "Remove webhook",
    )?;
    Ok(true)
}

pub fn list_webhook_deliveries(
    api_base: &str,
    token: Option<&str>,
    id: &str,
    query: &WebhookQuery,
) -> DeployResult<Page<WebhookDelivery>> {
    validate_webhook_id(id)?;
    let op = "List webhook deliveries";
    let path = format!("/v0/webhooks/{id}/deliveries");
    let params = page_params(query);
    let res = send("GET", api_base, &path, token, &params, None, op)?;
    Ok(into_page(json::<RawPage<WebhookDelivery>>(res, op)?))
}

pub fn test_webhook(
    api_base: &str,
    token: Option<&str>,
    id: &str,
) -> DeployResult<WebhookDelivery> {
    validate_webhook_id(id)?;
    let op = "Test webhook";
    let path = format!("/v0/webhooks/{id}/test");
    json(send("POST", api_base, &path, token, &[], None, op)?, op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// One-shot mock server: replies with `status`/`body`, returns the raw request text.
    fn serve(status: &'static str, body: String) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // Read until the headers and the full Content-Length body have
            // arrived: a single read can return the headers before the body.
            let mut req = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = s.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            s.write_all(resp.as_bytes()).unwrap();
            String::from_utf8_lossy(&req).into_owned()
        });
        (base, h)
    }

    const ID: &str = "whk_abcdefghij12";
    const HOOK: &str = r#"{"id":"whk_abcdefghij12","url":"https://x.test/h","events":["deployment.ready"],"projectId":null,"disabledAt":null,"createdAt":1}"#;
    const DELIVERY: &str = r#"{"id":"dlv_1","eventId":"evt_1","event":"webhook.test","attempt":1,"status":"ok","responseStatus":200,"nextAttemptAt":null,"createdAt":5}"#;

    #[test]
    fn ids_and_inputs_are_validated_client_side() {
        assert!(validate_webhook_id(ID).is_ok());
        assert!(validate_webhook_id("whk_short").is_err());
        assert!(validate_webhook_id("../x").is_err());
        let ev = vec!["deployment.ready".to_string()];
        assert!(validate_new_webhook("https://x.test/h", &ev, Some("prj_abcdefghij12")).is_ok());
        assert!(validate_new_webhook("http://x.test/h", &ev, None).is_err());
        assert!(validate_new_webhook("https://x.test/h", &[], None).is_err());
        let bad = vec!["deploy.done".to_string()];
        let err = validate_new_webhook("https://x.test/h", &bad, None).unwrap_err();
        assert!(err.to_string().contains("deployment.created"));
        assert!(validate_new_webhook("https://x.test/h", &ev, Some("my-app")).is_err());
    }

    #[test]
    fn list_maps_next_cursor_and_sends_paging() {
        let (base, h) = serve(
            "200 OK",
            format!(r#"{{"items":[{HOOK}],"nextCursor":"c2"}}"#),
        );
        let q = WebhookQuery {
            limit: Some(5),
            cursor: Some("c1".into()),
        };
        let page = list_webhooks(&base, Some("tok"), &q).unwrap();
        let req = h.join().unwrap();
        assert!(
            req.starts_with("GET /v0/webhooks?limit=5&cursor=c1 "),
            "{req}"
        );
        assert!(req.to_lowercase().contains("authorization: bearer tok"));
        assert_eq!(page.next.as_deref(), Some("c2"));
        assert_eq!(page.items[0].id, ID);
        assert!(page.items[0].secret.is_none());
    }

    #[test]
    fn add_returns_secret_once_and_never_prints_it_in_debug() {
        let body = HOOK.replace("\"createdAt\":1", "\"createdAt\":1,\"secret\":\"whsec_s3\"");
        let (base, h) = serve("201 Created", body);
        let ev = vec!["deployment.ready".to_string()];
        let hook = add_webhook(&base, None, "https://x.test/h", &ev, None).unwrap();
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /v0/webhooks "));
        assert!(req.contains(r#""events":["deployment.ready"]"#));
        assert_eq!(hook.secret.as_deref(), Some("whsec_s3"));
        assert!(!format!("{hook:?}").contains("whsec_s3"));
    }

    #[test]
    fn deliveries_test_remove_and_errors() {
        let (base, h) = serve(
            "200 OK",
            format!(r#"{{"items":[{DELIVERY}],"nextCursor":null}}"#),
        );
        let page = list_webhook_deliveries(&base, None, ID, &WebhookQuery::default()).unwrap();
        assert!(
            h.join()
                .unwrap()
                .starts_with(&format!("GET /v0/webhooks/{ID}/deliveries "))
        );
        assert_eq!(page.items[0].response_status, Some(200));
        assert!(page.next.is_none());

        let (base, h) = serve("200 OK", DELIVERY.to_string());
        let t = test_webhook(&base, None, ID).unwrap();
        assert!(
            h.join()
                .unwrap()
                .starts_with(&format!("POST /v0/webhooks/{ID}/test "))
        );
        assert_eq!(t.status, "ok");

        let (base, h) = serve("204 No Content", String::new());
        assert!(remove_webhook(&base, None, ID).unwrap());
        assert!(
            h.join()
                .unwrap()
                .starts_with(&format!("DELETE /v0/webhooks/{ID} "))
        );

        let (base, _h) = serve("404 Not Found", r#"{"error":"webhook not found"}"#.into());
        let err = get_webhook(&base, None, ID).unwrap_err();
        assert!(err.to_string().contains("webhook not found"));
    }
}
