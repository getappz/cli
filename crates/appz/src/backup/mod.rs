//! Source backups through the `rustic` CLI.
//!
//! Only the source that cannot be rebuilt is snapshotted (see [`select`]);
//! working-tree environment files are excluded. Git bundles preserve committed history.

pub mod config;
pub mod engine;
pub mod manifest;
pub mod repo;
pub mod restore;
pub mod retention;
pub mod select;
pub mod snapshot;

#[allow(unused_imports)]
pub use repo::{Interactive, PASSWORD_ENV, open_repo, vault_key};
#[allow(unused_imports)]
pub use snapshot::{BackupSummary, CreateInput, create as create_snapshot};

/// Parameters for best-effort post-deploy source backup.
pub struct AfterDeploy<'a> {
    pub fingerprint: &'a str,
    pub target: &'a str,
    pub production: bool,
    pub url: Option<&'a str>,
    pub build_env_names: Vec<String>,
    pub interactive: Interactive,
    pub json: bool,
}

/// Best-effort source backup + retention after a successful deploy.
///
/// Never returns an error so a backup failure never fails or alters the deploy
/// result; emits one-line notes to stderr unless `req.json` is true.
enum RepoOpen<E> {
    Opened(E),
    Skipped(String),
}

pub fn after_deploy(root: &Path, req: &AfterDeploy<'_>) -> Option<BackupSummary> {
    after_deploy_with(
        root,
        req,
        |cfg| {
            let mut skip_note = None;
            let res = repo::open_repo_noting(root, cfg, req.interactive, |msg| {
                skip_note = Some(msg.to_string());
                if !req.json {
                    crate::ui::hint(msg);
                }
            });
            match res {
                Ok(Some(cli)) => Ok(RepoOpen::Opened(cli)),
                Ok(None) => Ok(RepoOpen::Skipped(
                    skip_note.unwrap_or_else(|| "backup skipped".to_string()),
                )),
                Err(e) => Err(e),
            }
        },
        |msg| {
            if !req.json {
                crate::ui::hint(msg);
            }
        },
    )
}

fn after_deploy_with<E: Rustic>(
    root: &Path,
    req: &AfterDeploy<'_>,
    open: impl FnOnce(&config::BackupConfig) -> std::result::Result<RepoOpen<E>, String>,
    mut note: impl FnMut(&str),
) -> Option<BackupSummary> {
    let mut cfg = match config::read_config(root) {
        Ok(cfg) => cfg,
        Err(err) => {
            note(&format!("backup skipped: {err}"));
            return Some(BackupSummary {
                snapshot: None,
                added_bytes: 0,
                skipped: Some(err),
            });
        }
    };
    if !cfg.is_enabled() {
        return None;
    }

    let engine = match resolve_repository(root, &mut cfg)
        .map_err(|e| e.to_string())
        .and_then(|()| open(&cfg))
    {
        Ok(RepoOpen::Opened(engine)) => engine,
        Ok(RepoOpen::Skipped(reason)) => {
            return Some(BackupSummary {
                snapshot: None,
                added_bytes: 0,
                skipped: Some(reason),
            });
        }
        Err(err) => {
            note(&format!("backup skipped: {err}"));
            return Some(BackupSummary {
                snapshot: None,
                added_bytes: 0,
                skipped: Some(err),
            });
        }
    };

    let summary = match snapshot::create_quiet_deduped(
        &engine,
        CreateInput {
            root,
            fingerprint: req.fingerprint,
            target: Some(req.target),
            production: Some(req.production),
            url: req.url,
            build_env_names: req.build_env_names.clone(),
        },
        req.json,
    ) {
        Ok(summary) => summary,
        Err(err) => {
            let msg = err.to_string();
            note(&format!("backup skipped: {msg}"));
            return Some(BackupSummary {
                snapshot: None,
                added_bytes: 0,
                skipped: Some(msg),
            });
        }
    };

    if summary.snapshot.is_some() {
        let repo = config::repo_location(&cfg, root);
        if let Err(err) = retention::apply(&engine, &cfg.retention, root, &repo) {
            note(&format!("backup retention skipped: {err}"));
        }
    }

    Some(summary)
}

#[cfg(test)]
pub(crate) fn run_after_deploy_failed_fixture(
    root: &Path,
    req: &AfterDeploy<'_>,
) -> Option<BackupSummary> {
    struct FailingEngine;
    impl Rustic for FailingEngine {
        fn run(&self, _: &[&str]) -> std::result::Result<engine::RunOut, engine::EngineError> {
            Err(engine::EngineError::Failed("boom".to_string()))
        }
    }
    let mut notes = Vec::new();
    after_deploy_with(
        root,
        req,
        |_| Ok(RepoOpen::Opened(FailingEngine)),
        |m| notes.push(m.to_string()),
    )
}

use std::path::{Path, PathBuf};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::Dir;
use clap::{Args, Subcommand};
use engine::{Rustic, RusticCli};
use serde_json::json;
#[cfg(test)]
use sha2::{Digest, Sha256};

