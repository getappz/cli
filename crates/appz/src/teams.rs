//! `appz teams` / `appz switch`: manage appz.dev organizations (teams).
//!
//! Membership calls hit better-auth's organization plugin
//! (`/api/auth/organization/*`, with an explicit `organizationId`); deletion
//! uses the `/v0/team/*` API. The team is resolved per run by
//! `appz_deploy::team` (`--scope`, `APPZ_TEAM_ID`, project, `auth.json`);
//! `switch` only saves it to `~/.appz/auth.json`, never to the server session.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{assume_yes, error, is_tty, success, ui};

const ROLES: [&str; 3] = ["member", "admin", "owner"];

#[derive(Args)]
pub struct TeamsArgs {
    #[command(subcommand)]
    command: TeamsCmd,
}

#[derive(Args, Debug)]
pub struct SwitchArgs {
    /// Team slug or id (omit to pick from a list)
    team: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum TeamsCmd {
    /// List the teams you belong to (the active one is marked)
    #[command(alias = "list")]
    Ls,
    /// Create a team and make it active
    #[command(visible_alias = "add")]
    Create {
        name: String,
        /// URL slug (default: derived from the name)
        #[arg(long)]
        slug: Option<String>,
    },
    /// Set the team later commands target (saved locally)
    Switch(SwitchArgs),
    /// Invite people to the active team by email
    Invite {
        #[arg(required = true)]
        emails: Vec<String>,
        #[arg(long, default_value = "member", value_parser = ROLES)]
        role: String,
    },
    /// List members of the active team, or change them
    Members {
        #[command(subcommand)]
        command: Option<MembersCmd>,
    },
    /// List pending invitations, or cancel one
    Invitations {
        #[command(subcommand)]
        command: Option<InvitationsCmd>,
    },
    /// Leave a team (default: the active one)
    Leave { team: Option<String> },
    /// Delete a team (default: the current one; it must hold no projects)
    #[command(alias = "delete")]
    Rm { team: Option<String> },
}

#[derive(Subcommand, Debug)]
pub enum MembersCmd {
    /// Remove a member by email or member id
    #[command(alias = "remove")]
    Rm { member: String },
    /// Change a member's role
    Role {
        /// Email or member id
        member: String,
        #[arg(value_parser = ROLES)]
        role: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum InvitationsCmd {
    /// Cancel a pending invitation
    Cancel { id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Org {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Member {
    id: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    user: MemberUser,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct MemberUser {
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Invitation {
    id: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    expires_at: Option<String>,
}

struct Api {
    base: String,
    token: String,
    /// Resolved team id (`None`: the server falls back to the session's org).
    team: Option<String>,
}

impl Api {
    fn new() -> Api {
        let token = appz_deploy::resolve_auth_token(None)
            .unwrap_or_else(|| error("not authenticated. Run `appz login` or set APPZ_TOKEN"));
        let base =
            std::env::var("APPZ_API_URL").unwrap_or_else(|_| "https://api.appz.dev".to_string());
        Api {
            base,
            token,
            team: appz_deploy::team::current_team(),
        }
    }

    fn with_team(&self, id: &str) -> Api {
        Api {
            base: self.base.clone(),
            token: self.token.clone(),
            team: Some(id.to_string()),
        }
    }

    /// `body: None` sends a GET with `query`; `Some` sends a JSON POST.
    /// `/v0/` paths carry this Api's team as `teamId`.
    fn try_call(
        &self,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let url = format!("{}{path}", self.base);
        let method = if body.is_some() { "POST" } else { "GET" };
        let mut req =
            appz_deploy::team::scoped_with(ureq::request(method, &url), self.team.as_deref())
                .set("Authorization", &format!("Bearer {}", self.token));
        for (k, v) in query {
            req = req.query(k, v);
        }
        let res = match body {
            None => req.call(),
            Some(b) => req.send_json(b),
        };
        match res {
            Ok(r) => Ok(r
                .into_string()
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null)),
            Err(ureq::Error::Status(code, resp)) if code != 401 => {
                let j: Value = resp.into_json().unwrap_or(Value::Null);
                Err(["message", "error"]
                    .iter()
                    .find_map(|k| j.get(k).and_then(|m| m.as_str()))
                    .map_or_else(|| format!("HTTP {code}"), String::from))
            }
            Err(e) => Err(appz_deploy::parse_api_error(e, "AppzDev", "Team request").to_string()),
        }
    }

    fn call(&self, path: &str, body: Option<Value>) -> Value {
        self.try_call(path, &[], body).unwrap_or_else(|e| error(&e))
    }

    /// better-auth org route not tied to a team (`list`, `create`).
    fn org(&self, path: &str, body: Option<Value>) -> Value {
        self.call(&format!("/api/auth/organization/{path}"), body)
    }

    /// better-auth org route scoped to the resolved team via `organizationId`
    /// (body for POST, query for GET); a body that sets it already wins.
    fn org_in(&self, path: &str, body: Option<Value>) -> Value {
        self.org_in_try(path, body).unwrap_or_else(|e| error(&e))
    }

    fn org_in_try(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        let path = format!("/api/auth/organization/{path}");
        match (body, self.team.as_deref()) {
            (None, Some(t)) => self.try_call(&path, &[("organizationId", t)], None),
            (Some(mut b), Some(t)) => {
                b["organizationId"] = b.get("organizationId").cloned().unwrap_or(json!(t));
                self.try_call(&path, &[], Some(b))
            }
            (b, None) => self.try_call(&path, &[], b),
        }
    }

    fn orgs(&self) -> Vec<Org> {
        serde_json::from_value(self.org("list", None)).unwrap_or_default()
    }

    /// The team requests target: the resolved scope, else the session's active org.
    fn current_id(&self) -> Option<String> {
        self.team.clone().or_else(|| self.session_org())
    }

    fn session_org(&self) -> Option<String> {
        let v = self.call("/api/auth/get-session", None);
        v.pointer("/session/activeOrganizationId")
            .and_then(|s| s.as_str())
            .map(String::from)
    }
}

/// Find a team by exact id, then exact slug.
fn resolve_org<'a>(orgs: &'a [Org], q: &str) -> Result<&'a Org, String> {
    orgs.iter()
        .find(|o| o.id == q)
        .or_else(|| orgs.iter().find(|o| o.slug == q))
        .ok_or_else(|| format!("no team '{q}' (see `appz teams ls`)"))
}

/// Find a member by id, then case-insensitive email.
fn resolve_member<'a>(members: &'a [Member], q: &str) -> Result<&'a Member, String> {
    members
        .iter()
        .find(|m| m.id == q)
        .or_else(|| {
            members
                .iter()
                .find(|m| m.user.email.eq_ignore_ascii_case(q))
        })
        .ok_or_else(|| format!("no member '{q}' (see `appz teams members`)"))
}

/// list-members answers `{members, total}` (older servers: a bare array).
fn parse_members(v: Value) -> Vec<Member> {
    let items = match v {
        Value::Array(_) => v,
        mut o => o.get_mut("members").map_or(Value::Null, Value::take),
    };
    serde_json::from_value(items).unwrap_or_default()
}

fn is_active(org: &Org, active: Option<&str>) -> bool {
    active == Some(org.id.as_str())
}

fn print_json<T: Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

fn date(s: &Option<String>) -> String {
    s.as_deref().unwrap_or("").chars().take(10).collect()
}

/// Save the current team in `~/.appz/auth.json`. With an `APPZ_TOKEN` login and no
/// file yet, the file is created with an empty token, which token resolution ignores.
fn store_team(org_id: Option<String>) {
    let mut a = appz_deploy::load_stored_auth().unwrap_or(appz_deploy::StoredAuth {
        token: String::new(),
        user_id: None,
        email: None,
        organization_id: None,
    });
    a.organization_id = org_id;
    if let Err(e) = appz_deploy::save_stored_auth(&a) {
        error(&format!("could not save the current team: {e}"));
    }
}

/// Whether `id` is the team saved by `switch`.
fn is_stored_team(id: &str) -> bool {
    appz_deploy::load_stored_auth()
        .and_then(|a| a.organization_id)
        .as_deref()
        == Some(id)
}

fn pick_org(orgs: &[Org], active: Option<&str>) -> String {
    let mut sel = cliclack::select("Select a team");
    for o in orgs {
        let hint = if is_active(o, active) { "active" } else { "" };
        sel = sel.item(o.id.clone(), format!("{} ({})", o.name, o.slug), hint);
    }
    if let Some(a) = active {
        sel = sel.initial_value(a.to_string());
    }
    sel.interact().unwrap_or_else(|_| error("Cancelled"))
}

pub fn switch(args: SwitchArgs, json: bool) {
    let api = Api::new();
    let orgs = api.orgs();
    let org = match args.team {
        Some(q) => resolve_org(&orgs, &q).unwrap_or_else(|e| error(&e)).clone(),
        None if is_tty() && !json && !orgs.is_empty() => {
            let id = pick_org(&orgs, api.current_id().as_deref());
            orgs.iter().find(|o| o.id == id).unwrap().clone()
        }
        None => error(
            "pass a team slug or id (see `appz teams ls`); the picker needs an interactive terminal",
        ),
    };
    store_team(Some(org.id.clone()));
    if json {
        return print_json(&org);
    }
    success(&format!("Now using team {} ({})", org.name, org.slug));
}

/// The deletion/leave target: an explicit team, else the active one.
fn target<'a>(orgs: &'a [Org], arg: Option<&str>, active: Option<&str>) -> &'a Org {
    match arg.or(active) {
        Some(q) => resolve_org(orgs, q).unwrap_or_else(|e| error(&e)),
        None => error("no active team; pass a slug or run `appz teams switch`"),
    }
}

fn confirm(prompt: String) -> bool {
    if assume_yes() {
        return true;
    }
    if !is_tty() {
        error("confirmation required — pass --yes to continue without prompting");
    }
    cliclack::confirm(prompt)
        .initial_value(false)
        .interact()
        .unwrap_or(false)
}

fn delete_team(api: &Api, arg: Option<String>, json: bool) {
    let orgs = api.orgs();
    let org = target(&orgs, arg.as_deref(), api.current_id().as_deref()).clone();
    let api = &api.with_team(&org.id);
    let c = api.call("/v0/team/deletion-check", None);
    if c["canDelete"] != json!(true) {
        let n = |k: &str| c[k].as_u64().unwrap_or(0) as usize;
        let blockers: Vec<String> = [
            ("projects", "project"),
            ("deletedProjects", "deleted project"),
            ("blobs", "blob"),
            ("webhooks", "webhook"),
            ("transfers", "transfer"),
        ]
        .iter()
        .filter(|(k, _)| n(k) > 0)
        .map(|(k, noun)| ui::fmt::plural(n(k), noun))
        .collect();
        error(&format!(
            "cannot delete {}: it still has {}",
            org.slug,
            blockers.join(", ")
        ));
    }
    if !confirm(format!(
        "Permanently delete team {} ({})?",
        org.name, org.slug
    )) {
        error("Cancelled");
    }
    api.call("/v0/team/delete", Some(json!({ "slug": org.slug })));
    if is_stored_team(&org.id) {
        store_team(None);
    }
    if json {
        return print_json(&json!({ "deleted": org.slug }));
    }
    success(&format!("Deleted team {}", org.slug));
}

fn invite(api: &Api, emails: &[String], role: &str, json: bool) {
    let (mut out, mut failed) = (Vec::new(), false);
    for email in emails {
        let body = json!({ "email": email, "role": role });
        match api.org_in_try("invite-member", Some(body)) {
            Ok(_) => {
                if !json {
                    success(&format!("Invited {email} as {role}"));
                }
                out.push(json!({ "email": email, "role": role, "invited": true }));
            }
            Err(msg) => {
                failed = true;
                if !json {
                    ui::warning(&format!("{email}: {msg}"));
                }
                out.push(json!({ "email": email, "invited": false, "error": msg }));
            }
        }
    }
    if json {
        print_json(&out);
    }
    if failed {
        std::process::exit(1);
    }
}

pub fn run(args: TeamsArgs, json: bool) {
    let api = Api::new();
    match args.command {
        TeamsCmd::Switch(a) => switch(a, json),
        TeamsCmd::Ls => {
            let fetch = ui::fetching("teams");
            let orgs = api.orgs();
            let active = api.current_id();
            if json {
                let v: Vec<Value> = orgs
                    .iter()
                    .map(|o| {
                        json!({ "id": o.id, "name": o.name, "slug": o.slug,
                            "active": is_active(o, active.as_deref()) })
                    })
                    .collect();
                return print_json(&v);
            }
            fetch.done(&format!("{} found", ui::fmt::plural(orgs.len(), "team")));
            if orgs.is_empty() {
                return ui::empty("teams", Some("Run `appz teams create <name>`"));
            }
            let mut t = ui::Table::new(&["", "Slug", "Name", "ID"]);
            for o in &orgs {
                let mark = if is_active(o, active.as_deref()) {
                    "*"
                } else {
                    ""
                };
                t.row(vec![
                    mark.into(),
                    o.slug.clone(),
                    o.name.clone(),
                    o.id.clone(),
                ]);
            }
            t.print();
        }
        TeamsCmd::Create { name, slug } => {
            let slug = slug.unwrap_or_else(|| appz_deploy::slugify_project_id(&name));
            let v = api.org("create", Some(json!({ "name": name, "slug": slug })));
            let org: Org = serde_json::from_value(v)
                .unwrap_or_else(|_| error("unexpected response creating team"));
            store_team(Some(org.id.clone()));
            if json {
                return print_json(&org);
            }
            success(&format!(
                "Created team {} ({}) and switched to it",
                org.name, org.slug
            ));
        }
        TeamsCmd::Invite { emails, role } => invite(&api, &emails, &role, json),
        TeamsCmd::Members { command: None } => {
            let fetch = ui::fetching("members");
            let members = parse_members(api.org_in("list-members", None));
            if json {
                return print_json(&members);
            }
            fetch.done(&format!(
                "{} found",
                ui::fmt::plural(members.len(), "member")
            ));
            let mut t = ui::Table::new(&["Email", "Name", "Role", "Joined", "ID"]);
            for m in &members {
                t.row(vec![
                    m.user.email.clone(),
                    m.user.name.clone(),
                    m.role.clone(),
                    date(&m.created_at),
                    m.id.clone(),
                ]);
            }
            t.print();
        }
        TeamsCmd::Members {
            command: Some(MembersCmd::Rm { member }),
        } => {
            if !confirm(format!("Remove {member} from the active team?")) {
                error("Cancelled");
            }
            api.org_in("remove-member", Some(json!({ "memberIdOrEmail": member })));
            if json {
                return print_json(&json!({ "removed": member }));
            }
            success(&format!("Removed {member}"));
        }
        TeamsCmd::Members {
            command: Some(MembersCmd::Role { member, role }),
        } => {
            let members = parse_members(api.org_in("list-members", None));
            let m = resolve_member(&members, &member).unwrap_or_else(|e| error(&e));
            api.org_in(
                "update-member-role",
                Some(json!({ "memberId": m.id, "role": role })),
            );
            if json {
                return print_json(&json!({ "memberId": m.id, "role": role }));
            }
            success(&format!("{} is now {role}", m.user.email));
        }
        TeamsCmd::Invitations { command: None } => {
            let fetch = ui::fetching("invitations");
            let all: Vec<Invitation> =
                serde_json::from_value(api.org_in("list-invitations", None)).unwrap_or_default();
            let pending: Vec<_> = all.into_iter().filter(|i| i.status == "pending").collect();
            if json {
                return print_json(&pending);
            }
            fetch.done(&format!(
                "{} found",
                ui::fmt::plural(pending.len(), "pending invitation")
            ));
            if pending.is_empty() {
                return ui::empty(
                    "pending invitations",
                    Some("Run `appz teams invite <email>`"),
                );
            }
            let mut t = ui::Table::new(&["Email", "Role", "Expires", "ID"]);
            for i in &pending {
                t.row(vec![
                    i.email.clone(),
                    i.role.clone().unwrap_or_default(),
                    date(&i.expires_at),
                    i.id.clone(),
                ]);
            }
            t.print();
        }
        TeamsCmd::Invitations {
            command: Some(InvitationsCmd::Cancel { id }),
        } => {
            api.org_in("cancel-invitation", Some(json!({ "invitationId": id })));
            if json {
                return print_json(&json!({ "canceled": id }));
            }
            success(&format!("Canceled invitation {id}"));
        }
        TeamsCmd::Leave { team } => {
            let orgs = api.orgs();
            let active = api.current_id();
            let org = target(&orgs, team.as_deref(), active.as_deref()).clone();
            if !confirm(format!("Leave team {} ({})?", org.name, org.slug)) {
                error("Cancelled");
            }
            api.org_in("leave", Some(json!({ "organizationId": org.id })));
            if is_stored_team(&org.id) {
                store_team(None);
            }
            if json {
                return print_json(&json!({ "left": org.slug }));
            }
            success(&format!("Left team {}", org.slug));
        }
        TeamsCmd::Rm { team } => delete_team(&api, team, json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn org(id: &str, slug: &str) -> Org {
        Org {
            id: id.into(),
            name: slug.into(),
            slug: slug.into(),
        }
    }

    #[test]
    fn resolves_id_before_slug() {
        let orgs = [org("a", "b"), org("b", "a")];
        assert_eq!(resolve_org(&orgs, "a").unwrap().id, "a");
        assert_eq!(resolve_org(&orgs, "b").unwrap().id, "b");
        assert!(resolve_org(&orgs, "zz").is_err());
    }

    #[test]
    fn marks_active_org() {
        let o = org("o1", "x");
        assert!(is_active(&o, Some("o1")));
        assert!(!is_active(&o, None));
    }

    #[test]
    fn parses_both_member_shapes() {
        let m = json!({ "id": "m1", "role": "owner", "createdAt": "2026-01-02T00:00:00Z",
            "user": { "name": "A", "email": "A@x.io" } });
        assert_eq!(
            parse_members(json!({ "members": [m.clone()], "total": 1 })).len(),
            1
        );
        let list = parse_members(json!([m]));
        assert_eq!(resolve_member(&list, "a@x.io").unwrap().id, "m1");
        assert_eq!(resolve_member(&list, "m1").unwrap().role, "owner");
        assert!(resolve_member(&list, "nope").is_err());
        assert!(parse_members(Value::Null).is_empty());
    }
}
