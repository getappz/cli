mod publish;
use std::fs;
use std::path::{Component, Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};

use super::engine::Rustic;
use super::manifest::{MANIFEST_NAME, Manifest};
use super::{Interactive, Result, config, path_arg, select};

#[derive(Args)]
pub struct RestoreArgs {
    /// Snapshot ID or prefix. "latest" selects the newest snapshot, which must have an appz manifest.
    id: String,
    /// Empty directory to restore into
    #[arg(long, required_unless_present = "force", conflicts_with = "force")]
    to: Option<PathBuf>,
    /// Restore in the current project after verifying it is clean
    #[arg(long)]
    force: bool,
}

pub enum RestoreDest {
    NewDir(PathBuf),
    InPlace,
}

#[derive(Debug, Serialize)]
pub struct RestoreReport {
    dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_dir: Option<PathBuf>,
    fingerprint_ok: bool,
    missing_env: Vec<String>,
}

pub fn run(args: RestoreArgs, json: bool) -> Result<()> {
    let root = std::env::current_dir()?;
    let mut cfg = config::read_config(&root)?;
    cfg.enabled = Some(true);
    if let Some(repo) = super::engine::env_repository() {
        cfg.repository = Some(path_arg(&repo)?.to_owned());
    }
    let engine = super::open_repo(&root, &cfg, Interactive::No)?
        .ok_or("could not open the backup repository")?;
    let dest = args.to.map_or(RestoreDest::InPlace, RestoreDest::NewDir);
    let report = restore(&engine, &root, &args.id, dest)?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        crate::success(&format!("Restored to {}", report.dir.display()));
        if let Some(path) = &report.recovery_dir {
            crate::ui::hint(&format!(
                "Pre-restore files and index saved in {}",
                path.display()
            ));
        }
        if !report.fingerprint_ok {
            crate::warning("Restored source fingerprint differs from the snapshot");
        }
        for name in &report.missing_env {
            crate::warning(&format!(
                "Missing environment variable {name}. Run appz env set {name}"
            ));
        }
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = select::git_command()
        .current_dir(dir)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?.trim_end().to_owned())
}

fn object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || !path.components().all(|c| match c {
            Component::Normal(s) => {
                let name = s.to_string_lossy().to_ascii_lowercase();
                !name.contains([':', '\\'])
                    && !matches!(name.trim_end_matches([' ', '.']), ".git" | "git~1")
                    && !name.ends_with([' ', '.'])
            }
            _ => false,
        })
    {
        return Err(format!("invalid snapshot path {}", path.display()).into());
    }
    Ok(())
}

