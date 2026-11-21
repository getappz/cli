//! Incremental source snapshots (`backup::create`).
//!
//! Stages selected non-rebuildable files, `appz-manifest.json`, and any local
//! git bundle into a private directory, skips when the latest snapshot's `fp:`
//! tag already matches, warns about `.env*` keys missing from the vault, and
//! invokes `rustic backup` with `--host appz-<project_id>` and spec tags.

// Consumed by upcoming backup tasks (after_deploy, appz backup, restore).
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cap_std::fs::Dir;
use serde::Deserialize;

pub use appz_deploy::BackupSummary;

use super::config;
use super::engine::{EngineError, Rustic};
use super::manifest::{MANIFEST_NAME, Manifest, tags};
use super::select;

/// Inputs describing the snapshot to create.
pub struct CreateInput<'a> {
    pub root: &'a Path,
    pub fingerprint: &'a str,
    pub target: Option<&'a str>,
    pub production: Option<bool>,
    pub url: Option<&'a str>,
    pub build_env_names: Vec<String>,
}

/// Create an incremental backup snapshot for `input.root` using `engine`.
pub fn create(engine: &dyn Rustic, input: CreateInput<'_>) -> Result<BackupSummary, EngineError> {
    create_with(engine, input, crate::secrets::known_names, |msg| {
        crate::warning(msg);
    })
}

pub(super) fn create_quiet_deduped(
    engine: &dyn Rustic,
    input: CreateInput<'_>,
    quiet: bool,
) -> Result<BackupSummary, EngineError> {
    create_with_dedup(
        engine,
        input,
        crate::secrets::known_names,
        |msg| {
            if !quiet {
                crate::warning(msg);
            }
        },
        true,
    )
}

fn create_with(
    engine: &dyn Rustic,
    input: CreateInput<'_>,
    vault_names: impl FnOnce(&Path) -> BTreeSet<String>,
    warn: impl FnMut(&str),
) -> Result<BackupSummary, EngineError> {
    create_with_dedup(engine, input, vault_names, warn, false)
}

fn create_with_dedup(
    engine: &dyn Rustic,
    input: CreateInput<'_>,
    vault_names: impl FnOnce(&Path) -> BTreeSet<String>,
    mut warn: impl FnMut(&str),
    derive_fp_when_empty: bool,
) -> Result<BackupSummary, EngineError> {
    let outputs: Vec<_> = appz_core::detect_toolchains(input.root)
        .map_err(EngineError::Failed)?
        .into_iter()
        .filter_map(|t| t.output_directory)
        .collect();
    let selection = select::select(input.root, &outputs).map_err(EngineError::Failed)?;

    let res = create_from_selection(
        engine,
        &input,
        &selection,
        vault_names,
        &mut warn,
        derive_fp_when_empty,
    );
    if let Some(bundle) = &selection.bundle {
        select::remove_bundle(bundle);
    }
    res
}

