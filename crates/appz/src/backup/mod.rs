//! Source backups through the `rustic` CLI.
//!
//! Only the source that cannot be rebuilt is snapshotted (see [`select`]);
//! working-tree environment files are excluded. Git bundles preserve committed history.

pub mod config;
pub mod engine;
pub mod manifest;
pub mod repo;
pub mod retention;
pub mod select;
pub mod snapshot;

#[allow(unused_imports)]
pub use repo::{Interactive, PASSWORD_ENV, open_repo, vault_key};
#[allow(unused_imports)]
pub use snapshot::{BackupSummary, CreateInput, create as create_snapshot};

use std::path::{Path, PathBuf};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::Dir;
use clap::{Args, Subcommand};
use engine::{ENV_PASSWORD, ENV_REPOSITORY, Rustic, RusticCli};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Args)]
#[command(
    after_help = "Set APPZ_BACKUP_REPOSITORY to a dedicated Rustic repository outside the project. Set APPZ_BACKUP_PASSWORD or store it with `appz vault set APPZ_BACKUP_PASSWORD` and run `appz vault unlock` before backup."
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
    let repo = repository_location(&root, path_arg(&repo)?)?;
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
    let cli = RusticCli::new(bin, &repo, password, &root);
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
            let result = retention::apply(&cli, &policy, &root, &repo)?;
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

pub(super) fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| format!("Rustic requires a UTF-8 path: {}", path.display()).into())
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
    let mut builder = tempfile::Builder::new();
    builder.prefix("appz-backup-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir()
}

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

fn retention_policy(root: &Path) -> Result<retention::Retention> {
    Ok(config::read_config(root)?.retention)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{EngineError, RunOut};

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
}