#[derive(Args)]
#[command(
    after_help = "Uses ~/.appz/backups/<project-id>/ by default (or appz.jsonc backup.repository / APPZ_BACKUP_REPOSITORY). Set APPZ_BACKUP_PASSWORD or let interactive `appz backup` store a generated password in the global vault."
)]
pub struct BackupArgs {
    /// Project directory (relative repository and restore paths resolve here)
    #[arg(long, global = true, default_value = ".")]
    dir: PathBuf,
    #[command(subcommand)]
    command: Option<BackupCmd>,
}

#[derive(Subcommand)]
enum BackupCmd {
    /// Initialize an empty backup repository
    Init,
    /// Back up selected source and local commits (default), initializing the repository if needed
    Create,
    /// List snapshots in the repository
    #[command(alias = "ls")]
    List,
    /// Restore a snapshot (ID or latest) into a destination directory
    Restore {
        snapshot: String,
        destination: PathBuf,
    },
    /// Apply backup.retention from appz.jsonc to the entire repository
    Prune,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Structured output for `appz backup ls` / `backup::list`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ListOutput {
    pub repository: String,
    pub off_machine: bool,
    pub size: u64,
    pub max_size: u64,
    pub snapshots: Vec<SnapshotRow>,
}

/// One snapshot row in [`ListOutput`], derived from snapshot tags and summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SnapshotRow {
    pub id: String,
    pub time: String,
    pub target: Option<String>,
    pub environment: Option<String>,
    pub commit: Option<String>,
    pub url: Option<String>,
    pub added_bytes: u64,
}

fn is_off_machine(repository: &str) -> bool {
    let trimmed = repository.trim();
    if trimmed.is_empty() || trimmed == "local" || trimmed.starts_with("local:") {
        return false;
    }
    if Path::new(trimmed).is_absolute() {
        return false;
    }
    if trimmed
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && trimmed.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    trimmed
        .split_once(':')
        .is_some_and(|(backend, _)| !backend.is_empty() && !backend.contains(['/', '\\']))
}

