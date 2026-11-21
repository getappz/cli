//! Publish a prepared restore without letting Git overwrite the live working tree.
use super::{Manifest, Result, ensure_parents, fs, git, object_id, path_arg, relative, select};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

#[derive(PartialEq, serde::Serialize)]
pub(super) struct Head {
    branch: Option<String>,
    commit: Option<String>,
}

pub(super) fn head(root: &Path) -> Result<Head> {
    let branch = git(root, &["symbolic-ref", "--quiet", "HEAD"]).ok();
    let commit = git(root, &["rev-parse", "--verify", "HEAD"]).ok();
    if commit.is_none() && branch.is_none() {
        return Err("cannot identify the original Git HEAD".into());
    }
    Ok(Head { branch, commit })
}

fn paths(root: &Path, args: &[&str]) -> Result<BTreeSet<PathBuf>> {
    let out = select::git_command()
        .current_dir(root)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string().into());
    }
    let mut result = BTreeSet::new();
    for bytes in out.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(std::str::from_utf8(bytes)?);
        relative(&path)?;
        result.insert(path);
    }
    Ok(result)
}

// Return None for an absent leaf or an ancestor that is itself a file.
// Never follow a symlink to inspect or move anything below it.
fn leaf(root: &Path, path: &Path) -> Result<Option<fs::FileType>> {
    relative(path)?;
    let mut parent = root.to_owned();
    let mut parts = path.components().peekable();
    while let Some(Component::Normal(part)) = parts.next() {
        parent.push(part);
        match fs::symlink_metadata(&parent) {
            Ok(meta) if parts.peek().is_none() => return Ok(Some(meta.file_type())),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(None)
}

pub(super) fn apply(root: &Path, prepared: &Path, m: &Manifest, before: &Head) -> Result<PathBuf> {
    let info = m.git.as_ref().ok_or("expected a Git snapshot")?;
    if info.sha.is_empty() && before.commit.is_some() {
        return Err("use --to to restore an unborn branch over a committed checkout".into());
    }
    let parent = root
        .parent()
        .ok_or("restore requires a project parent directory")?;
    let recovery = super::super::private_tempdir_in(parent)?;
    let saved = recovery.path().join("files");
    fs::create_dir(&saved)?;

    // A temporary commit transports staged-only blobs as well as the target commit.
    // Its ref lives exclusively in the disposable rehearsal repository.
    let tree = git(prepared, &["write-tree"])?;
    let mut args = vec![
        "-c",
        "user.name=appz",
        "-c",
        "user.email=restore@localhost",
        "-c",
        "commit.gpgSign=false",
        "commit-tree",
        &tree,
        "-m",
        "appz restore index",
    ];
    if !info.sha.is_empty() {
        args.extend(["-p", &info.sha]);
    }
    let index_commit = git(prepared, &args)?;
    git(
        prepared,
        &["update-ref", "refs/appz/restore-index", &index_commit],
    )?;
    git(
        root,
        &[
            "fetch",
            "--no-tags",
            "--no-write-fetch-head",
            "--",
            path_arg(prepared)?,
            "refs/appz/restore-index",
        ],
    )?;

    let tracked = paths(root, &["ls-files", "-z"])?;
    let mut affected = match (&before.commit, info.sha.as_str()) {
        (Some(old), new) if old != new => paths(
            root,
            &["diff", "--name-only", "--no-renames", "-z", old, new, "--"],
        )?,
        (None, new) if !new.is_empty() => {
            paths(root, &["ls-tree", "-r", "-z", "--name-only", new])?
        }
        _ => BTreeSet::new(),
    };
    affected.extend(m.files.iter().chain(&m.deleted.worktree).map(PathBuf::from));
    let mut moved = Vec::new();
    for path in &affected {
        if let Some(kind) = leaf(root, path)?
            && !kind.is_dir()
        {
            if !tracked.contains(path) {
                return Err(
                    format!("refusing to replace untracked file {}", path.display()).into(),
                );
            }
            moved.push(path.clone());
        }
    }

    let next = recovery.path().join("next-index");
    let out = select::git_command()
        .current_dir(root)
        .env("GIT_INDEX_FILE", &next)
        .args(["read-tree", &tree])
        .output()?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string().into());
    }
    let index = PathBuf::from(git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )?);
    let lock = index.with_extension("lock");
    // Git uses index.lock (also for linked worktrees). Hold it through file publication.
    let mut index_file = fs::File::create_new(&lock)?;
    let lock = tempfile::TempPath::try_from_path(lock)?;
    index_file.write_all(&fs::read(&next)?)?;
    index_file.sync_all()?;
    drop(index_file);
    if &head(root)? != before
        || !git(
            root,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=all",
            ],
        )?
        .is_empty()
    {
        return Err("the checkout changed while preparing the restore".into());
    }
    if index.try_exists()? {
        fs::copy(&index, recovery.path().join("original-index"))?;
    }
    fs::write(
        recovery.path().join("original-head.json"),
        serde_json::to_vec_pretty(before)?,
    )?;
    fs::write(
        recovery.path().join("affected.json"),
        serde_json::to_vec_pretty(&affected)?,
    )?;
    fs::write(
        recovery.path().join("RECOVERY.txt"),
        "The files directory holds original files moved from the project, including edits made during publication. affected.json lists the paths involved. Keep any subsequent edits before recovering. Restore the original branch or detached commit from original-head.json, restore original-index to the Git index if present, then copy the saved files back at their relative paths. Paths newly created by restore can be removed after inspection. An interrupted restore may have moved only some originals. Do not delete unrelated files.\n",
    )?;
    let path = recovery.keep();
    let publish = || -> Result<()> {
        for rel in &moved {
            ensure_parents(&saved, rel)?;
            fs::rename(root.join(rel), saved.join(rel))?;
        }
        // Verify the exact files just captured, not an earlier read of their names.
        for rel in &moved {
            git(
                root,
                &[
                    "--literal-pathspecs",
                    &format!("--work-tree={}", path_arg(&saved)?),
                    "diff",
                    "--quiet",
                    "--",
                    path_arg(rel)?,
                ],
            )?;
        }
        for rel in affected.iter().rev() {
            if leaf(root, rel)?.is_some_and(|t| t.is_dir())
                && !leaf(prepared, rel)?.is_some_and(|t| t.is_dir())
            {
                fs::remove_dir(root.join(rel))?;
            }
        }
        let source = cap_std::fs::Dir::open_ambient_dir(prepared, cap_std::ambient_authority())?;
        for rel in &affected {
            match leaf(prepared, rel)? {
                Some(kind) if kind.is_dir() => ensure_parents(root, &rel.join(".placeholder"))?,
                Some(_) => {
                    ensure_parents(root, rel)?;
                    super::super::stage_file(&source, rel, &root.join(rel))?;
                }
                None if leaf(root, rel)?.is_some() => {
                    return Err(format!("path appeared during restore: {}", rel.display()).into());
                }
                None => {}
            }
        }
        if &head(root)? != before {
            return Err("Git HEAD changed during restore".into());
        }
        if !info.sha.is_empty() && before.commit.as_deref() != Some(info.sha.as_str()) {
            if !object_id(&info.sha) {
                return Err("invalid snapshot commit".into());
            }
            let zero = "0".repeat(info.sha.len());
            git(
                root,
                &[
                    "update-ref",
                    "--no-deref",
                    "HEAD",
                    &info.sha,
                    before.commit.as_deref().unwrap_or(&zero),
                ],
            )?;
        }
        lock.persist(&index)?;
        Ok(())
    };
    publish().map_err(|err| {
        format!(
            "in-place restore stopped: {err}; recovery files: {}",
            path.display()
        )
    })?;
    Ok(path)
}
