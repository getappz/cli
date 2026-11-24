//! `appz projects pause|resume|keep|update|transfer` and `appz transfers`.

use std::path::Path;
use std::time::Duration;

use appz_deploy::{ProjectSettingsUpdate, ProjectState, Transfer};

use crate::{assume_yes, deploy, error, info, intro, is_tty, outro, success};

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
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| ms.to_string())
}

fn print_json<T: serde::Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

fn project_lines(p: &ProjectState) -> String {
    let mut lines = vec![format!("id:    {}\nslug:  {}", p.id, p.slug)];
    lines.push(format!(
        "state: {}",
        match (p.paused_at, p.pause_reason.as_deref()) {
            (Some(at), Some(why)) => format!("paused since {} ({why})", fmt_ms(at)),
            (Some(at), None) => format!("paused since {}", fmt_ms(at)),
            _ => "live".into(),
        }
    ));
    if let Some(at) = p.expires_at {
        lines.push(format!(
            "expires: {} (run `appz projects keep {}` to keep it)",
            fmt_ms(at),
            p.slug
        ));
    }
    lines.join("\n")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Pause,
    Resume,
    Keep,
}

pub fn lifecycle(dir: &Path, target: Option<String>, name: &str, action: Lifecycle, json: bool) {
    let (verb, done) = match action {
        Lifecycle::Pause => ("pause", "paused"),
        Lifecycle::Resume => ("resume", "resumed"),
        Lifecycle::Keep => ("keep", "kept permanently"),
    };
    if action == Lifecycle::Pause
        && !confirm(
            &format!("Pause {name}? Its production aliases stop serving until you resume it."),
            json,
        )
    {
        error("cancelled");
    }
    let res = deploy::with_provider(dir, target, |p, ctx| match action {
        Lifecycle::Pause => p.pause_project(ctx, name),
        Lifecycle::Resume => p.resume_project(ctx, name),
        Lifecycle::Keep => p.keep_project(ctx, name),
    });
    match res {
        Ok(p) if json => print_json(&p),
        Ok(p) => {
            intro(&format!("appz projects {verb}"));
            success(&format!("{done}: {}", p.slug));
            info(&project_lines(&p));
            outro("done");
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
            intro("appz projects update");
            if changes.is_empty() {
                info("settings already match; nothing changed");
            } else {
                let lines: Vec<String> = changes
                    .iter()
                    .map(|(f, a, b)| format!("{f}: {a} -> {b}"))
                    .collect();
                success(&format!("updated {}", after.slug));
                info(&lines.join("\n"));
            }
            outro("done");
        }
        Err(e) => error(&e),
    }
}

fn bytes(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.1} KiB", n as f64 / 1024.0),
        _ => format!("{:.1} MiB", n as f64 / 1_048_576.0),
    }
}

fn transfer_lines(t: &Transfer) -> String {
    let mut lines = vec![
        format!("transfer: {}", t.id),
        format!("project:  {}", t.project_id),
        format!(
            "from:     {}\nto:       {}",
            t.source_organization_id, t.target_organization_id
        ),
        format!("status:   {} (expires {})", t.status, fmt_ms(t.expires_at)),
    ];
    if let Some(p) = &t.preview {
        lines.push(format!(
            "moves:    {} deployments, {} domains, {} environment keys ({} of data)",
            p.deployments,
            p.domains,
            p.environment_keys,
            bytes(p.incremental_bytes)
        ));
        if !p.alias_hosts.is_empty() {
            lines.push(format!("aliases:  {}", p.alias_hosts.join(", ")));
        }
        if p.revoked_keys > 0 {
            lines.push(format!(
                "LOSES ACCESS: {} API key(s) tied to the source organization are revoked",
                p.revoked_keys
            ));
        }
    }
    lines.join("\n")
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
    intro("appz projects transfer");
    info(&transfer_lines(&t));
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
            Ok(_) => success("transfer request cancelled; nothing moved"),
            Err(e) => error(&e),
        }
        outro("done");
        return;
    }
    info(&format!(
        "Nothing has moved yet. The destination organization's owner runs `appz transfers accept {0}`; cancel with `appz transfers cancel {0}`.",
        t.id
    ));
    outro("done");
}

pub fn transfers_list(dir: &Path, target: Option<String>, json: bool) {
    match deploy::with_provider(dir, target, |p, ctx| p.list_transfers(ctx)) {
        Ok(list) if json => print_json(&list),
        Ok(list) => {
            intro("appz transfers");
            if list.is_empty() {
                info("no transfers");
            } else {
                let lines: Vec<String> = list
                    .iter()
                    .map(|t| {
                        format!(
                            "{:<18} {:<10} {} -> {}  project:{}",
                            t.id,
                            t.status,
                            t.source_organization_id,
                            t.target_organization_id,
                            t.project_id
                        )
                    })
                    .collect();
                info(&lines.join("\n"));
            }
            outro("done");
        }
        Err(e) => error(&e),
    }
}

pub fn transfers_inspect(dir: &Path, target: Option<String>, id: &str, json: bool) {
    match deploy::with_provider(dir, target, |p, ctx| p.get_transfer(ctx, id)) {
        Ok(t) if json => print_json(&t),
        Ok(t) => {
            intro("appz transfers inspect");
            info(&transfer_lines(&t));
            outro("done");
        }
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
        intro("appz transfers accept");
        info(&transfer_lines(&pending));
    }
    if !confirm(
        &format!(
            "Accept the transfer of {} into your organization?",
            pending.project_id
        ),
        json,
    ) {
        error("cancelled");
    }
    let res = deploy::with_provider(dir, target, |p, ctx| {
        p.accept_transfer(ctx, id, Duration::from_secs(timeout))
    });
    match res {
        Ok(t) if json => print_json(&t),
        Ok(t) => {
            success(&format!("transfer {} is {}", t.id, t.status));
            outro("done");
        }
        Err(e) => error(&e),
    }
}

pub fn transfers_cancel(dir: &Path, target: Option<String>, id: &str, json: bool) {
    if !confirm(&format!("Cancel transfer {id}?"), json) {
        error("cancelled");
    }
    match deploy::with_provider(dir, target, |p, ctx| p.cancel_transfer(ctx, id)) {
        Ok(t) if json => print_json(&t),
        Ok(t) => {
            intro("appz transfers cancel");
            success(&format!("transfer {} is {}", t.id, t.status));
            outro("done");
        }
        Err(e) => error(&e),
    }
}
