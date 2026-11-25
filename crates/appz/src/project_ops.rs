//! `appz projects pause|resume|keep|update|transfer` and `appz transfers`.

use std::path::Path;
use std::time::Duration;

use appz_deploy::{ProjectSettingsUpdate, ProjectState, Transfer};

use crate::{assume_yes, deploy, error, is_tty, success, ui, warning};

/// Ask before a consequential step. Non-interactive runs and `--json`
/// need an explicit `--yes`.
fn confirm(prompt: &str, json: bool) -> bool {
    if assume_yes() {
        return true;
    }
    if json || !is_tty() {
        error("confirmation required: pass --yes to continue without prompting");
    }
    cliclack::confirm(prompt)
        .initial_value(false)
        .interact()
        .unwrap_or(false)
}

fn fmt_ms(ms: i64) -> String {
    ui::fmt::datetime(ms)
}

fn print_json<T: serde::Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

fn print_project(p: &ProjectState) {
    let paused = ui::fmt::status("paused");
    let state = match (p.paused_at, p.pause_reason.as_deref()) {
        (Some(at), Some(why)) => format!("{paused} since {} ({why})", fmt_ms(at)),
        (Some(at), None) => format!("{paused} since {}", fmt_ms(at)),
        _ => ui::fmt::status("live"),
    };
    let mut rows = vec![
        ("ID", p.id.clone()),
        ("Slug", p.slug.clone()),
        ("State", state),
    ];
    if let Some(at) = p.expires_at {
        rows.push(("Expires", fmt_ms(at)));
    }
    ui::print_kv(&rows);
    if p.expires_at.is_some() {
        ui::hint(&format!("Run `appz projects keep {}` to keep it", p.slug));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Pause,
    Resume,
    Keep,
}

pub fn lifecycle(dir: &Path, target: Option<String>, name: &str, action: Lifecycle, json: bool) {
    let done = match action {
        Lifecycle::Pause => "paused",
        Lifecycle::Resume => "resumed",
        Lifecycle::Keep => "kept permanently",
    };
    if action == Lifecycle::Pause
        && !confirm(
            &format!("Pause {name}? Its production aliases stop serving until you resume it."),
            json,
        )
    {
        error("Cancelled");
    }
    let res = deploy::with_provider(dir, target, |p, ctx| match action {
        Lifecycle::Pause => p.pause_project(ctx, name),
        Lifecycle::Resume => p.resume_project(ctx, name),
        Lifecycle::Keep => p.keep_project(ctx, name),
    });
    match res {
        Ok(p) if json => print_json(&p),
        Ok(p) => {
            success(&format!("{} {}", capitalize(done), p.slug));
            print_project(&p);
        }
        Err(e) => error(&e),
    }
}

/// `(field, before, after)` for each setting the update would change.
fn setting_changes(
    before: &ProjectState,
    upd: &ProjectSettingsUpdate,
) -> Vec<(String, String, String)> {
    fn show<T: ToString>(v: &Option<T>) -> String {
        v.as_ref().map_or("-".into(), ToString::to_string)
    }
    let mut out = Vec::new();
    if let Some(n) = &upd.name
        && n.trim() != before.name
    {
        out.push(("name".into(), before.name.clone(), n.trim().to_string()));
    }
    if let Some(m) = &upd.toolbar_mode
        && before.toolbar_mode.as_ref() != Some(m)
    {
        out.push(("toolbarMode".into(), show(&before.toolbar_mode), m.clone()));
    }
    if let Some(m) = upd.markdown
        && before.markdown != Some(m)
    {
        out.push(("markdown".into(), show(&before.markdown), m.to_string()));
    }
    if let Some(d) = upd.deployment_protection_disabled
        && before.deployment_protection_disabled != Some(d)
    {
        out.push((
            "deploymentProtectionDisabled".into(),
            show(&before.deployment_protection_disabled),
            d.to_string(),
        ));
    }
    out
}

pub fn update(
    dir: &Path,
    target: Option<String>,
    name: &str,
    upd: ProjectSettingsUpdate,
    json: bool,
) {
    if upd.is_empty() {
        error(
            "nothing to update: pass at least one of --name, --toolbar-mode, --markdown, --deployment-protection-disabled",
        );
    }
    let res = deploy::with_provider(dir, target, |p, ctx| {
        let before = p.project_state(ctx, name)?;
        let changes = setting_changes(&before, &upd);
        if changes.is_empty() {
            return Ok((before.clone(), before, changes));
        }
        let after = p.update_project(ctx, name, &upd)?;
        Ok((before, after, changes))
    });
    match res {
        Ok((_, after, changes)) if json => {
            let changes: Vec<_> = changes
                .iter()
                .map(|(f, a, b)| serde_json::json!({ "field": f, "from": a, "to": b }))
                .collect();
            print_json(&serde_json::json!({ "project": after, "changes": changes }));
        }
        Ok((_, after, changes)) => {
            if changes.is_empty() {
                ui::hint("Settings already match; nothing changed");
            } else {
                success(&format!("Updated {}", after.slug));
                let mut t = ui::Table::new(&["Setting", "From", "To"]);
                for (f, a, b) in &changes {
                    t.row(vec![f.clone(), a.clone(), b.clone()]);
                }
                t.print();
            }
        }
        Err(e) => error(&e),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn print_transfer(t: &Transfer) {
    let mut rows = vec![
        ("Transfer", t.id.clone()),
        ("Project", t.project_id.clone()),
        ("From", t.source_organization_id.clone()),
        ("To", t.target_organization_id.clone()),
        ("Status", ui::fmt::status(&t.status)),
        ("Expires", fmt_ms(t.expires_at)),
    ];
    if let Some(p) = &t.preview {
        rows.push((
            "Moves",
            format!(
                "{}, {}, {} ({} of data)",
                ui::fmt::plural(p.deployments as usize, "deployment"),
                ui::fmt::plural(p.domains as usize, "domain"),
                ui::fmt::plural(p.environment_keys as usize, "environment key"),
                ui::fmt::bytes(p.incremental_bytes)
            ),
        ));
        if !p.alias_hosts.is_empty() {
            rows.push(("Aliases", p.alias_hosts.join(", ")));
        }
    }
    ui::print_kv(&rows);
    if let Some(p) = t.preview.as_ref().filter(|p| p.revoked_keys > 0) {
        warning(&format!(
            "Loses access: {} tied to the source organization are revoked",
            ui::fmt::plural(p.revoked_keys as usize, "API key")
        ));
    }
}

/// Request a transfer. The server records it as pending and returns the
/// preview; nothing moves until the destination owner accepts it.
pub fn transfer(dir: &Path, target: Option<String>, name: &str, to: &str, json: bool) {
    let t = match deploy::with_provider(dir, target.clone(), |p, ctx| {
        p.request_transfer(ctx, name, to)
    }) {
        Ok(t) => t,
        Err(e) => error(&e),
    };
    if json {
        print_json(&t);
        return;
    }
    print_transfer(&t);
    // The request already exists server-side. Only an interactive user gets
    // the chance to withdraw it; scripts keep it and can cancel explicitly.
    let keep = assume_yes()
        || !is_tty()
        || cliclack::confirm("Keep this transfer request open for the destination owner?")
            .initial_value(true)
            .interact()
            .unwrap_or(true);
    if !keep {
        match deploy::with_provider(dir, target, |p, ctx| p.cancel_transfer(ctx, &t.id)) {
            Ok(_) => success("Transfer request cancelled; nothing moved"),
            Err(e) => error(&e),
        }
        return;
    }
    ui::hint(&format!(
        "Nothing has moved yet. The destination organization's owner runs `appz transfers accept {0}`; cancel with `appz transfers cancel {0}`.",
        t.id
    ));
}

pub fn transfers_list(dir: &Path, target: Option<String>, json: bool) {
    match deploy::with_provider(dir, target, |p, ctx| p.list_transfers(ctx)) {
        Ok(list) if json => print_json(&list),
        Ok(list) => {
            if list.is_empty() {
                ui::empty("transfers", None);
                return;
            }
            let mut t = ui::Table::new(&["ID", "Project", "From", "To", "Status", "Expires"]);
            for x in &list {
                t.row(vec![
                    x.id.clone(),
                    x.project_id.clone(),
                    x.source_organization_id.clone(),
                    x.target_organization_id.clone(),
                    ui::fmt::status(&x.status),
                    fmt_ms(x.expires_at),
                ]);
            }
            t.print();
        }
        Err(e) => error(&e),
    }
}

pub fn transfers_inspect(dir: &Path, target: Option<String>, id: &str, json: bool) {
    match deploy::with_provider(dir, target, |p, ctx| p.get_transfer(ctx, id)) {
        Ok(t) if json => print_json(&t),
        Ok(t) => print_transfer(&t),
        Err(e) => error(&e),
    }
}

pub fn transfers_accept(dir: &Path, target: Option<String>, id: &str, timeout: u64, json: bool) {
    let pending = match deploy::with_provider(dir, target.clone(), |p, ctx| p.get_transfer(ctx, id))
    {
        Ok(t) => t,
        Err(e) => error(&e),
    };
    if !json {
        print_transfer(&pending);
    }
    if !confirm(
        &format!(
            "Accept the transfer of {} into your organization?",
            pending.project_id
        ),
        json,
    ) {
        error("Cancelled");
    }
    let res = deploy::with_provider(dir, target, |p, ctx| {
        p.accept_transfer(ctx, id, Duration::from_secs(timeout))
    });
    match res {
        Ok(t) if json => print_json(&t),
        Ok(t) => success(&format!("Transfer {} is {}", t.id, t.status)),
        Err(e) => error(&e),
    }
}

pub fn transfers_cancel(dir: &Path, target: Option<String>, id: &str, json: bool) {
    if !confirm(&format!("Cancel transfer {id}?"), json) {
        error("Cancelled");
    }
    match deploy::with_provider(dir, target, |p, ctx| p.cancel_transfer(ctx, id)) {
        Ok(t) if json => print_json(&t),
        Ok(t) => success(&format!("Transfer {} is {}", t.id, t.status)),
        Err(e) => error(&e),
    }
}
