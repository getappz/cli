//! Source backups through the `rustic` CLI.
//!
//! Only the source that cannot be rebuilt is snapshotted (see [`select`]);
//! environment values live in the vault, never in a snapshot.

pub mod engine;
pub mod retention;
pub mod select;

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use engine::{ENV_PASSWORD, ENV_REPOSITORY, Rustic, RusticCli};
use serde_json::json;

#[derive(Args)]
#[command(
    after_help = "Set APPZ_BACKUP_REPOSITORY to a dedicated Rustic repository outside the project. Set APPZ_BACKUP_PASSWORD or store it with `appz vault set APPZ_BACKUP_PASSWORD`."
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

pub fn run(args: BackupArgs, json_output: bool) -> Result<()> {
    let root = appz_core::canonicalize(&args.dir)?;
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()).into());
    }
    let repo = engine::env_repository()
        .ok_or_else(|| format!("set {ENV_REPOSITORY} to the backup repository"))?;
    let password = match engine::env_password()? {
        Some(value) => value,
        None => crate::secrets::deploy_env(&root, &[ENV_PASSWORD.to_string()])?
            .vars
            .remove(ENV_PASSWORD)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("set {ENV_PASSWORD} or unlock a vault containing it"))?,
    };
    let consent = if crate::assume_yes() {
        crate::deploy::Consent::Yes
    } else if !json_output && crate::is_tty() {
        crate::deploy::Consent::Ask
    } else {
        crate::deploy::Consent::Never
    };
    let bin = engine::ensure_installed(&root, consent)?;
    let cli = RusticCli::new(bin, repo, password, &root);
    match args.command.unwrap_or(BackupCmd::Create) {
        BackupCmd::Create => create(&cli, &root, json_output)?,
        BackupCmd::Init => {
            cli.run(&["init"])?;
            report(
                json_output,
                "Repository initialized",
                json!({"status":"initialized"}),
            );
        }
        BackupCmd::List => {
            let args: &[&str] = if json_output {
                &["snapshots", "--json"]
            } else {
                &["snapshots"]
            };
            print!("{}", cli.with_query_timeout().run(args)?.stdout);
        }
        BackupCmd::Restore {
            snapshot,
            destination,
        } => {
            cli.run(&["restore", "--", &snapshot, path_arg(&destination)?])?;
            report(
                json_output,
                "Snapshot restored",
                json!({"status":"restored","destination":destination}),
            );
        }
        BackupCmd::Prune => {
            let policy = retention_policy(&root)?;
            let result = retention::apply(&cli, &policy, root.join(".appz/backup.json"))?;
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

fn report(json_output: bool, message: &str, data: serde_json::Value) {
    if json_output {
        println!("{data}");
    } else {
        crate::success(message);
    }
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| format!("Rustic requires a UTF-8 path: {}", path.display()).into())
}

fn create(cli: &impl Rustic, root: &Path, json_output: bool) -> Result<()> {
    let outputs: Vec<_> = appz_core::detect_toolchains(root)?
        .iter()
        .filter_map(|t| t.output_directory)
        .collect();
    let selection = select::select(root, &outputs)?;
    // Rustic 0.11 can panic on multiple relative sources at the project root.
    let paths: Vec<_> = selection
        .files
        .iter()
        .map(|p| root.join(p))
        .chain(selection.bundle.iter().cloned())
        .collect();
    let snapshot = write_snapshot(cli, &paths, json_output);
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

fn write_snapshot(
    cli: &impl Rustic,
    paths: &[PathBuf],
    json_output: bool,
) -> Result<Option<serde_json::Value>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut args = vec!["backup", "--init", "--skip-if-unchanged", "--json", "--"];
    for path in paths {
        args.push(path_arg(path)?);
    }
    let out = cli.run(&args)?;
    if !json_output {
        eprint!("{}", out.stderr);
    }
    if out.stdout.trim().is_empty() {
        return Ok(None);
    }
    let snapshot: serde_json::Value = serde_json::from_str(&out.stdout)?;
    // With --skip-if-unchanged Rustic may emit the candidate snapshot without an ID.
    Ok(snapshot["id"].is_string().then_some(snapshot))
}

fn retention_policy(root: &Path) -> Result<retention::Retention> {
    let source = match std::fs::read_to_string(root.join("appz.jsonc")) {
        Ok(source) => source,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(e.into()),
    };
    let config: serde_json::Value = serde_json::from_str(&appz_core::strip_jsonc(&source))?;
    let policy: retention::Retention = match config.pointer("/backup/retention") {
        Some(policy) => serde_json::from_value(policy.clone())?,
        None => Default::default(),
    };
    if let Some(size) = &policy.max_size {
        retention::parse_size(size)?;
    }
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{EngineError, RunOut};

    #[derive(Default)]
    struct SnapshotFixture(std::cell::Cell<u32>);

    impl Rustic for SnapshotFixture {
        fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
            self.0.set(self.0.get() + 1);
            let (_, paths) = args.split_at(args.iter().position(|arg| *arg == "--").unwrap() + 1);
            assert_eq!(paths.len(), 1);
            assert!(Path::new(paths[0]).is_absolute());
            assert!(Path::new(paths[0]).ends_with("two words.txt"));
            Ok(RunOut {
                stdout: r#"{"id":"fixture"}"#.into(),
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
}