fn ensure_parents(root: &Path, rel: &Path) -> Result<()> {
    relative(rel)?;
    let mut path = root.to_owned();
    for part in rel.parent().unwrap().components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => return Err(format!("unsafe destination directory {}", path.display()).into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)?,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn checkout(archive: &Path, root: &Path, dir: &Path, m: &Manifest) -> Result<()> {
    let Some(info) = &m.git else { return Ok(()) };
    if !info.sha.is_empty() && !object_id(&info.sha) {
        return Err("invalid commit in manifest".into());
    }
    git(dir, &["init"])?;
    if let Some(remote) = &info.remote {
        match git(root, &["remote", "get-url", "--", remote]) {
            Ok(url) => {
                git(dir, &["remote", "add", "--", remote, &url])?;
            }
            Err(_) if m.has_bundle => {} // A self-contained bundle needs no remote.
            Err(e) => return Err(e),
        }
    }
    let fetch_remote = || -> Result<()> {
        let remote = info
            .remote
            .as_deref()
            .ok_or("snapshot requires a Git remote")?;
        git(dir, &["fetch", "--no-tags", "--", remote])?;
        Ok(())
    };
    if m.has_bundle {
        let bundle = archive.join("git/local-commits.bundle");
        if !fs::symlink_metadata(bundle.parent().unwrap())?.is_dir()
            || !fs::symlink_metadata(&bundle)?.is_file()
        {
            return Err("invalid Git bundle in snapshot".into());
        }
        if git(dir, &["bundle", "verify", path_arg(&bundle)?]).is_err() {
            fetch_remote()?;
        }
        git(
            dir,
            &["fetch", "--no-tags", "--", path_arg(&bundle)?, "HEAD"],
        )?;
    } else if !info.sha.is_empty() && git(dir, &["cat-file", "-e", &info.sha]).is_err() {
        fetch_remote()?;
    }
    if !info.sha.is_empty() {
        git(
            dir,
            &["checkout", "--no-overwrite-ignore", "--detach", &info.sha],
        )?;
        if let Some(branch) = &info.branch {
            git(dir, &["check-ref-format", "--branch", branch])?;
            git(dir, &["switch", "-C", branch, &info.sha])?;
        }
    }
    Ok(())
}

fn install_index(
    base: &Path,
    node: &Path,
    dir: &Path,
    link: bool,
    manifest: &Manifest,
) -> Result<()> {
    for entry in fs::read_dir(node)? {
        let e = entry?;
        let path = e.path();
        let kind = e.file_type()?;
        if kind.is_dir() {
            install_index(base, &path, dir, link, manifest)?;
            continue;
        }
        if !kind.is_file() {
            return Err("invalid archived index entry".into());
        }
        let mut parts = path.strip_prefix(base)?.components();
        let id = path_arg(Path::new(
            parts.next().ok_or("missing object ID")?.as_os_str(),
        ))?;
        let rel = parts.as_path();
        relative(rel)?;
        if !object_id(id) {
            return Err("invalid staged object ID".into());
        }
        let mode = if link {
            "160000".to_owned()
        } else {
            if git(
                dir,
                &["hash-object", "-w", "--no-filters", "--", path_arg(&path)?],
            )? != id
            {
                return Err("staged object does not match its archived ID".into());
            }
            let key = path_arg(rel)?.replace('\\', "/");
            if let Some(mode) = manifest.staged_modes.get(&key) {
                if !matches!(mode.as_str(), "100644" | "100755" | "120000") {
                    return Err("invalid archived index mode".into());
                }
                mode.clone()
            } else {
                let tracked = git(
                    dir,
                    &[
                        "--literal-pathspecs",
                        "ls-files",
                        "--stage",
                        "--",
                        path_arg(rel)?,
                    ],
                )?;
                tracked
                    .split_whitespace()
                    .next()
                    .unwrap_or("100644")
                    .to_owned()
            }
        };
        git(
            dir,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &mode,
                id,
                path_arg(rel)?,
            ],
        )?;
    }
    Ok(())
}

