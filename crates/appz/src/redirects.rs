//! `appz redirects`: staged redirect versions (list, add, rm, import, promote, restore).

use std::path::Path;

use appz_deploy::redirects::{
    RedirectInput, RedirectList, RedirectRule, RedirectVersions, parse_redirect_import,
    validate_redirect,
};
use serde_json::json;

use crate::{
    RedirectsArgs, RedirectsCmd, assume_yes, deploy, error, is_interactive_tty, success, ui,
    warning,
};

pub(crate) fn run(args: RedirectsArgs, json: bool) {
    let dir = crate::resolve_root(&args.dir);
    let target = args.target;
    let ctx = Ctx {
        dir: &dir,
        target,
        json,
    };
    match args.command {
        RedirectsCmd::Ls { version, search } => ctx.ls(version, search.as_deref()),
        RedirectsCmd::Add {
            source,
            destination,
            status,
        } => ctx.add(RedirectInput {
            source,
            destination,
            status,
        }),
        RedirectsCmd::Rm { id } => ctx.rm(&id),
        RedirectsCmd::Import { file } => ctx.import(&file),
        RedirectsCmd::Versions => ctx.versions(),
        RedirectsCmd::Promote { version } => ctx.update_version(version, "promote"),
        RedirectsCmd::Restore { version } => ctx.update_version(version, "restore"),
    }
}

struct Ctx<'a> {
    dir: &'a Path,
    target: Option<String>,
    json: bool,
}