fn tag_value(tags: &[String], prefix: &str) -> Option<String> {
    tags.iter()
        .find_map(|tag| tag.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// List snapshots and repository usage for `root`.
pub fn list(
    engine: &dyn Rustic,
    cfg: &config::BackupConfig,
    root: &Path,
) -> std::result::Result<ListOutput, engine::EngineError> {
    let repository = config::repo_location(cfg, root);
    let off_machine = is_off_machine(&repository);
    let max_size = cfg
        .retention
        .max_size
        .as_deref()
        .map(config::parse_size)
        .transpose()
        .map_err(engine::EngineError::Failed)?
        .unwrap_or(0);
    let size = retention::query_repo_size(engine)?;
    let host = format!("appz-{}", config::project_id(root));
    let mut snapshots = snapshot::list_host_snapshots(engine, &host)?;
    snapshots.reverse();
    let rows = snapshots
        .into_iter()
        .map(|meta| {
            let added_bytes = meta
                .summary
                .as_ref()
                .and_then(|s| s.data_added.or(s.data_added_packed))
                .unwrap_or(0);
            SnapshotRow {
                id: meta.id,
                time: meta.time.unwrap_or_default(),
                target: tag_value(&meta.tags, "target:"),
                environment: tag_value(&meta.tags, "env:"),
                commit: tag_value(&meta.tags, "commit:"),
                url: tag_value(&meta.tags, "url:"),
                added_bytes,
            }
        })
        .collect();

    Ok(ListOutput {
        repository,
        off_machine,
        size,
        max_size,
        snapshots: rows,
    })
}

/// Create a manual source backup snapshot (with selection-based dedup) and apply retention when added.
pub fn manual_create(
    engine: &dyn Rustic,
    cfg: &config::BackupConfig,
    root: &Path,
    quiet: bool,
) -> std::result::Result<(BackupSummary, Option<retention::RetentionReport>), engine::EngineError> {
    let summary = snapshot::create_quiet_deduped(
        engine,
        CreateInput {
            root,
            fingerprint: "",
            target: None,
            production: None,
            url: None,
            build_env_names: vec![],
        },
        quiet,
    )?;

    let retention_report = if summary.snapshot.is_some() {
        let repository = config::repo_location(cfg, root);
        match retention::apply(engine, &cfg.retention, root, &repository) {
            Ok(report) => Some(report),
            Err(err) => {
                if !quiet {
                    crate::warning(&format!("Backup retention skipped: {err}"));
                }
                None
            }
        }
    } else {
        None
    };

    Ok((summary, retention_report))
}

fn open_cli(
    root: &Path,
    cfg: &config::BackupConfig,
    interactive: Interactive,
    allow_disabled: bool,
) -> std::result::Result<std::result::Result<RusticCli, String>, String> {
    let mut effective = cfg.clone();
    if allow_disabled {
        effective.enabled = None;
    }
    let mut skip_note = None;
    match repo::open_repo_noting(root, &effective, interactive, |msg| {
        skip_note = Some(msg.to_string());
    })? {
        Some(cli) => Ok(Ok(cli)),
        None => Ok(Err(skip_note.unwrap_or_else(|| {
            "backup skipped: disabled in configuration".to_string()
        }))),
    }
}

pub fn run(args: BackupArgs, json_output: bool) -> Result<()> {
    let root = appz_core::canonicalize(&args.dir)?;
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()).into());
    }
    let mut cfg = config::read_config(&root)?;
    resolve_repository(&root, &mut cfg)?;
    let interactive = if !json_output && crate::is_tty() {
        Interactive::Yes
    } else {
        Interactive::No
    };

    match args.command.unwrap_or(BackupCmd::Create) {
        BackupCmd::Create => {
            if !cfg.is_enabled() {
                let summary = BackupSummary {
                    snapshot: None,
                    added_bytes: 0,
                    skipped: Some("backup is disabled in appz.jsonc".to_string()),
                };
                if json_output {
                    println!("{}", serde_json::to_string(&summary)?);
                } else {
                    crate::info("Backup skipped (disabled in appz.jsonc)");
                }
                return Ok(());
            }
            let cli = match open_cli(&root, &cfg, interactive, false)? {
                Ok(cli) => cli,
                Err(reason) => {
                    let summary = BackupSummary {
                        snapshot: None,
                        added_bytes: 0,
                        skipped: Some(reason.clone()),
                    };
                    if json_output {
                        println!("{}", serde_json::to_string(&summary)?);
                    } else {
                        crate::warning(&reason);
                    }
                    return Ok(());
                }
            };
            let (summary, retention_report) = manual_create(&cli, &cfg, &root, json_output)?;
            if json_output {
                println!("{}", serde_json::to_string(&summary)?);
            } else if let Some(id) = &summary.snapshot {
                crate::success(&format!(
                    "Snapshot {id} created ({} added)",
                    crate::ui::fmt::bytes(summary.added_bytes)
                ));
                if retention_report.is_some_and(|r| r.over_cap) {
                    crate::warning("Repository still exceeds the configured size limit");
                }
            } else if summary.skipped.as_deref() == Some("no changes") {
                crate::ui::hint("Nothing new to back up (no changes)");
            } else if let Some(reason) = &summary.skipped {
                crate::warning(&format!("Backup skipped: {reason}"));
            }
        }
        BackupCmd::Init => {
            let _cli = open_cli(&root, &cfg, Interactive::Yes, false)?
                .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?;
            report(
                json_output,
                "Repository initialized",
                json!({"status":"initialized","repository":config::repo_location(&cfg, &root)}),
            );
        }
        BackupCmd::List => {
            let cli = open_cli(&root, &cfg, Interactive::No, true)?
                .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?
                .with_query_timeout();
            let out = list(&cli, &cfg, &root)?;
            if json_output {
                println!("{}", serde_json::to_string(&out)?);
            } else {
                print_list_human(&out);
            }
        }
        BackupCmd::Restore {
            snapshot,
            destination,
        } => {
            let cli = open_cli(&root, &cfg, Interactive::No, true)?
                .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?;
            cli.run(&["restore", "--", &snapshot, path_arg(&destination)?])?;
            report(
                json_output,
                "Snapshot restored",
                json!({"status":"restored","destination":destination}),
            );
        }
        BackupCmd::Prune => {
            let cli = open_cli(&root, &cfg, Interactive::No, true)?
                .map_err(|reason| -> Box<dyn std::error::Error> { reason.into() })?;
            let repo = config::repo_location(&cfg, &root);
            let result = retention::apply(&cli, &cfg.retention, &root, &repo)?;
            let message = if result.ran {
                "Retention applied"
            } else {
                "Retention already ran today"
            };
            report(json_output, message, serde_json::to_value(&result)?);
            if result.over_cap && !json_output {
                crate::warning("Repository still exceeds the configured size limit");
            }
        }
    }
    Ok(())
}

/// Repository summary plus, when there are snapshots, the snapshot table.
fn list_view(out: &ListOutput, now_ms: i64) -> String {
    let location_kind = if out.off_machine {
        "off-machine"
    } else {
        "local"
    };
    let size = if out.max_size > 0 {
        format!(
            "{} / {}",
            crate::ui::fmt::bytes(out.size),
            crate::ui::fmt::bytes(out.max_size)
        )
    } else {
        crate::ui::fmt::bytes(out.size)
    };
    let mut view = crate::ui::table::kv(&[
        (
            "Repository",
            format!("{} ({location_kind})", out.repository),
        ),
        ("Size", size),
    ]);
    if out.snapshots.is_empty() {
        return view;
    }
    let mut table = crate::ui::Table::new(&["Snapshot", "Age", "Size", "Target", "Commit"]);
    for row in &out.snapshots {
        let age = chrono::DateTime::parse_from_rfc3339(&row.time)
            .map(|t| crate::ui::fmt::age(t.timestamp_millis(), now_ms))
            .unwrap_or_default();
        let commit = row
            .commit
            .as_deref()
            .map(|s| s[..s.len().min(7)].to_string())
            .unwrap_or_default();
        table.row(vec![
            row.id.clone(),
            age,
            crate::ui::fmt::bytes(row.added_bytes),
            row.target.clone().unwrap_or_default(),
            commit,
        ]);
    }
    view.push_str("\n");
    view.push_str(&table.render(None));
    view
}