/// Remove only the leaf, without following a restored link in a parent directory.
fn remove_archived_file(dir: &Path, rel: &Path) -> Result<()> {
    use cap_fs_ext::DirExt;
    relative(rel)?;
    let mut parent = cap_std::fs::Dir::open_ambient_dir(dir, cap_std::ambient_authority())?;
    for component in rel.parent().unwrap().components() {
        match parent.open_dir_nofollow(component.as_os_str()) {
            Ok(next) => parent = next,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    let leaf = rel.file_name().unwrap();
    let removed = match parent.symlink_metadata(leaf) {
        Ok(meta) if meta.is_dir() => parent.remove_dir(leaf),
        Ok(_) => parent.remove_file(leaf),
        Err(e) => Err(e),
    };
    match removed {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn project_dir(checkout: &Path, m: &Manifest) -> Result<PathBuf> {
    let prefix = m.git.as_ref().map_or("", |git| git.prefix.as_str());
    if prefix.is_empty() {
        return Ok(checkout.to_owned());
    }
    let prefix = Path::new(prefix);
    relative(prefix)?;
    // Check every ancestor including the final project directory.
    ensure_parents(checkout, &prefix.join(MANIFEST_NAME))?;
    Ok(checkout.join(prefix))
}

fn overlay(archive: &Path, dir: &Path, m: &Manifest) -> Result<()> {
    if m.git.is_none() && (!m.deleted.index.is_empty() || !m.deleted.worktree.is_empty()) {
        return Err("unexpected Git deletions in a non-Git snapshot".into());
    }
    for name in &m.deleted.index {
        relative(Path::new(name))?;
        git(dir, &["update-index", "--force-remove", "--", name])?;
    }
    for name in &m.deleted.worktree {
        remove_archived_file(dir, Path::new(name))?;
    }
    if m.git.is_some() && archive.join("git").try_exists()? {
        if !fs::symlink_metadata(archive.join("git"))?.is_dir() {
            return Err("invalid git archive".into());
        }
        for (name, link) in [("git/index", false), ("git/gitlinks", true)] {
            let base = archive.join(name);
            if base.try_exists()? {
                if !fs::symlink_metadata(&base)?.is_dir() {
                    return Err("invalid index directory".into());
                }
                install_index(&base, &base, dir, link, m)?;
            }
        }
    }
    if m.files.is_empty() {
        return Ok(());
    }
    let source = archive.join("worktree");
    if !fs::symlink_metadata(&source)?.is_dir() {
        return Err("invalid worktree archive".into());
    }
    let source = cap_std::fs::Dir::open_ambient_dir(source, cap_std::ambient_authority())?;
    for name in &m.files {
        let rel = Path::new(name);
        ensure_parents(dir, rel)?;
        let target = dir.join(rel);
        let temp = tempfile::tempdir_in(target.parent().unwrap())?;
        let replacement = temp.path().join("file");
        super::stage_file(&source, rel, &replacement)?;
        if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir()) {
            fs::remove_dir(&target)?;
        }
        fs::rename(replacement, target)?;
    }
    Ok(())
}

fn fingerprint(dir: &Path, m: &Manifest, restored: bool) -> Result<String> {
    let outputs: Vec<_> = appz_core::detect_toolchains(dir)?
        .into_iter()
        .filter_map(|t| t.output_directory)
        .collect();
    let mut selection = select::select(dir, &outputs)?;
    if restored {
        // Secret values are deliberately absent from a restored worktree.
        selection.env_names.clone_from(&m.env_names);
    }
    let result = if m.source_fingerprint.is_some() {
        super::snapshot::source_fingerprint(dir, &selection, &m.build_env_names)
    } else {
        // Legacy tags predate index mode recording.
        for entry in &mut selection.staged {
            entry.mode.clear();
        }
        select::selection_fingerprint(dir, &selection, &m.build_env_names)
    };
    if let Some(bundle) = selection.bundle {
        select::remove_bundle(&bundle);
    }
    Ok(result?)
}

fn empty(dir: &Path) -> Result<()> {
    match fs::symlink_metadata(dir) {
        Ok(m) if m.is_dir() && fs::read_dir(dir)?.next().is_none() => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "restore destination must be an empty directory: {}",
            dir.display()
        )
        .into()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
struct Snapshot {
    id: String,
    tags: Vec<String>,
}

pub fn restore(
    engine: &dyn Rustic,
    root: &Path,
    id: &str,
    dest: RestoreDest,
) -> Result<RestoreReport> {
    let in_place = matches!(dest, RestoreDest::InPlace);
    let dir = match dest {
        RestoreDest::NewDir(p) => {
            let dir = root.join(p);
            empty(&dir)?;
            dir
        }
        RestoreDest::InPlace => root.to_owned(),
    };
    let meta = engine.run(&["snapshots", "--json", "--", id])?;
    #[derive(Deserialize)]
    struct Group {
        snapshots: Vec<Snapshot>,
    }
    let groups: Vec<Group> = serde_json::from_str(&meta.stdout)?;
    let mut matches = groups.into_iter().flat_map(|g| g.snapshots);
    let snapshot = matches.next().ok_or("snapshot not found")?;
    if matches.next().is_some() {
        return Err("ambiguous snapshot ID".into());
    }
    let archive = super::private_tempdir()?;
    engine.run(&["restore", "--", &snapshot.id, path_arg(archive.path())?])?;
    let metadata = archive.path().join(MANIFEST_NAME);
    if !fs::symlink_metadata(&metadata).is_ok_and(|m| m.is_file()) {
        return Err(
            "snapshot lacks an appz manifest; use appz backup restore for raw extraction".into(),
        );
    }
    let m: Manifest = serde_json::from_slice(&fs::read(metadata)?)?;
    let expected = snapshot
        .tags
        .iter()
        .find_map(|t| t.strip_prefix("fp:"))
        .ok_or("missing snapshot fingerprint")?;
    if expected != m.fingerprint {
        return Err("snapshot tag and manifest fingerprints differ".into());
    }

    let expected = m.source_fingerprint.as_deref().unwrap_or(expected);
    if in_place && m.git.is_some() {
        if !root.join(".git").try_exists()?
            || !git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty()
        {
            return Err("in-place restore requires a clean Git project root".into());
        }
        if m.git.as_ref().is_some_and(|git| !git.prefix.is_empty()) {
            return Err("restore a nested Git project using --to".into());
        }
        for name in m.files.iter().chain(&m.deleted.worktree) {
            relative(Path::new(name))?;
            if fs::symlink_metadata(root.join(name)).is_ok()
                && git(root, &["--literal-pathspecs", "ls-files", "--", name])?.is_empty()
            {
                return Err(
                    format!("in-place restore would overwrite untracked file {name}").into(),
                );
            }
        }
    } else if in_place && fingerprint(root, &m, false)? != expected {
        return Err("in-place restore requires an unchanged project fingerprint".into());
    }
    let mut recovery_dir = None;
    let fingerprint_ok = if in_place && m.git.is_none() {
        true // The current tree already matches the requested snapshot.
    } else if in_place {
        // Exercise the checkout and validate every archived object before touching the live index.
        let initial = publish::head(root)?;
        let rehearsal = super::private_tempdir()?;
        checkout(archive.path(), root, rehearsal.path(), &m)?;
        overlay(archive.path(), rehearsal.path(), &m)?;
        if fingerprint(rehearsal.path(), &m, true)? != expected {
            return Err("snapshot verification failed before in-place restore".into());
        }
        let recovery = publish::apply(root, rehearsal.path(), &m, &initial)?;
        let verified = fingerprint(&dir, &m, true)
            .map_err(|e| format!("{e}; recovery files: {}", recovery.display()))?
            == expected;
        recovery_dir = Some(recovery);
        verified
    } else {
        let parent = dir
            .parent()
            .ok_or("destination requires a parent directory")?;
        fs::create_dir_all(parent)?;
        let stage = super::private_tempdir_in(parent)?;
        checkout(archive.path(), root, stage.path(), &m)?;
        let project = project_dir(stage.path(), &m)?;
        overlay(archive.path(), &project, &m)?;
        let verified = fingerprint(&project, &m, true)? == expected;
        empty(&dir)?;
        #[cfg(unix)]
        let destination_access = fs::metadata(&dir).ok().map(|meta| meta.permissions());
        if dir.try_exists()? {
            fs::remove_dir(&dir)?;
        }
        fs::rename(stage.path(), &dir)?;
        #[cfg(unix)]
        if let Some(perms) = destination_access {
            fs::set_permissions(&dir, perms)?;
        }
        verified
    };
    let dir = project_dir(&dir, &m)?;
    let known = crate::secrets::known_names(&dir);
    let mut missing_env: Vec<_> = m
        .env_names
        .into_iter()
        .chain(m.build_env_names)
        .filter(|key| !known.contains(key))
        .collect();
    missing_env.sort();
    missing_env.dedup();
    Ok(RestoreReport {
        dir,
        recovery_dir,
        fingerprint_ok,
        missing_env,
    })
}

#[cfg(test)]
mod tests {
    use super::super::engine::{EngineError, RunOut, RusticCli};
    use super::*;
    use clap::Parser;

    struct Fixture {
        manifest: Manifest,
    }

    impl Rustic for Fixture {
        fn run(&self, args: &[&str]) -> std::result::Result<RunOut, EngineError> {
            let stdout = match args[0] {
                "snapshots" => serde_json::json!([{"snapshots":[{"id":"abc","tags":[format!("fp:{}",self.manifest.fingerprint)]}]}]).to_string(),
                "restore" => {
                    let archive = Path::new(args[3]);
                    fs::write(archive.join(MANIFEST_NAME), serde_json::to_vec(&self.manifest).unwrap()).unwrap();
                    fs::create_dir(archive.join("worktree")).unwrap();
                    fs::write(archive.join("worktree/hello"), "world").unwrap();
                    String::new()
                }
                other => panic!("unexpected rustic command {other}"),
            };
            Ok(RunOut {
                stdout,
                stderr: String::new(),
            })
        }
    }

    fn fixture() -> Fixture {
        Fixture {
            manifest: Manifest {
                appz_version: String::new(),
                created_at: String::new(),
                target: None,
                environment: None,
                url: None,
                fingerprint: "sample".into(),
                git: None,
                tools: Default::default(),
                env_names: vec!["APPZ_RESTORE_FIXTURE".into()],
                build_env_names: vec![],
                files: vec!["hello".into()],
                has_bundle: false,
                deleted: Default::default(),
                staged_modes: Default::default(),
                source_fingerprint: None,
            },
        }
    }

    #[test]
    fn refuses_non_empty_destination() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("existing"), "keep").unwrap();
        assert!(
            restore(
                &fixture(),
                root.path(),
                "abc",
                RestoreDest::NewDir(root.path().into())
            )
            .unwrap_err()
            .to_string()
            .contains("empty directory")
        );
        assert_eq!(fs::read(root.path().join("existing")).unwrap(), b"keep");
    }

    #[test]
    fn in_place_requires_force_and_clean_tree() {
        assert!(crate::AppzCli::try_parse_from(["appz", "restore", "abc"]).is_err());
        assert!(crate::AppzCli::try_parse_from(["appz", "restore", "abc", "--force"]).is_ok());
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("hello"), "keep").unwrap();
        assert!(restore(&fixture(), root.path(), "abc", RestoreDest::InPlace).is_err());
        assert_eq!(fs::read(root.path().join("hello")).unwrap(), b"keep");
    }

    #[test]
    fn reports_env_names_missing_from_vault() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("hello"), "world").unwrap();
        let mut snapshot = fixture();
        snapshot.manifest.fingerprint = fingerprint(root.path(), &snapshot.manifest, true).unwrap();
        let dest = tempfile::tempdir().unwrap();
        let report = restore(
            &snapshot,
            root.path(),
            "abc",
            RestoreDest::NewDir(dest.path().join("restored")),
        )
        .unwrap();
        assert_eq!(report.missing_env, ["APPZ_RESTORE_FIXTURE"]);
        assert_eq!(fs::read(report.dir.join("hello")).unwrap(), b"world");
        assert!(report.fingerprint_ok);
    }

    #[test]
    fn removes_only_empty_gitlink_directories() {
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("submodule");
        fs::create_dir(&link).unwrap();
        fs::write(link.join("local"), "keep").unwrap();
        assert!(remove_archived_file(root.path(), Path::new("submodule")).is_err());
        fs::remove_file(link.join("local")).unwrap();
        remove_archived_file(root.path(), Path::new("submodule")).unwrap();
        assert!(!link.exists());
    }

    #[test]
    fn publish_rechecks_edits_and_keeps_originals_on_failure() {
        let sandbox = tempfile::tempdir().unwrap();
        let live = sandbox.path().join("live");
        let prepared = sandbox.path().join("prepared");
        fs::create_dir(&live).unwrap();
        git(&live, &["init"]).unwrap();
        fs::write(live.join("tracked"), "original").unwrap();
        fs::write(live.join(".gitignore"), "local/\n").unwrap();
        git(&live, &["add", "."]).unwrap();
        git(
            &live,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "base",
            ],
        )
        .unwrap();
        git(
            &live,
            &[
                "clone",
                "--",
                path_arg(&live).unwrap(),
                path_arg(&prepared).unwrap(),
            ],
        )
        .unwrap();
        let before = publish::head(&live).unwrap();
        let mut manifest = fixture().manifest;
        manifest.git = select::select(&prepared, &[]).unwrap().git;
        manifest.files = vec!["tracked".into()];
        fs::write(live.join("tracked"), "editor save").unwrap();
        assert!(
            publish::apply(&live, &prepared, &manifest, &before)
                .unwrap_err()
                .to_string()
                .contains("changed while preparing")
        );
        assert_eq!(fs::read(live.join("tracked")).unwrap(), b"editor save");

        fs::write(live.join("tracked"), "original").unwrap();
        fs::create_dir(live.join("local")).unwrap();
        fs::write(live.join("local/user-file"), "ignored").unwrap();
        manifest.deleted.worktree.insert("local".into());
        let error = publish::apply(&live, &prepared, &manifest, &before)
            .unwrap_err()
            .to_string();
        let (_, path) = error
            .split_once("recovery files: ")
            .unwrap_or_else(|| panic!("{error}"));
        assert_eq!(
            fs::read(Path::new(path).join("files/tracked")).unwrap(),
            b"original"
        );
        assert!(Path::new(path).join("original-index").is_file());
        assert_eq!(fs::read(live.join("local/user-file")).unwrap(), b"ignored");
        assert_eq!(git(&live, &["show", ":tracked"]).unwrap(), "original");
        assert!(!live.join(".git/index.lock").exists());
    }

    #[cfg(unix)]
    #[test]
    fn directory_can_be_replaced_by_a_symlink_after_its_tracked_file_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        let archive = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("pkg")).unwrap();
        fs::write(root.path().join("pkg/old"), "old").unwrap();
        fs::create_dir(archive.path().join("worktree")).unwrap();
        std::os::unix::fs::symlink("target", archive.path().join("worktree/pkg")).unwrap();
        let mut m = fixture().manifest;
        git(root.path(), &["init"]).unwrap();
        m.git = select::select(root.path(), &[]).unwrap().git;
        m.files = vec!["pkg".into()];
        m.deleted.worktree.insert("pkg/old".into());
        overlay(archive.path(), root.path(), &m).unwrap();
        assert_eq!(
            fs::read_link(root.path().join("pkg")).unwrap(),
            Path::new("target")
        );
    }

    #[cfg(unix)]
    #[test]
    fn destination_access_survives_publication() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let dst = root.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o710)).unwrap();
        let stage = super::super::private_tempdir_in(root.path()).unwrap();
        assert_eq!(
            fs::metadata(stage.path()).unwrap().permissions().mode() & 0o077,
            0
        );
        restore(
            &fixture(),
            root.path(),
            "abc",
            RestoreDest::NewDir(dst.clone()),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(dst).unwrap().permissions().mode() & 0o777,
            0o710
        );
    }

    #[test]
    #[ignore = "requires rustic and Git"]
    fn round_trip_restores_same_fingerprint() {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        fs::create_dir(&project).unwrap();
        let root = project.as_path();
        git(root, &["init"]).unwrap();
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "--bare"]).unwrap();
        git(
            root,
            &["remote", "add", "origin", path_arg(remote.path()).unwrap()],
        )
        .unwrap();
        fs::write(root.join("hello"), "committed").unwrap();
        for name in ["staged-delete", "unstaged-delete", "cached-delete"] {
            fs::write(root.join(name), name).unwrap();
        }
        git(root, &["add", "."]).unwrap();
        git(
            root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "base",
            ],
        )
        .unwrap();
        git(root, &["rm", "--", "staged-delete"]).unwrap();
        fs::remove_file(root.join("unstaged-delete")).unwrap();
        git(root, &["rm", "--cached", "--", "cached-delete"]).unwrap();
        fs::write(root.join("hello"), "staged").unwrap();
        git(root, &["add", "hello"]).unwrap();
        git(root, &["update-index", "--chmod=+x", "--", "hello"]).unwrap();
        fs::write(root.join("hello"), "working").unwrap();
        let cli = RusticCli::new(
            super::super::engine::find_rustic(root).unwrap(),
            repo.path(),
            super::super::repo::generate_recovery_password(),
            root,
        );
        cli.run(&["init"]).unwrap();
        let snapshot = super::super::snapshot::create_quiet_deduped(
            &cli,
            super::super::CreateInput {
                root,
                fingerprint: "",
                target: None,
                production: None,
                url: None,
                build_env_names: vec![],
            },
            true,
        )
        .unwrap()
        .snapshot
        .unwrap();
        // The archived history was local at backup time; remote reachability can change later.
        git(root, &["push", "origin", "HEAD"]).unwrap();
        git(
            root,
            &[
                "remote",
                "set-url",
                "origin",
                path_arg(&dest.path().join("offline")).unwrap(),
            ],
        )
        .unwrap();
        let report = restore(
            &cli,
            root,
            &snapshot,
            RestoreDest::NewDir(dest.path().join("project")),
        )
        .unwrap();
        assert_eq!(
            git(&report.dir, &["status", "--porcelain"]).unwrap(),
            git(root, &["status", "--porcelain"]).unwrap()
        );
        assert!(!report.dir.join("staged-delete").exists());
        assert!(!report.dir.join("unstaged-delete").exists());
        assert!(report.dir.join("cached-delete").is_file());
        assert_eq!(fs::read(report.dir.join("hello")).unwrap(), b"working");
        assert_eq!(git(&report.dir, &["show", ":hello"]).unwrap(), "staged");
        assert!(
            git(&report.dir, &["ls-files", "--stage", "--", "hello"])
                .unwrap()
                .starts_with("100755 ")
        );
        assert!(report.fingerprint_ok);
        assert!(restore(&cli, root, &snapshot, RestoreDest::InPlace).is_err());
        assert_eq!(fs::read(root.join("hello")).unwrap(), b"working");
        git(root, &["restore", "--staged", "--worktree", "--", "."]).unwrap();
        fs::write(root.join("later.txt"), "later commit").unwrap();
        git(root, &["add", "later.txt"]).unwrap();
        git(
            root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "later",
            ],
        )
        .unwrap();
        let later = git(root, &["rev-parse", "HEAD"]).unwrap();
        let branch = git(root, &["symbolic-ref", "HEAD"]).unwrap();
        let report = restore(&cli, root, &snapshot, RestoreDest::InPlace).unwrap();
        assert_eq!(git(root, &["rev-parse", &branch]).unwrap(), later);
        assert!(!root.join("later.txt").exists());
        assert!(report.fingerprint_ok);
        let recovery = report.recovery_dir.unwrap();
        assert_eq!(
            fs::read(recovery.join("files/hello")).unwrap(),
            b"committed"
        );
        assert!(recovery.join("original-index").is_file());
        assert_eq!(git(root, &["show", ":hello"]).unwrap(), "staged");
        assert_eq!(fs::read(root.join("hello")).unwrap(), b"working");
    }
}
