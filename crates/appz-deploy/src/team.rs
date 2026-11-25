//! Team scope for appz.dev requests. Resolved once per run, then sent as
//! `?teamId=<id>` on every `/v0/...` request (see [`scoped`]).
//!
//! Precedence: `--scope` > `APPZ_TEAM_ID` > linked project's
//! `.appz/project.json` `orgId` > `organization_id` in `~/.appz/auth.json`
//! (written by `appz switch`). None: no param, so the server uses the
//! session's active organization.
//!
//! The project/stored team belongs to the `appz login` session, so it is only
//! sent when that session token authenticates the request. A token from
//! `APPZ_TOKEN` (env or vault) is bound to its own team (an org API key), and
//! sending the login's team with it is rejected; only an explicit `--scope` /
//! `APPZ_TEAM_ID` is sent with such a token.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static SCOPE: OnceLock<Option<String>> = OnceLock::new();
/// The resolved team and whether it was asked for explicitly (`--scope`/env).
static TEAM: OnceLock<Option<(String, bool)>> = OnceLock::new();
/// Set by `resolve_auth_token`: the token in use is the `appz login` session.
static LOGIN_TOKEN: AtomicBool = AtomicBool::new(false);

pub(crate) fn note_login_token(login: bool) {
    LOGIN_TOKEN.store(login, Ordering::Relaxed);
}

/// Record `--scope` (call once at startup, before any request).
pub fn set_scope(scope: Option<String>) {
    let _ = SCOPE.set(scope.filter(|s| !s.is_empty()));
}

/// Pick the team source. The flag is `(value, needs_lookup)`: `--scope` and
/// the env var may hold a slug, stored values are already ids.
fn pick(
    scope: Option<String>,
    env: Option<String>,
    project: Option<String>,
    stored: Option<String>,
) -> Option<(String, bool)> {
    let some = |v: Option<String>| v.filter(|s| !s.is_empty());
    some(scope)
        .map(|s| (s, true))
        .or_else(|| some(env).map(|s| (s, true)))
        .or_else(|| some(project).map(|s| (s, false)))
        .or_else(|| some(stored).map(|s| (s, false)))
}

/// Find an organization id in a better-auth `organization/list` reply by id or slug.
fn match_org(list: &serde_json::Value, q: &str) -> Option<String> {
    let orgs = list.as_array()?;
    let field = |k: &str| orgs.iter().find(|o| o[k] == q);
    field("id")
        .or_else(|| field("slug"))
        .and_then(|o| o["id"].as_str().map(String::from))
}

fn project_org() -> Option<String> {
    let raw = std::fs::read_to_string(".appz/project.json").ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v["orgId"].as_str().map(String::from)
}

/// Slug -> id through the org list; an unresolvable value is used as an id
/// and the server's membership check reports a bad one.
fn lookup(q: &str) -> String {
    let base = std::env::var("APPZ_API_URL").unwrap_or_else(|_| "https://api.appz.dev".to_string());
    crate::providers::appzdev::auth_token(None)
        .0
        .and_then(|t| {
            ureq::get(&format!("{base}/api/auth/organization/list"))
                .set("Authorization", &format!("Bearer {t}"))
                .call()
                .ok()?
                .into_json()
                .ok()
        })
        .and_then(|list| match_org(&list, q))
        .unwrap_or_else(|| q.to_string())
}

fn resolved() -> Option<(String, bool)> {
    // Unit tests mock the API and assert exact paths: never read the developer's login.
    if cfg!(test) {
        return None;
    }
    TEAM.get_or_init(|| {
        let (v, explicit) = pick(
            SCOPE.get().cloned().flatten(),
            std::env::var("APPZ_TEAM_ID").ok(),
            project_org(),
            crate::load_stored_auth().and_then(|a| a.organization_id),
        )?;
        Some((if explicit { lookup(&v) } else { v }, explicit))
    })
    .clone()
}

/// The team id for this run, if any.
pub fn current_team() -> Option<String> {
    resolved().map(|(id, _)| id)
}