fn create_from_selection(
    engine: &dyn Rustic,
    input: &CreateInput<'_>,
    selection: &select::Selection,
    vault_names: impl FnOnce(&Path) -> BTreeSet<String>,
    warn: &mut impl FnMut(&str),
    derive_fp_when_empty: bool,
) -> Result<BackupSummary, EngineError> {
    if !selection.env_names.is_empty() {
        let known = vault_names(input.root);
        for key in &selection.env_names {
            if !known.contains(key) {
                warn(&format!(
                    "{key} is only in .env; run appz env set {key} to keep it"
                ));
            }
        }
    }

    let mut build_env_names = input.build_env_names.clone();
    build_env_names.sort();
    build_env_names.dedup();

    let derived_fp;
    let effective_fp = if !input.fingerprint.is_empty() {
        input.fingerprint
    } else if derive_fp_when_empty {
        derived_fp = select::selection_fingerprint(input.root, selection, &build_env_names)
            .map_err(EngineError::Failed)?;
        derived_fp.as_str()
    } else {
        ""
    };

    let project_id = config::project_id(input.root);
    let host = format!("appz-{project_id}");

    if !effective_fp.is_empty() {
        match latest_snapshot_tags(engine, &host) {
            Ok(Some(last_tags)) if last_tags.iter().any(|t| t == &format!("fp:{effective_fp}")) => {
                return Ok(BackupSummary {
                    snapshot: None,
                    added_bytes: 0,
                    skipped: Some("no changes".to_string()),
                });
            }
            Ok(_) => {}
            Err(EngineError::Locked) => return Ok(locked_summary()),
            Err(err) => return Err(err),
        }
    }

    let files: Vec<String> = selection
        .files
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();

    let mut manifest = Manifest {
        appz_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        target: input.target.map(str::to_string),
        environment: input
            .production
            .map(|p| if p { "production" } else { "preview" }.to_string()),
        url: input.url.map(str::to_string),
        fingerprint: effective_fp.to_string(),
        git: selection.git.clone(),
        tools: BTreeMap::new(),
        env_names: selection.env_names.clone(),
        build_env_names,
        files,
        deleted: selection.deleted.clone(),
        staged_modes: selection.staged_modes(),
        source_fingerprint: None,
        has_bundle: selection.bundle.is_some(),
    };

    let stage = super::private_tempdir().map_err(|e| EngineError::Failed(e.to_string()))?;
    let project = Dir::open_ambient_dir(input.root, cap_std::ambient_authority())
        .map_err(|e| EngineError::Failed(e.to_string()))?;

    for rel in &selection.files {
        super::stage_file(&project, rel, &stage.path().join("worktree").join(rel))
            .map_err(|e| EngineError::Failed(e.to_string()))?;
    }
    for rel in &selection.staged {
        select::save_staged(input.root, rel, stage.path()).map_err(EngineError::Failed)?;
    }
    if let Some(bundle) = &selection.bundle {
        let dir = stage.path().join("git");
        std::fs::create_dir_all(&dir).map_err(|e| EngineError::Failed(e.to_string()))?;
        std::fs::copy(bundle, dir.join("local-commits.bundle"))
            .map_err(|e| EngineError::Failed(e.to_string()))?;
    }

    let worktree = stage.path().join("worktree");
    std::fs::create_dir_all(&worktree).map_err(|e| EngineError::Failed(e.to_string()))?;
    manifest.source_fingerprint = Some(
        source_fingerprint(&worktree, selection, &manifest.build_env_names)
            .map_err(EngineError::Failed)?,
    );
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| EngineError::Failed(e.to_string()))?;
    std::fs::write(stage.path().join(MANIFEST_NAME), manifest_bytes)
        .map_err(|e| EngineError::Failed(e.to_string()))?;

    let snapshot_tags = tags(&manifest);
    let stage_arg =
        super::path_arg(stage.path()).map_err(|e| EngineError::Failed(e.to_string()))?;

    let mut args: Vec<&str> = vec![
        "backup",
        "--force",
        "--json",
        "--as-path",
        "/",
        "--host",
        &host,
    ];
    for tag in &snapshot_tags {
        args.push("--tag");
        args.push(tag.as_str());
    }
    args.push("--");
    args.push(stage_arg);

    let out = match engine.run(&args) {
        Ok(out) => out,
        Err(EngineError::Locked) => return Ok(locked_summary()),
        Err(err) => return Err(err),
    };

    let parsed: serde_json::Value = serde_json::from_str(&out.stdout)
        .map_err(|e| EngineError::Failed(format!("invalid rustic backup JSON output: {e}")))?;

    let id = parsed["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EngineError::Failed("rustic returned no ID for the new snapshot".into()))?
        .to_string();

    let added_bytes = parsed
        .pointer("/summary/data_added")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            parsed
                .pointer("/summary/data_added_packed")
                .and_then(|v| v.as_u64())
        })
        .unwrap_or(0);

    Ok(BackupSummary {
        snapshot: Some(id),
        added_bytes,
        skipped: None,
    })
}

/// Verify reconstructed source independently of branch names and remote reachability.
pub(super) fn source_fingerprint(
    root: &Path,
    selected: &select::Selection,
    env: &[String],
) -> Result<String, String> {
    let mut source = selected.clone();
    source.bundle = None;
    if let Some(info) = &mut source.git {
        info.branch = None;
        info.remote = None;
        info.ahead = 0;
    }
    select::selection_fingerprint(root, &source, env)
}

fn locked_summary() -> BackupSummary {
    BackupSummary {
        snapshot: None,
        added_bytes: 0,
        skipped: Some("repository locked by another rustic process".to_string()),
    }
}