impl Ctx<'_> {
    fn op<T>(
        &self,
        f: impl FnOnce(
            &dyn appz_deploy::DeployProvider,
            &appz_deploy::DeployContext,
        ) -> appz_deploy::DeployResult<T>,
    ) -> Result<T, String> {
        deploy::with_provider(self.dir, self.target.clone(), f)
    }

    fn versions_or_exit(&self) -> RedirectVersions {
        self.op(|p, c| p.list_redirect_versions(c))
            .unwrap_or_else(|e| error(&e))
    }

    /// Stop unless the user confirmed (or passed --yes). Never prompts in
    /// JSON mode or without a terminal.
    fn confirm(&self, prompt: &str) {
        if assume_yes() {
            return;
        }
        if self.json || !is_interactive_tty() {
            error("confirmation required: pass --yes to proceed without a prompt");
        }
        let ok = cliclack::confirm(prompt)
            .initial_value(false)
            .interact()
            .unwrap_or(false);
        if !ok {
            warning("Cancelled, nothing changed");
            std::process::exit(0);
        }
    }

    fn ls(&self, version: Option<u64>, search: Option<&str>) {
        let fetch = ui::fetching("redirects");
        let mut list = self
            .op(|p, c| p.list_redirects(c, version))
            .unwrap_or_else(|e| error(&e));
        let versions = self.op(|p, c| p.list_redirect_versions(c)).ok();
        if let Some(q) = search {
            list.rules
                .retain(|r| r.source.contains(q) || r.destination.contains(q));
        }
        if self.json {
            let mut out = json!({ "version": list.version, "rules": list.rules });
            if let Some(v) = &versions {
                out["liveVersion"] = json!(v.live_version);
                out["stagedVersion"] = json!(v.staged_version);
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            return;
        }
        let label = match (list.version, &versions) {
            (None, _) => "no redirect versions yet".to_string(),
            (Some(n), Some(v)) => format!("version {n}, {}", version_tag(v, n)),
            (Some(n), None) => format!("version {n}"),
        };
        fetch.done(&format!(
            "{} found ({label})",
            ui::fmt::plural(list.rules.len(), "redirect")
        ));
        if list.rules.is_empty() {
            ui::hint("Run `appz redirects add <source> <destination>`");
        } else {
            rules_table(&list.rules).print();
        }
        if let Some(v) = &versions {
            print_state(v);
        }
    }

    fn add(&self, rule: RedirectInput) {
        if let Some(problem) = validate_redirect(&rule.source, &rule.destination, rule.status) {
            error(&problem);
        }
        let list = self
            .op(|p, c| p.add_redirect(c, &rule))
            .unwrap_or_else(|e| error(&e));
        self.report_staged(
            &format!(
                "Added {} -> {} ({})",
                rule.source, rule.destination, rule.status
            ),
            &list,
            json!({ "added": rule }),
        );
    }

    fn rm(&self, id: &str) {
        let current = self
            .op(|p, c| p.list_redirects(c, None))
            .unwrap_or_else(|e| error(&e));
        let Some(rule) = current
            .rules
            .iter()
            .find(|r| r.id == id)
            .or_else(|| current.rules.iter().find(|r| r.source == id))
            .cloned()
        else {
            error(&format!(
                "no redirect with ID or source '{id}'; run `appz redirects ls`"
            ))
        };
        self.confirm(&format!(
            "Remove redirect {} -> {} ({})? This stages a new version without it.",
            rule.source, rule.destination, rule.status
        ));
        let list = self
            .op(|p, c| p.remove_redirect(c, &rule.id))
            .unwrap_or_else(|e| error(&e));
        self.report_staged(
            &format!("Removed {} -> {}", rule.source, rule.destination),
            &list,
            json!({ "removed": rule }),
        );
    }

    fn import(&self, file: &Path) {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| error(&format!("cannot read {}: {e}", file.display())));
        let csv = match file.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("csv") => true,
            Some(e) if e.eq_ignore_ascii_case("json") => false,
            _ => error("import file must end in .json or .csv"),
        };
        let rules = match parse_redirect_import(&text, csv) {
            Ok(r) => r,
            Err(errors) => {
                if self.json {
                    let errs: Vec<_> = errors
                        .iter()
                        .map(|e| json!({ "line": e.line, "message": e.message }))
                        .collect();
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({ "status": "invalid", "errors": errs })
                        )
                        .unwrap()
                    );
                    std::process::exit(1);
                }
                let lines: Vec<String> = errors
                    .iter()
                    .map(|e| format!("{}:{}: {}", file.display(), e.line, e.message))
                    .collect();
                error(&format!(
                    "{} problem(s) in {}; nothing was imported\n{}",
                    errors.len(),
                    file.display(),
                    lines.join("\n")
                ));
            }
        };
        self.confirm(&format!(
            "Import {} redirect(s) from {} as a new staged version? It replaces the staged redirects.",
            rules.len(),
            file.display()
        ));
        let list = self
            .op(|p, c| p.import_redirects(c, &rules))
            .unwrap_or_else(|e| error(&e));
        self.report_staged(
            &format!("Imported {}", ui::fmt::plural(rules.len(), "redirect")),
            &list,
            json!({ "imported": rules.len() }),
        );
    }

    fn versions(&self) {
        let v = self.versions_or_exit();
        if self.json {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            return;
        }
        if v.versions.is_empty() {
            ui::empty("redirect versions", None);
        } else {
            let mut t = ui::Table::new(&["Version", "State", "Created"]);
            for ver in &v.versions {
                t.row(vec![
                    ver.version.to_string(),
                    ui::fmt::status(version_tag(&v, ver.version)),
                    format_time(ver.created_at),
                ]);
            }
            t.print();
        }
        print_state(&v);
    }

    /// `promote` or `restore`: both change what is served, so confirm first.
    fn update_version(&self, version: u64, action: &str) {
        let before = self.versions_or_exit();
        if !before.versions.iter().any(|v| v.version == version) {
            error(&format!(
                "version {version} not found; run `appz redirects versions`"
            ));
        }
        if before.live_version == Some(version) {
            error(&format!("version {version} is already live"));
        }
        if action == "promote" && before.staged_version != Some(version) {
            error(&format!(
                "version {version} is not staged; only the staged version can be promoted (use `appz redirects restore {version}` for older ones)"
            ));
        }
        let change = self.diff_vs_live(&before, version);
        self.confirm(&format!(
            "{} redirect version {version}{}? This changes live redirect behaviour.",
            if action == "promote" {
                "Promote"
            } else {
                "Restore"
            },
            change
                .as_deref()
                .map(|c| format!(" ({c})"))
                .unwrap_or_default()
        ));
        let done = self
            .op(|p, c| p.update_redirect_version(c, version, action))
            .unwrap_or_else(|e| error(&e));
        let after = self.versions_or_exit();
        let past = if action == "promote" {
            "promoted"
        } else {
            "restored"
        };
        if self.json {
            let out = json!({
                "status": past,
                "version": done.version,
                "liveVersion": after.live_version,
                "stagedVersion": after.staged_version,
                "promoteCommand": promote_command(&after),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            return;
        }
        success(&format!("Redirect version {version} {past}"));
        print_state(&after);
    }

    /// "+2 -1" summary of `version` against the live version, if both load.
    fn diff_vs_live(&self, v: &RedirectVersions, version: u64) -> Option<String> {
        let live = self.op(|p, c| p.list_redirects(c, v.live_version)).ok()?;
        let new = self.op(|p, c| p.list_redirects(c, Some(version))).ok()?;
        let key = |r: &RedirectRule| (r.source.clone(), r.destination.clone(), r.status);
        let old: std::collections::HashSet<_> = live.rules.iter().map(key).collect();
        let cur: std::collections::HashSet<_> = new.rules.iter().map(key).collect();
        Some(format!(
            "+{} -{} vs live",
            cur.difference(&old).count(),
            old.difference(&cur).count()
        ))
    }

    /// Output shared by every command that stages a new version.
    fn report_staged(&self, what: &str, list: &RedirectList, extra: serde_json::Value) {
        let state = self.op(|p, c| p.list_redirect_versions(c)).ok();
        if self.json {
            let mut out = json!({
                "status": "staged",
                "version": list.version,
                "rules": list.rules,
            });
            out.as_object_mut()
                .unwrap()
                .extend(extra.as_object().cloned().unwrap_or_default());
            if let Some(v) = &state {
                out["liveVersion"] = json!(v.live_version);
                out["stagedVersion"] = json!(v.staged_version);
                out["promoteCommand"] = json!(promote_command(v));
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            return;
        }
        success(what);
        match &state {
            Some(v) => print_state(v),
            None => {
                if let Some(n) = list.version {
                    warning(&format!(
                        "Staged v{n} (not live yet), run `appz redirects promote {n}`"
                    ));
                }
            }
        }
    }
}

fn promote_command(v: &RedirectVersions) -> Option<String> {
    v.staged_version
        .filter(|s| Some(*s) != v.live_version)
        .map(|s| format!("appz redirects promote {s}"))
}

/// Print where staged and live stand, with the next step when they differ.
fn print_state(v: &RedirectVersions) {
    let live = v
        .live_version
        .map_or("live: none".to_string(), |n| format!("live v{n}"));
    match promote_command(v) {
        Some(cmd) => warning(&format!(
            "Staged v{} (not live yet), {live}; run `{cmd}`",
            v.staged_version.unwrap_or_default()
        )),
        None => ui::hint(&format!("{}; no staged changes", capitalize_first(&live))),
    }
}

fn version_tag(v: &RedirectVersions, n: u64) -> &'static str {
    match (v.live_version == Some(n), v.staged_version == Some(n)) {
        (true, _) => "live",
        (_, true) => "staged",
        _ => "previous",
    }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn rules_table(rules: &[RedirectRule]) -> ui::Table {
    let mut t = ui::Table::new(&["ID", "Source", "Destination", "Status"]);
    for r in rules {
        t.row(vec![
            r.id.clone(),
            r.source.clone(),
            r.destination.clone(),
            r.status.to_string(),
        ]);
    }
    t
}

/// The API may send seconds or milliseconds.
fn format_time(ts: i64) -> String {
    ui::fmt::datetime(ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(live: Option<u64>, staged: Option<u64>) -> RedirectVersions {
        RedirectVersions {
            live_version: live,
            staged_version: staged,
            versions: vec![],
        }
    }

    #[test]
    fn promote_hint_only_when_staged_differs_from_live() {
        assert_eq!(
            promote_command(&versions(Some(4), Some(5))).as_deref(),
            Some("appz redirects promote 5")
        );
        assert_eq!(promote_command(&versions(Some(5), Some(5))), None);
        assert_eq!(promote_command(&versions(Some(5), None)), None);
        assert_eq!(version_tag(&versions(Some(4), Some(5)), 5), "staged");
        assert_eq!(version_tag(&versions(Some(4), Some(5)), 3), "previous");
    }

    #[test]
    fn formats_seconds_and_milliseconds_alike() {
        assert_eq!(format_time(1762732800), format_time(1762732800000));
    }

    #[test]
    fn parses_redirects_commands() {
        use clap::Parser;
        let cli = crate::AppzCli::try_parse_from([
            "appz",
            "redirects",
            "add",
            "/a",
            "/b",
            "--status",
            "301",
        ])
        .unwrap();
        let crate::AppzCmd::Redirects(a) = cli.command else {
            panic!("expected redirects")
        };
        assert!(matches!(a.command, RedirectsCmd::Add { status: 301, .. }));
        for argv in [
            ["appz", "redirects", "ls", "--version", "3"],
            ["appz", "redirects", "rm", "rdr_1", "--yes"],
            ["appz", "redirects", "import", "r.csv", "--yes"],
            ["appz", "redirects", "promote", "5", "--yes"],
        ] {
            assert!(crate::AppzCli::try_parse_from(argv).is_ok(), "{argv:?}");
        }
        assert!(crate::AppzCli::try_parse_from(["appz", "redirects", "promote", "x"]).is_err());
    }
}