/// Whether a resolved team goes on a request: always when explicit, else only
/// with the login session token it belongs to.
fn sendable(team: Option<(String, bool)>, login_token: bool) -> Option<String> {
    team.filter(|(_, explicit)| *explicit || login_token)
        .map(|(id, _)| id)
}

/// Add `teamId` to a `/v0/` request.
pub fn scoped_with(req: ureq::Request, team: Option<&str>) -> ureq::Request {
    match team {
        Some(t) if req.url().contains("/v0/") => req.query("teamId", t),
        _ => req,
    }
}

pub fn scoped(req: ureq::Request) -> ureq::Request {
    if !req.url().contains("/v0/") {
        return req;
    }
    scoped_with(
        req,
        sendable(resolved(), LOGIN_TOKEN.load(Ordering::Relaxed)).as_deref(),
    )
}

pub fn request(method: &str, url: &str) -> ureq::Request {
    scoped(ureq::request(method, url))
}

pub fn get(url: &str) -> ureq::Request {
    request("GET", url)
}

pub fn post(url: &str) -> ureq::Request {
    request("POST", url)
}

pub fn delete(url: &str) -> ureq::Request {
    request("DELETE", url)
}

pub fn patch(url: &str) -> ureq::Request {
    request("PATCH", url)
}

#[allow(clippy::result_large_err)] // fixed by ureq's middleware signature
/// For `ureq::AgentBuilder::middleware`, so pooled agents are scoped too.
pub fn middleware(
    req: ureq::Request,
    next: ureq::MiddlewareNext,
) -> Result<ureq::Response, ureq::Error> {
    next.handle(scoped(req))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn implicit_team_only_rides_the_login_token() {
        let t = |explicit| Some(("t1".to_string(), explicit));
        assert_eq!(sendable(t(false), true).as_deref(), Some("t1"));
        assert_eq!(
            sendable(t(false), false),
            None,
            "vault/env key keeps its own team"
        );
        assert_eq!(
            sendable(t(true), false).as_deref(),
            Some("t1"),
            "--scope is always sent"
        );
        assert_eq!(sendable(None, true), None);
    }

    #[test]
    fn precedence_scope_env_project_stored() {
        assert_eq!(
            pick(s("a"), s("b"), s("c"), s("d")),
            Some(("a".into(), true))
        );
        assert_eq!(pick(None, s("b"), s("c"), s("d")), Some(("b".into(), true)));
        assert_eq!(pick(None, None, s("c"), s("d")), Some(("c".into(), false)));
        assert_eq!(pick(None, s(""), None, s("d")), Some(("d".into(), false)));
        assert_eq!(pick(None, None, None, None), None);
    }

    #[test]
    fn matches_org_by_id_then_slug() {
        let list = serde_json::json!([
            { "id": "o1", "slug": "acme" },
            { "id": "o2", "slug": "o1" }
        ]);
        assert_eq!(match_org(&list, "o1").as_deref(), Some("o1"));
        assert_eq!(match_org(&list, "acme").as_deref(), Some("o1"));
        assert_eq!(match_org(&list, "nope"), None);
    }

    #[test]
    fn appends_team_id_to_v0_only() {
        let url = |u: &str, t| scoped_with(ureq::get(u), t).url().to_string();
        assert_eq!(
            url("https://api.appz.dev/v0/projects", Some("t1")),
            "https://api.appz.dev/v0/projects?teamId=t1"
        );
        assert_eq!(
            url("https://api.appz.dev/v0/projects?limit=5", Some("t1")),
            "https://api.appz.dev/v0/projects?limit=5&teamId=t1"
        );
        assert_eq!(
            url(
                "https://api.appz.dev/api/auth/organization/list",
                Some("t1")
            ),
            "https://api.appz.dev/api/auth/organization/list"
        );
        assert_eq!(
            url("https://api.appz.dev/v0/projects", None),
            "https://api.appz.dev/v0/projects"
        );
    }
}