#[derive(Deserialize)]
struct SnapshotMeta {
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    time: Option<chrono::DateTime<chrono::FixedOffset>>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct GroupKey {
    #[serde(default)]
    hostname: Option<String>,
}

fn latest_snapshot_tags(
    engine: &dyn Rustic,
    host: &str,
) -> Result<Option<Vec<String>>, EngineError> {
    let out = match engine.run(&["snapshots", "--json"]) {
        Ok(out) => out,
        Err(EngineError::Failed(msg))
            if msg.contains("No repository config file found")
                || msg.contains("No snapshots found") =>
        {
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Group {
            #[serde(default)]
            group_key: Option<GroupKey>,
            snapshots: Vec<SnapshotMeta>,
        },
        Single(SnapshotMeta),
    }

    let entries: Vec<Entry> = serde_json::from_str(&out.stdout)
        .map_err(|e| EngineError::Failed(format!("invalid rustic snapshots JSON output: {e}")))?;

    let mut all = Vec::new();
    for entry in entries {
        match entry {
            Entry::Group {
                group_key,
                snapshots,
            } => {
                let group_host = group_key.and_then(|g| g.hostname);
                for s in snapshots {
                    let effective_host = s.hostname.as_deref().or(group_host.as_deref());
                    if effective_host == Some(host) {
                        all.push(s);
                    }
                }
            }
            Entry::Single(s) => {
                if s.hostname.as_deref() == Some(host) {
                    all.push(s);
                }
            }
        }
    }

    if all.is_empty() {
        return Ok(None);
    }

    if all.iter().all(|s| s.time.is_some()) {
        all.sort_by_key(|s| s.time);
    }

    Ok(all.pop().map(|s| s.tags))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::backup::engine::RunOut;

    struct RecordingEngine {
        snapshots_reply: Result<String, EngineError>,
        backup_reply: Result<String, EngineError>,
        calls: RefCell<Vec<Vec<String>>>,
        staged_files_seen: RefCell<Vec<String>>,
        manifest_seen: RefCell<Option<Manifest>>,
    }

    impl RecordingEngine {
        fn ok(snapshots_json: &str, backup_json: &str) -> Self {
            Self {
                snapshots_reply: Ok(snapshots_json.to_string()),
                backup_reply: Ok(backup_json.to_string()),
                calls: RefCell::new(Vec::new()),
                staged_files_seen: RefCell::new(Vec::new()),
                manifest_seen: RefCell::new(None),
            }
        }
    }

    impl Rustic for RecordingEngine {
        fn run(&self, args: &[&str]) -> Result<RunOut, EngineError> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| (*s).to_string()).collect());

            if args == ["snapshots", "--json"] {
                return match &self.snapshots_reply {
                    Ok(stdout) => Ok(RunOut {
                        stdout: stdout.clone(),
                        stderr: String::new(),
                    }),
                    Err(EngineError::Locked) => Err(EngineError::Locked),
                    Err(EngineError::Missing) => Err(EngineError::Missing),
                    Err(EngineError::WrongPassword) => Err(EngineError::WrongPassword),
                    Err(EngineError::Failed(msg)) => Err(EngineError::Failed(msg.clone())),
                };
            }

            if let Some(sep) = args.iter().position(|a| *a == "--") {
                let stage = Path::new(args[sep + 1]);
                let manifest_path = stage.join(MANIFEST_NAME);
                if manifest_path.is_file() {
                    let raw = std::fs::read_to_string(&manifest_path).unwrap();
                    *self.manifest_seen.borrow_mut() = Some(serde_json::from_str(&raw).unwrap());
                }
                let wt = stage.join("worktree");
                if wt.is_dir() {
                    let mut collected = Vec::new();
                    collect_rel(&wt, &wt, &mut collected);
                    collected.sort();
                    *self.staged_files_seen.borrow_mut() = collected;
                }
            }

            match &self.backup_reply {
                Ok(stdout) => Ok(RunOut {
                    stdout: stdout.clone(),
                    stderr: String::new(),
                }),
                Err(EngineError::Locked) => Err(EngineError::Locked),
                Err(EngineError::Missing) => Err(EngineError::Missing),
                Err(EngineError::WrongPassword) => Err(EngineError::WrongPassword),
                Err(EngineError::Failed(msg)) => Err(EngineError::Failed(msg.clone())),
            }
        }
    }

    fn collect_rel(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collect_rel(base, &path, out);
            } else {
                out.push(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }

    fn input_for(root: &Path) -> CreateInput<'_> {
        CreateInput {
            root,
            fingerprint: "fp123",
            target: Some("vercel"),
            production: Some(true),
            url: Some("https://app.example.com"),
            build_env_names: vec!["PUBLIC_URL".to_string()],
        }
    }

    #[test]
    fn skips_when_fingerprint_matches_last_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hi</h1>").unwrap();
        let host = format!("appz-{}", config::project_id(dir.path()));
        let snapshots_json = format!(
            r#"[{{"group_key":{{"hostname":"{host}"}},"snapshots":[{{"id":"prev","time":"2026-11-20T00:00:00Z","tags":["appz","fp:fp123"]}}]}}]"#
        );

        let engine = RecordingEngine::ok(
            &snapshots_json,
            r#"{"id":"new","summary":{"data_added":10}}"#,
        );

        let summary =
            create_with(&engine, input_for(dir.path()), |_| BTreeSet::new(), |_| {}).unwrap();

        assert_eq!(
            summary,
            BackupSummary {
                snapshot: None,
                added_bytes: 0,
                skipped: Some("no changes".to_string()),
            }
        );
        assert_eq!(engine.calls.borrow().len(), 1);
        assert_eq!(engine.calls.borrow()[0], vec!["snapshots", "--json"]);
    }

    #[test]
    fn does_not_skip_when_fingerprint_matches_another_host() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hi</h1>").unwrap();

        let engine = RecordingEngine::ok(
            r#"[{"group_key":{"hostname":"appz-otherproject00"},"snapshots":[{"id":"other","time":"2026-11-20T00:00:00Z","tags":["appz","fp:fp123"]}]}]"#,
            r#"{"id":"mine","summary":{"data_added":42}}"#,
        );

        let summary =
            create_with(&engine, input_for(dir.path()), |_| BTreeSet::new(), |_| {}).unwrap();

        assert_eq!(summary.snapshot.as_deref(), Some("mine"));
        assert_eq!(summary.added_bytes, 42);
        assert_eq!(engine.calls.borrow().len(), 2);
    }

    #[test]
    fn passes_host_tags_and_relative_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join(".env"), "API_KEY=secret").unwrap();

        let engine = RecordingEngine::ok("[]", r#"{"id":"snap1","summary":{"data_added":512}}"#);
        let expected_host = format!("appz-{}", config::project_id(dir.path()));

        let summary = create_with(
            &engine,
            input_for(dir.path()),
            |_| BTreeSet::from(["API_KEY".to_string()]),
            |_| {},
        )
        .unwrap();
        assert_eq!(summary.snapshot.as_deref(), Some("snap1"));

        let calls = engine.calls.borrow();
        let backup_args = &calls[1];
        assert!(
            backup_args
                .windows(2)
                .any(|w| w == ["--host", expected_host.as_str()])
        );
        assert!(backup_args.windows(2).any(|w| w == ["--tag", "appz"]));
        assert!(
            backup_args
                .windows(2)
                .any(|w| w == ["--tag", "target:vercel"])
        );
        assert!(
            backup_args
                .windows(2)
                .any(|w| w == ["--tag", "env:production"])
        );
        assert!(backup_args.windows(2).any(|w| w == ["--tag", "fp:fp123"]));
        assert_eq!(*engine.staged_files_seen.borrow(), vec!["src/main.rs"]);

        let manifest = engine.manifest_seen.borrow().clone().unwrap();
        assert_eq!(manifest.files, vec!["src/main.rs"]);
        assert_eq!(manifest.env_names, vec!["API_KEY"]);
        assert_eq!(manifest.build_env_names, vec!["PUBLIC_URL"]);
    }

    #[test]
    fn reports_snapshot_id_and_added_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.js"), "console.log(1);").unwrap();

        let engine = RecordingEngine::ok(
            "[]",
            r#"{"id":"abc987","summary":{"data_added":4096,"total_files_processed":2}}"#,
        );

        let summary =
            create_with(&engine, input_for(dir.path()), |_| BTreeSet::new(), |_| {}).unwrap();

        assert_eq!(
            summary,
            BackupSummary {
                snapshot: Some("abc987".to_string()),
                added_bytes: 4096,
                skipped: None,
            }
        );
    }

    #[test]
    fn lock_error_becomes_skip_note_not_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("app.js"), "console.log(1);").unwrap();

        let engine = RecordingEngine {
            snapshots_reply: Ok("[]".to_string()),
            backup_reply: Err(EngineError::Locked),
            calls: RefCell::new(Vec::new()),
            staged_files_seen: RefCell::new(Vec::new()),
            manifest_seen: RefCell::new(None),
        };

        let summary =
            create_with(&engine, input_for(dir.path()), |_| BTreeSet::new(), |_| {}).unwrap();

        assert_eq!(summary.snapshot, None);
        assert_eq!(summary.added_bytes, 0);
        assert!(
            summary
                .skipped
                .as_deref()
                .is_some_and(|s| s.contains("locked")),
            "unexpected skipped: {:?}",
            summary.skipped
        );
    }

    #[test]
    fn spaces_in_root_are_one_argument() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("my spaced project");
        std::fs::create_dir_all(root.join("dir with spaces")).unwrap();
        std::fs::write(root.join("dir with spaces/my file.txt"), "hello").unwrap();

        let engine = RecordingEngine::ok("[]", r#"{"id":"s1","summary":{"data_added":128}}"#);

        let summary = create_with(&engine, input_for(&root), |_| BTreeSet::new(), |_| {}).unwrap();
        assert_eq!(summary.snapshot.as_deref(), Some("s1"));

        let calls = engine.calls.borrow();
        let backup_args = &calls[1];
        let sep = backup_args.iter().position(|a| a == "--").unwrap();
        assert_eq!(backup_args.len() - (sep + 1), 1);
        assert_eq!(
            *engine.staged_files_seen.borrow(),
            vec!["dir with spaces/my file.txt"]
        );
    }

    #[test]
    fn warns_for_env_names_missing_from_vault() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".env"),
            "PRESENT_KEY=keep_me_secret\nMISSING_KEY=super_secret_value\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".env.local"),
            "MISSING_KEY=duplicate_in_local\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("index.html"), "ok").unwrap();

        let engine = RecordingEngine::ok("[]", r#"{"id":"s2","summary":{"data_added":64}}"#);
        let mut warnings = Vec::new();

        create_with(
            &engine,
            input_for(dir.path()),
            |_| BTreeSet::from(["PRESENT_KEY".to_string()]),
            |msg| warnings.push(msg.to_string()),
        )
        .unwrap();

        assert_eq!(
            warnings,
            vec!["MISSING_KEY is only in .env; run appz env set MISSING_KEY to keep it"]
        );
        assert!(warnings.iter().all(|w| !w.contains("super_secret_value")));
    }

    #[test]
    #[ignore]
    fn round_trip_second_backup_after_one_file_edit_adds_under_64kib() {
        let project = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join("a.txt"), "line 1\n".repeat(1000)).unwrap();

        let bin = super::super::engine::find_rustic(project.path())
            .expect("rustic binary required for ignored integration test");
        let mut raw = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
        let runtime_secret: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let cli =
            super::super::engine::RusticCli::new(bin, repo.path(), runtime_secret, project.path());
        cli.run(&["init"]).unwrap();

        let first = create(
            &cli,
            CreateInput {
                root: project.path(),
                fingerprint: "fp-1",
                target: None,
                production: None,
                url: None,
                build_env_names: vec![],
            },
        )
        .unwrap();
        assert!(first.snapshot.is_some());

        std::fs::write(project.path().join("b.txt"), "small edit\n").unwrap();
        let second = create(
            &cli,
            CreateInput {
                root: project.path(),
                fingerprint: "fp-2",
                target: None,
                production: None,
                url: None,
                build_env_names: vec![],
            },
        )
        .unwrap();
        assert!(second.snapshot.is_some());
        assert!(second.added_bytes < 64 * 1024);
    }

    #[test]
    fn create_quiet_deduped_derives_selection_fingerprint_and_skips_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hello</h1>").unwrap();

        let first_engine =
            RecordingEngine::ok("[]", r#"{"id":"snap-1","summary":{"data_added":64}}"#);
        let first = create_quiet_deduped(
            &first_engine,
            CreateInput {
                root: dir.path(),
                fingerprint: "",
                target: Some("vercel"),
                production: Some(false),
                url: None,
                build_env_names: vec!["PUBLIC_URL".into()],
            },
            true,
        )
        .unwrap();
        assert_eq!(first.snapshot.as_deref(), Some("snap-1"));
        let fp = first_engine
            .manifest_seen
            .borrow()
            .as_ref()
            .expect("manifest staged")
            .fingerprint
            .clone();
        assert_eq!(fp.len(), 64);

        let host = format!("appz-{}", config::project_id(dir.path()));
        let second_snapshots = format!(
            r#"[{{"group_key":{{"hostname":"{host}"}},"snapshots":[{{"id":"snap-1","time":"2026-11-20T00:00:00Z","tags":["appz","fp:{fp}"]}}]}}]"#
        );
        let second_engine = RecordingEngine::ok(
            &second_snapshots,
            r#"{"id":"snap-2","summary":{"data_added":64}}"#,
        );
        let second = create_quiet_deduped(
            &second_engine,
            CreateInput {
                root: dir.path(),
                fingerprint: "",
                target: Some("vercel"),
                production: Some(false),
                url: None,
                build_env_names: vec!["PUBLIC_URL".into()],
            },
            true,
        )
        .unwrap();
        assert_eq!(second.snapshot, None);
        assert_eq!(second.skipped.as_deref(), Some("no changes"));
        assert_eq!(second_engine.calls.borrow().len(), 1);
    }
}
