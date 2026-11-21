use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use clap::Args;
use serde::{Deserialize, Serialize};

use super::engine::Rustic;
use super::manifest::{MANIFEST_NAME, Manifest};
use super::{Interactive, Result, config, path_arg, select};

#[derive(Args)]
pub struct RestoreArgs {
    /// Snapshot ID, prefix, or latest
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
        println!("Restored to {}", report.dir.display());
        if !report.fingerprint_ok {
            crate::warning("Restored source fingerprint differs from the snapshot");
        }
        for name in &report.missing_env {
            println!("Missing environment variable {name}. Run appz env set {name}");
        }
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
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

fn checkout(archive: &Path, root: &Path, dir: &Path, m: &Manifest, in_place: bool) -> Result<()> {
    let Some(info) = &m.git else { return Ok(()) };
    if !info.sha.is_empty() && !object_id(&info.sha) {
        return Err("invalid commit in manifest".into());
    }
    if !in_place {
        if let Some(remote) = &info.remote {
            let url = git(root, &["remote", "get-url", "--", remote])?;
            git(
                root,
                &[
                    "clone",
                    "--no-local",
                    "--no-checkout",
                    "--origin",
                    remote,
                    "--",
                    &url,
                    path_arg(dir)?,
                ],
            )?;
        } else {
            git(dir, &["init"])?;
        }
    }
    if m.has_bundle {
        let bundle = archive.join("git/local-commits.bundle");
        if !fs::symlink_metadata(bundle.parent().unwrap())?.is_dir()
            || !fs::symlink_metadata(&bundle)?.is_file()
        {
            return Err("invalid Git bundle in snapshot".into());
        }
        git(
            dir,
            &["fetch", "--no-tags", "--", path_arg(&bundle)?, "HEAD"],
        )?;
    }
    if !info.sha.is_empty()
        && !(in_place
            && git(dir, &["rev-parse", "--verify", "HEAD"]).ok().as_ref() == Some(&info.sha))
    {
        git(
            dir,
            &["checkout", "--no-overwrite-ignore", "--detach", &info.sha],
        )?;
        if !in_place && let Some(branch) = &info.branch {
            git(dir, &["check-ref-format", "--branch", branch])?;
            git(dir, &["switch", "-C", branch, &info.sha])?;
        }
    }
    Ok(())
}

fn install_index(base: &Path, node: &Path, dir: &Path, link: bool) -> Result<()> {
    for entry in fs::read_dir(node)? {
        let e = entry?;
        let path = e.path();
        let kind = e.file_type()?;
        if kind.is_dir() {
            install_index(base, &path, dir, link)?;
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

fn overlay(archive: &Path, dir: &Path, m: &Manifest) -> Result<()> {
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
                install_index(&base, &base, dir, link)?;
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
    let result = select::selection_fingerprint(dir, &selection, &m.build_env_names);
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

    if in_place && m.git.is_some() {
        if !root.join(".git").try_exists()?
            || !git(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty()
        {
            return Err("in-place restore requires a clean Git project root".into());
        }
        for name in &m.files {
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
    let fingerprint_ok = if in_place && m.git.is_none() {
        true // The current tree already matches the requested snapshot.
    } else if in_place {
        // Exercise the checkout and validate every archived object before touching the live index.
        let rehearsal = super::private_tempdir()?;
        checkout(archive.path(), root, rehearsal.path(), &m, false)?;
        overlay(archive.path(), rehearsal.path(), &m)?;
        checkout(archive.path(), root, &dir, &m, true)?;
        overlay(archive.path(), &dir, &m)?;
        fingerprint(&dir, &m, true)? == expected
    } else {
        let parent = dir
            .parent()
            .ok_or("destination requires a parent directory")?;
        fs::create_dir_all(parent)?;
        let stage = tempfile::tempdir_in(parent)?;
        checkout(archive.path(), root, stage.path(), &m, false)?;
        overlay(archive.path(), stage.path(), &m)?;
        let verified = fingerprint(stage.path(), &m, true)? == expected;
        empty(&dir)?;
        if dir.try_exists()? {
            fs::remove_dir(&dir)?;
        }
        fs::rename(stage.path(), &dir)?;
        verified
    };
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
    #[ignore = "requires rustic and Git"]
    fn round_trip_restores_same_fingerprint() {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let root = root.path();
        git(root, &["init"]).unwrap();
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "--bare"]).unwrap();
        git(
            root,
            &["remote", "add", "origin", path_arg(remote.path()).unwrap()],
        )
        .unwrap();
        fs::write(root.join("hello"), "committed").unwrap();
        git(root, &["add", "hello"]).unwrap();
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
        fs::write(root.join("hello"), "staged").unwrap();
        git(root, &["add", "hello"]).unwrap();
        fs::write(root.join("hello"), "working").unwrap();
        let cli = RusticCli::new(
            super::super::engine::find_rustic(root).unwrap(),
            repo.path(),
            "fixture",
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
        let report = restore(
            &cli,
            root,
            &snapshot,
            RestoreDest::NewDir(dest.path().join("project")),
        )
        .unwrap();
        assert_eq!(fs::read(report.dir.join("hello")).unwrap(), b"working");
        assert_eq!(git(&report.dir, &["show", ":hello"]).unwrap(), "staged");
        assert!(report.fingerprint_ok);
        assert!(restore(&cli, root, &snapshot, RestoreDest::InPlace).is_err());
        assert_eq!(fs::read(root.join("hello")).unwrap(), b"working");
        git(root, &["restore", "--staged", "--worktree", "--", "hello"]).unwrap();
        let report = restore(&cli, root, &snapshot, RestoreDest::InPlace).unwrap();
        assert!(report.fingerprint_ok);
        assert_eq!(git(root, &["show", ":hello"]).unwrap(), "staged");
        assert_eq!(fs::read(root.join("hello")).unwrap(), b"working");
    }
}