fn print_list_human(out: &ListOutput) {
    print!("{}", list_view(out, crate::ui::fmt::now_ms()));
    if out.snapshots.is_empty() {
        crate::ui::empty("snapshots", Some("Run `appz backup` to create one"));
    }
    if !out.off_machine {
        crate::ui::hint(
            "Local repository protects against git clean, bad edits, and deleted folders, not against losing the machine",
        );
    }
}

fn report(json_output: bool, message: &str, data: serde_json::Value) {
    if json_output {
        println!("{data}");
    } else {
        crate::success(message);
    }
}

pub(super) fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| format!("Rustic requires a UTF-8 path: {}", path.display()).into())
}

fn resolve_repository(root: &Path, cfg: &mut config::BackupConfig) -> Result<()> {
    let repository = match engine::env_repository() {
        Some(path) => path_arg(&path)?.to_owned(),
        None => config::repo_location(cfg, root),
    };
    cfg.repository = Some(repository_location(
        &appz_core::canonicalize(root)?,
        &repository,
    )?);
    Ok(())
}

// Local repositories must be disjoint from the source, including through symlinks.
fn repository_location(root: &Path, repo: &str) -> Result<String> {
    let explicit_local = repo.strip_prefix("local:");
    let local = Path::new(explicit_local.unwrap_or(repo));
    if !local.is_absolute()
        && matches!(
            local.components().next(),
            Some(std::path::Component::Prefix(_))
        )
    {
        return Err("drive-relative backup repository path: use an absolute path with a drive and directory".into());
    }
    if explicit_local.is_none()
        && !local.is_absolute()
        && repo
            .split_once(':')
            .is_some_and(|(backend, _)| !backend.contains(std::path::is_separator))
    {
        return Ok(repo.to_string());
    }
    if local.as_os_str().is_empty() {
        return Err("empty local backup repository path".into());
    }
    let mut path = PathBuf::new();
    for component in root.join(local).components() {
        match component {
            std::path::Component::ParentDir => {
                path.pop();
            }
            std::path::Component::CurDir => {}
            other => path.push(other.as_os_str()),
        }
        if path.is_absolute() {
            match appz_core::canonicalize(&path) {
                Ok(resolved) => path = resolved,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    if path.starts_with(root) || root.starts_with(&path) {
        return Err("choose a dedicated backup repository outside the project".into());
    }
    let path = path_arg(&path)?;
    if cfg!(unix) && path.contains(':') {
        return Ok(format!("local:{path}"));
    }
    Ok(path.to_owned())
}

#[cfg(test)]
fn create(cli: &impl Rustic, root: &Path, json_output: bool) -> Result<()> {
    let project = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    let outputs: Vec<_> = appz_core::detect_toolchains(root)?
        .iter()
        .filter_map(|t| t.output_directory)
        .collect();
    let selection = select::select(root, &outputs)?;
    let snapshot = write_snapshot(cli, root, &project, &selection, json_output);
    if let Some(path) = &selection.bundle {
        select::remove_bundle(path);
    }
    let snapshot = snapshot?;
    let (status, message) = if snapshot.is_some() {
        ("created", "Snapshot created")
    } else {
        ("unchanged", "Nothing new to back up")
    };
    report(
        json_output,
        message,
        json!({
            "status":status, "snapshot":snapshot, "env_names":selection.env_names, "git":selection.git
        }),
    );
    Ok(())
}

pub(super) fn private_tempdir() -> std::io::Result<tempfile::TempDir> {
    private_tempdir_in(&std::env::temp_dir())
}

pub(super) fn private_tempdir_in(parent: &Path) -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("appz-backup-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent)
}

#[cfg(test)]
fn write_snapshot(
    cli: &impl Rustic,
    root: &Path,
    project: &Dir,
    selection: &select::Selection,
    json_output: bool,
) -> Result<Option<serde_json::Value>> {
    if selection.files.is_empty() && selection.staged.is_empty() && selection.bundle.is_none() {
        return Ok(None);
    }
    let stage = private_tempdir()?;
    for rel in &selection.files {
        stage_file(project, rel, &stage.path().join("worktree").join(rel))?;
    }
    for rel in &selection.staged {
        select::save_staged(root, rel, stage.path())?;
    }
    if let Some(bundle) = &selection.bundle {
        let mut hash = Sha256::new();
        std::io::copy(&mut std::fs::File::open(bundle)?, &mut hash)?;
        let dir = stage
            .path()
            .join("git")
            .join(format!("{:x}", hash.finalize()));
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join("local-commits.bundle");
        std::fs::copy(bundle, &dest)?;
        filetime::set_file_mtime(dest, std::time::UNIX_EPOCH.into())?;
    }
    let mut digest = Sha256::new();
    let expected = fingerprint_tree(stage.path(), stage.path(), &mut digest)?;
    let selected =
        selection.files.len() + selection.staged.len() + usize::from(selection.bundle.is_some());
    if expected != selected as u64 {
        return Err(format!("staged {expected} files from {selected} selected sources").into());
    }
    let description = format!("appz-source-v1:{:x}", digest.finalize());
    // Fresh staging inodes cannot identify unchanged content. Compare the latest
    // snapshot's byte fingerprint, then force Rustic to read every changed tree.
    match cli.run(&["snapshots", "--json", "latest"]) {
        Ok(out) => {
            let latest: serde_json::Value = serde_json::from_str(&out.stdout)?;
            let snapshot = &latest[0]["snapshots"][0];
            if snapshot["description"].as_str() == Some(&description)
                && snapshot["summary"]["total_files_processed"].as_u64() == Some(expected)
            {
                return Ok(None);
            }
        }
        Err(engine::EngineError::Failed(msg))
            if msg.contains("No repository config file found")
                || msg.contains("No snapshots found") => {}
        Err(err) => return Err(err.into()),
    }
    let out = cli.run(&[
        "backup",
        "--init",
        "--force",
        "--description",
        &description,
        "--json",
        "--as-path",
        "/",
        "--set-ctime",
        "no",
        "--set-devid",
        "no",
        "--",
        path_arg(stage.path())?,
    ])?;
    if !json_output {
        eprint!("{}", out.stderr);
    }

    let snapshot: serde_json::Value = serde_json::from_str(&out.stdout)?;
    let count = snapshot
        .pointer("/summary/total_files_processed")
        .and_then(|v| v.as_u64());
    if count != Some(expected) {
        let reported = count.map_or_else(|| "no count".into(), |n| n.to_string());
        return Err(format!(
            "incomplete backup (snapshot {}): expected {expected} files, Rustic reported {reported}. {}",
            snapshot["id"].as_str().unwrap_or("unsaved"), out.stderr.trim()
        ).into());
    }
    if !snapshot["id"].is_string() {
        return Err("Rustic returned no ID for the new snapshot".into());
    }
    Ok(Some(snapshot))
}

pub(super) fn stage_file(project: &Dir, rel: &Path, dest: &Path) -> Result<()> {
    let mut parent = project.try_clone()?;
    let mut components = rel.components().peekable();
    let name = loop {
        let Some(std::path::Component::Normal(name)) = components.next() else {
            return Err(format!("invalid selected source: {}", rel.display()).into());
        };
        if components.peek().is_none() {
            break name;
        }
        parent = parent.open_dir_nofollow(name)?;
    };
    std::fs::create_dir_all(dest.parent().unwrap())?;
    let meta = parent.symlink_metadata(name)?;
    if meta.is_symlink() {
        let target = parent.read_link_contents(name)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, dest)?;
        #[cfg(windows)]
        {
            use cap_std::fs::MetadataExt;
            const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
            if meta.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
                std::os::windows::fs::symlink_dir(target, dest)?;
            } else {
                std::os::windows::fs::symlink_file(target, dest)?;
            }
        }
        let time = meta.modified()?.into_std().into();
        filetime::set_symlink_file_times(dest, time, time)?;
    } else {
        if !meta.is_file() {
            return Err(format!("unsupported backup source: {}", rel.display()).into());
        }
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let mut input = parent.open_with(name, &options)?.into_std();
        let meta = input.metadata()?;
        if !meta.is_file() {
            return Err(format!("expected a regular file: {}", rel.display()).into());
        }
        let mut output = std::fs::File::create_new(dest)?;
        std::io::copy(&mut input, &mut output)?;
        output.set_modified(meta.modified()?)?;
        output.set_permissions(meta.permissions())?;
    }
    Ok(())
}

/// Fingerprint only restored metadata and bytes, independent of temporary names and inodes.
#[cfg(test)]
fn fingerprint_tree(root: &Path, dir: &Path, hash: &mut Sha256) -> Result<u64> {
    let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut count = 0;
    for entry in entries {
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            count += fingerprint_tree(root, &path, hash)?;
            continue;
        }
        #[cfg(unix)]
        let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions());
        #[cfg(not(unix))]
        let mode = meta.permissions().readonly();
        let modified = filetime::FileTime::from_last_modification_time(&meta);
        serde_json::to_writer(
            &mut *hash,
            &(
                path.strip_prefix(root)?.as_os_str().as_encoded_bytes(),
                mode,
                modified.unix_seconds(),
                modified.nanoseconds(),
                meta.is_symlink(),
                meta.len(),
            ),
        )?;
        if meta.is_symlink() {
            serde_json::to_writer(
                &mut *hash,
                std::fs::read_link(&path)?.as_os_str().as_encoded_bytes(),
            )?;
        } else {
            std::io::copy(&mut std::fs::File::open(&path)?, hash)?;
        }
        count += 1;
    }
    filetime::set_file_mtime(dir, std::time::UNIX_EPOCH.into())?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{EngineError, RunOut};

    fn list_fixture(off_machine: bool, max_size: u64, snapshots: Vec<SnapshotRow>) -> ListOutput {
        ListOutput {
            repository: "local:.appz/backup".into(),
            off_machine,
            size: 4096,
            max_size,
            snapshots,
        }
    }

    #[test]
    fn list_view_renders_header_and_snapshot_table() {
        console::set_colors_enabled(false);
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-20T15:00:00Z")
            .unwrap()
            .timestamp_millis();
        let out = list_fixture(
            false,
            1_048_576,
            vec![SnapshotRow {
                id: "abc12345".into(),
                time: "2026-11-20T12:00:00Z".into(),
                target: Some("vercel".into()),
                environment: None,
                commit: Some("0123456789".into()),
                url: None,
                added_bytes: 2048,
            }],
        );
        assert_eq!(
            list_view(&out, now),
            "  Repository  local:.appz/backup (local)\n  Size        4.0 KB / 1.0 MB\n\n  Snapshot   Age   Size     Target   Commit\n  abc12345   3h    2.0 KB   vercel   0123456\n"
        );
    }

    #[test]
    fn list_view_without_snapshots_is_header_only() {
        console::set_colors_enabled(false);
        let out = list_fixture(true, 0, vec![]);
        assert_eq!(
            list_view(&out, 0),
            "  Repository  local:.appz/backup (off-machine)\n  Size        4.0 KB\n"
        );
    }

    #[derive(Default)]
    struct SnapshotFixture(std::cell::Cell<u32>, bool);

    impl Rustic for SnapshotFixture {
        fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
            if args == ["snapshots", "--json", "latest"] {
                return Err(EngineError::Failed("No snapshots found".into()));
            }
            assert!(args.contains(&"--force"));
            self.0.set(self.0.get() + 1);
            let (_, paths) = args.split_at(args.iter().position(|arg| *arg == "--").unwrap() + 1);
            assert_eq!(paths.len(), 1);
            assert!(Path::new(paths[0]).is_absolute());
            assert_eq!(
                std::fs::read(Path::new(paths[0]).join("worktree/two words.txt")).unwrap(),
                b"work"
            );
            assert!(!Path::new(paths[0]).join("worktree/.env").exists());
            Ok(RunOut {
                stdout:
                    json!({"id":"fixture","summary":{"total_files_processed":u64::from(!self.1)}})
                        .to_string(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn create_passes_only_selected_sources_and_skips_empty_snapshots() {
        let project = tempfile::tempdir().unwrap();
        let engine = SnapshotFixture::default();
        std::fs::write(project.path().join(".env"), "TOKEN=secret").unwrap();
        create(&engine, project.path(), true).unwrap();
        assert_eq!(engine.0.get(), 0);
        std::fs::write(project.path().join("two words.txt"), "work").unwrap();
        create(&engine, project.path(), true).unwrap();
        assert_eq!(engine.0.get(), 1);
    }

    #[test]
    fn incomplete_snapshot_reports_its_id() {
        let project = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join("two words.txt"), "work").unwrap();
        let engine = SnapshotFixture(Default::default(), true);
        let err = create(&engine, project.path(), true).unwrap_err();
        assert!(
            err.to_string()
                .contains("incomplete backup (snapshot fixture)")
        );
    }

    #[test]
    fn local_repository_cannot_overlap_the_project() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let root = appz_core::canonicalize(&project).unwrap();
        for repo in [".", "..", "local:./state/future", "../project/backup"] {
            assert!(repository_location(&root, repo).is_err(), "{repo}");
        }
        assert_eq!(
            repository_location(&root, "local:../repo").unwrap(),
            root.parent().unwrap().join("repo").to_str().unwrap()
        );
        assert_eq!(
            repository_location(&root, "rclone:remote:bucket").unwrap(),
            "rclone:remote:bucket"
        );
    }

    #[test]
    fn after_deploy_respects_disabled_config_and_swallows_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("appz.jsonc"),
            r#"{ "backup": { "enabled": false } }"#,
        )
        .unwrap();
        let req = AfterDeploy {
            fingerprint: "fp-1",
            target: "vercel",
            production: false,
            url: Some("https://example.com"),
            build_env_names: vec![],
            interactive: Interactive::No,
            json: true,
        };
        assert_eq!(run_after_deploy_failed_fixture(dir.path(), &req), None);

        std::fs::remove_file(dir.path().join("appz.jsonc")).unwrap();
        let summary = run_after_deploy_failed_fixture(dir.path(), &req).unwrap();
        assert_eq!(summary.snapshot, None);
        assert!(
            summary
                .skipped
                .as_deref()
                .is_some_and(|s| s.contains("boom"))
        );
    }
    #[test]
    fn fingerprint_tracks_bytes_with_identical_size_and_time_across_fresh_trees() {
        fn fingerprint(bytes: &[u8]) -> Vec<u8> {
            let tree = tempfile::tempdir().unwrap();
            let path = tree.path().join("file");
            std::fs::write(&path, bytes).unwrap();
            filetime::set_file_mtime(&path, std::time::UNIX_EPOCH.into()).unwrap();
            let mut digest = Sha256::new();
            assert_eq!(
                fingerprint_tree(tree.path(), tree.path(), &mut digest).unwrap(),
                1
            );
            digest.finalize().to_vec()
        }
        assert_eq!(fingerprint(b"AAA"), fingerprint(b"AAA"));
        assert_ne!(fingerprint(b"AAA"), fingerprint(b"BBB"));
    }

    #[test]
    fn staging_rejects_parent_components_and_directory_links() {
        let project = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        let source = Dir::open_ambient_dir(project.path(), cap_std::ambient_authority()).unwrap();
        let target = stage.path().join("copy");
        assert!(stage_file(&source, Path::new("../source"), &target).is_err());

        std::fs::create_dir(project.path().join("src")).unwrap();
        std::fs::write(project.path().join("src/file"), b"contents").unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink("src", project.path().join("link"));
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir("src", project.path().join("link"));
        match linked {
            Ok(()) => {
                assert!(stage_file(&source, Path::new("link/file"), &target).is_err());
                assert!(!target.exists());
            }
            #[cfg(windows)]
            Err(e) if e.raw_os_error() == Some(1314) => {}
            Err(e) => panic!("creating fixture directory link: {e}"),
        }
        stage_file(&source, Path::new("src/file"), &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"contents");
    }

    #[cfg(windows)]
    #[test]
    fn drive_relative_repository_requires_an_absolute_path() {
        let project = tempfile::tempdir().unwrap();
        let root = appz_core::canonicalize(project.path()).unwrap();
        for repo in ["C:backup", "C:", "local:C:backup"] {
            let err = repository_location(&root, repo).unwrap_err();
            assert!(err.to_string().contains("drive-relative"), "{repo}: {err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn colon_in_local_repository_retains_local_backend() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let root = appz_core::canonicalize(&project).unwrap();
        let repo = root.parent().unwrap().join("backup:archive");
        for arg in [
            "local:../backup:archive",
            "../backup:archive",
            repo.to_str().unwrap(),
        ] {
            assert_eq!(
                repository_location(&root, arg).unwrap(),
                format!("local:{}", repo.display())
            );
        }
        assert!(repository_location(&root, "local:./backup:archive").is_err());
    }

    #[test]
    fn ls_marks_local_repo_as_not_off_machine() {
        let dir = tempfile::tempdir().unwrap();
        struct EmptyRepoEngine;
        impl Rustic for EmptyRepoEngine {
            fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
                match args {
                    ["repoinfo", "--json"] => Ok(RunOut {
                        stdout: r#"{"files":{"repo":[{"size":1024}]}}"#.into(),
                        stderr: String::new(),
                    }),
                    ["snapshots", "--json"] => Ok(RunOut {
                        stdout: "[]".into(),
                        stderr: String::new(),
                    }),
                    other => panic!("unexpected rustic call: {other:?}"),
                }
            }
        }

        let local_default = list(
            &EmptyRepoEngine,
            &config::BackupConfig::default(),
            dir.path(),
        )
        .unwrap();
        assert!(!local_default.off_machine);
        assert!(
            local_default
                .repository
                .contains(&config::project_id(dir.path()))
        );
        assert_eq!(local_default.size, 1024);
        assert_eq!(local_default.max_size, 2 * 1024 * 1024 * 1024);

        let local_explicit = list(
            &EmptyRepoEngine,
            &config::BackupConfig {
                repository: Some("local:/tmp/backups".into()),
                ..Default::default()
            },
            dir.path(),
        )
        .unwrap();
        assert!(!local_explicit.off_machine);

        let remote = list(
            &EmptyRepoEngine,
            &config::BackupConfig {
                repository: Some("rclone:remote:appz-backups".into()),
                ..Default::default()
            },
            dir.path(),
        )
        .unwrap();
        assert!(remote.off_machine);
    }

    #[test]
    fn ls_rows_come_from_tags() {
        let dir = tempfile::tempdir().unwrap();
        let host = format!("appz-{}", config::project_id(dir.path()));
        let snapshots_json = format!(
            r#"[{{
                "group_key":{{"hostname":"{host}"}},
                "snapshots":[
                    {{
                        "id":"snap-old",
                        "time":"2026-11-19T10:00:00Z",
                        "tags":["appz","target:vercel","env:preview","commit:abc1234","fp:1"],
                        "summary":{{"data_added":512}}
                    }},
                    {{
                        "id":"snap-new",
                        "time":"2026-11-20T12:00:00Z",
                        "tags":["appz","target:netlify","env:production","commit:def5678","url:https://prod.example.com","fp:2"],
                        "summary":{{"data_added":2048}}
                    }}
                ]
            }},{{
                "group_key":{{"hostname":"appz-otherproject00"}},
                "snapshots":[{{"id":"other-host","time":"2026-11-21T00:00:00Z","tags":["appz"]}}]
            }}]"#
        );

        struct TaggedEngine(String);
        impl Rustic for TaggedEngine {
            fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
                match args {
                    ["repoinfo", "--json"] => Ok(RunOut {
                        stdout: r#"{"files":{"repo":[{"size":4096}]}}"#.into(),
                        stderr: String::new(),
                    }),
                    ["snapshots", "--json"] => Ok(RunOut {
                        stdout: self.0.clone(),
                        stderr: String::new(),
                    }),
                    other => panic!("unexpected rustic call: {other:?}"),
                }
            }
        }

        let out = list(
            &TaggedEngine(snapshots_json),
            &config::BackupConfig::default(),
            dir.path(),
        )
        .unwrap();
        assert_eq!(out.size, 4096);
        assert_eq!(out.snapshots.len(), 2);
        assert_eq!(
            out.snapshots[0],
            SnapshotRow {
                id: "snap-new".into(),
                time: "2026-11-20T12:00:00Z".into(),
                target: Some("netlify".into()),
                environment: Some("production".into()),
                commit: Some("def5678".into()),
                url: Some("https://prod.example.com".into()),
                added_bytes: 2048,
            }
        );
        assert_eq!(
            out.snapshots[1],
            SnapshotRow {
                id: "snap-old".into(),
                time: "2026-11-19T10:00:00Z".into(),
                target: Some("vercel".into()),
                environment: Some("preview".into()),
                commit: Some("abc1234".into()),
                url: None,
                added_bytes: 512,
            }
        );
    }

    #[test]
    fn manual_backup_reports_no_changes_when_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<h1>hello</h1>").unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        let cfg = config::BackupConfig {
            repository: Some(repo_dir.path().to_string_lossy().into_owned()),
            ..Default::default()
        };
        let host = format!("appz-{}", config::project_id(dir.path()));

        struct StatefulEngine {
            host: String,
            tags: std::cell::RefCell<Vec<String>>,
            calls: std::cell::RefCell<Vec<Vec<String>>>,
        }

        impl Rustic for StatefulEngine {
            fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
                self.calls
                    .borrow_mut()
                    .push(args.iter().map(|s| (*s).to_string()).collect());
                match args {
                    ["snapshots", "--json"] => {
                        let stdout = match self.tags.borrow().as_slice() {
                            [.., _] => format!(
                                r#"[{{"group_key":{{"hostname":"{}"}},"snapshots":[{{"id":"snap-1","time":"2026-11-20T00:00:00Z","tags":{},"summary":{{"total_files_processed":2}}}}]}}]"#,
                                self.host,
                                serde_json::to_string(&*self.tags.borrow()).unwrap()
                            ),
                            [] => "[]".to_string(),
                        };
                        Ok(RunOut {
                            stdout,
                            stderr: String::new(),
                        })
                    }
                    ["forget", ..] => Ok(RunOut {
                        stdout: r#"[{"items":[]}]"#.into(),
                        stderr: String::new(),
                    }),
                    ["prune"] => Ok(RunOut {
                        stdout: String::new(),
                        stderr: String::new(),
                    }),
                    ["repoinfo", "--json"] => Ok(RunOut {
                        stdout: r#"{"files":{"repo":[{"size":128}]}}"#.into(),
                        stderr: String::new(),
                    }),
                    _ if args.first() == Some(&"backup") => {
                        *self.tags.borrow_mut() = args
                            .windows(2)
                            .filter(|pair| pair[0] == "--tag")
                            .map(|pair| pair[1].to_owned())
                            .collect();
                        Ok(RunOut {
                            stdout: r#"{"id":"snap-1","summary":{"data_added":256,"total_files_processed":2}}"#.into(),
                            stderr: String::new(),
                        })
                    }
                    other => panic!("unexpected rustic call: {other:?}"),
                }
            }
        }

        let engine = StatefulEngine {
            host,
            tags: Default::default(),
            calls: std::cell::RefCell::new(Vec::new()),
        };

        let (first, retention_first) = manual_create(&engine, &cfg, dir.path(), true).unwrap();
        assert_eq!(first.snapshot.as_deref(), Some("snap-1"));
        assert!(retention_first.is_some_and(|r| r.ran));

        let calls_after_first = engine.calls.borrow().len();
        let (second, retention_second) = manual_create(&engine, &cfg, dir.path(), true).unwrap();
        assert_eq!(second.snapshot, None);
        assert_eq!(second.skipped.as_deref(), Some("no changes"));
        assert!(retention_second.is_none());
        assert_eq!(engine.calls.borrow().len(), calls_after_first + 1);
    }
}
