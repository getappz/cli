//! Decide what is worth backing up: only source that cannot be rebuilt.
//!
//! - Never `.env*` (their values belong in the vault; only the variable
//!   NAMES are reported), build output, tool state or `*.log`.
//! - Not in a git repository: every remaining source file.
//! - In a git repository: files git reports as changed (modified, staged,
//!   untracked) or ignored-but-not-rebuildable, so a clean, pushed tree
//!   backs up nothing at all - matching "no empty snapshots".
//! - Commits that are on no remote are captured in a git bundle, so a
//!   restore can rebuild the history without a reachable remote.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::deploy_state;

#[derive(Debug, Clone)]
pub struct Selection {
    /// Source files (relative to `root`, sorted) that changed and need a
    /// snapshot.
    pub files: Vec<PathBuf>,
    /// Variable names found in `.env*` files, sorted and deduped. Values are
    /// never read into this struct.
    pub env_names: Vec<String>,
    /// `None` when the project is not a git repository.
    pub git: Option<GitInfo>,
    /// Temp file holding commits that exist only locally. Present only when
    /// there is something to carry.
    pub bundle: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub sha: String,
    pub branch: Option<String>,
    pub remote: Option<String>,
    /// Commits reachable from HEAD but from no remote ref.
    pub ahead: u32,
}

/// Select the source to snapshot. `output_dirs` are build outputs (from the
/// toolchain config) that must never be backed up even if present.
pub fn select(root: &Path, output_dirs: &[&str]) -> Result<Selection, String> {
    let all = deploy_state::source_files(root)?;
    let allowed: Vec<PathBuf> = all
        .iter()
        .filter(|rel| is_allowed(rel, output_dirs))
        .cloned()
        .collect();
    let allowed_set: BTreeSet<PathBuf> = allowed.iter().cloned().collect();
    let env_names = env_names(root, &all);

    let Some(state) = git_state(root)? else {
        return Ok(Selection {
            files: allowed,
            env_names,
            git: None,
            bundle: None,
        });
    };

    let mut changed: BTreeSet<PathBuf> = BTreeSet::new();

    // Tracked changes and untracked files. `-z` keeps paths verbatim, so
    // spaces and non-ASCII names survive unchanged.
    for rel in git_status(root)? {
        if allowed_set.contains(&rel) {
            changed.insert(rel);
        }
    }
    // Ignored files are still source when they are not build output - e.g. a
    // generated config or a local asset - and they never appear in `status`.
    for rel in ignored_files(root)? {
        if allowed_set.contains(&rel) {
            changed.insert(rel);
        }
    }
    let bundle = if state.ahead > 0 {
        Some(create_bundle(root)?)
    } else {
        None
    };

    Ok(Selection {
        files: changed.into_iter().collect(),
        env_names,
        git: Some(state),
        bundle,
    })
}

/// Keep files that are source: not `.env*`, not one of the build output dirs.
fn is_allowed(rel: &Path, output_dirs: &[&str]) -> bool {
    if rel
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with(".env"))
    {
        return false;
    }
    let outputs: Vec<String> = output_dirs
        .iter()
        .map(|d| d.trim_end_matches(['/', '\\']).replace('\\', "/"))
        .filter(|d| !d.is_empty())
        .collect();
    if outputs.is_empty() {
        return true;
    }
    let joined = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    !outputs
        .iter()
        .any(|dir| joined == *dir || joined.starts_with(&format!("{dir}/")))
}

fn git(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))
}

fn git_ok(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git(root, args)?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `None` when `root` is not inside a work tree.
fn git_state(root: &Path) -> Result<Option<GitInfo>, String> {
    match git_ok(root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(out) if out == "true" => {}
        Ok(_) => return Ok(None),
        Err(_) => return Ok(None),
    }

    let sha = git_ok(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let branch = git_ok(root, &["symbolic-ref", "--short", "-q", "HEAD"]).ok();
    let remotes = git_ok(root, &["remote"])?;
    let remote = remotes
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string);

    let ahead = if remote.is_some() {
        git_ok(root, &["rev-list", "--count", "HEAD", "--not", "--remotes"])
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    } else {
        // No remote at all: every commit is local-only.
        git_ok(root, &["rev-list", "--count", "HEAD"])
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };

    Ok(Some(GitInfo {
        sha,
        branch,
        remote,
        ahead,
    }))
}

/// Paths git reports as modified/staged/untracked, relative to `root`.
fn git_status(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    )?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // Porcelain paths are repo-root relative; drop the prefix down to `root`.
    let prefix = git_ok(root, &["rev-parse", "--show-prefix"]).unwrap_or_default();

    let mut paths = Vec::new();
    let mut entries = out.stdout.split(|b| *b == 0);
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let code = &entry[..2];
        let path = &entry[3..];
        // A rename/copy entry is followed by the old path; skip it.
        if code[0] == b'R' || code[0] == b'C' {
            entries.next();
        }
        // Deleted files cannot be snapshotted.
        if code[0] == b'D' || code[1] == b'D' {
            continue;
        }
        let raw = String::from_utf8_lossy(path).into_owned();
        let rel = match raw.strip_prefix(&prefix) {
            Some(rest) => PathBuf::from(rest),
            None if prefix.is_empty() => PathBuf::from(raw),
            None => continue,
        };
        if root.join(&rel).is_file() {
            paths.push(rel);
        }
    }
    Ok(paths)
}

/// Ignored files (relative to `root`) - candidates only when they are not
/// build output.
fn ignored_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = git(
        root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
    )?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(String::from_utf8_lossy(p).into_owned()))
        .collect())
}

/// Bundle every commit that is not on any remote. The file lives in the
/// system temp dir; the caller owns it after the snapshot is written.
fn create_bundle(root: &Path) -> Result<PathBuf, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "appz-backup-{}-{unique}.bundle",
        std::process::id()
    ));
    let path_str = path.to_string_lossy().into_owned();
    let out = git(
        root,
        &["bundle", "create", &path_str, "HEAD", "--not", "--remotes"],
    )?;
    if !out.status.success() {
        return Err(format!(
            "git bundle create failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(path)
}

/// Variable names from every `.env*` file (root and nested source files).
/// Only the name before `=` is used; values never leave the file.
fn env_names(root: &Path, source: &[PathBuf]) -> Vec<String> {
    let is_env = |p: &Path| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".env"))
    };

    let mut candidates: Vec<PathBuf> = source.iter().filter(|p| is_env(p)).cloned().collect();
    // `.env*.local` is volatile and filtered out of `source_files`, but its
    // names are still worth reporting.
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(".env") && entry.path().is_file() {
                candidates.push(PathBuf::from(name));
            }
        }
    }

    let mut names = BTreeSet::new();
    for rel in candidates {
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_start_matches("export ").trim();
            if !key.is_empty()
                && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && key.chars().next().is_some_and(|c| !c.is_ascii_digit())
            {
                names.insert(key.to_string());
            }
        }
    }
    names.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn write(&self, rel: &str, body: &str) {
            let path = self.path().join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, body).unwrap();
        }

        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .args(args)
                .current_dir(self.path())
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn commit_all(&self, message: &str) {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
        }
    }

    fn plain(files: &[(&str, &str)]) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo { dir };
        for (rel, body) in files {
            repo.write(rel, body);
        }
        repo
    }

    fn git_repo(files: &[(&str, &str)]) -> Repo {
        let repo = plain(files);
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "appz test"]);
        repo.commit_all("initial");
        repo
    }

    fn with_remote(repo: &Repo) -> tempfile::TempDir {
        // Outside the work tree: an in-repo bare repo would show up as
        // untracked source.
        let bare = tempfile::tempdir().unwrap();
        let out = Command::new("git")
            .args(["init", "--bare", "-q"])
            .arg(bare.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        repo.git(&["remote", "add", "origin", &bare.path().to_string_lossy()]);
        repo.git(&["push", "-q", "origin", "HEAD"]);
        bare
    }

    fn rels(files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn non_git_takes_source_minus_rebuildable_and_env() {
        let repo = plain(&[
            ("src/a.rs", "fn main() {}"),
            ("package-lock.json", "{}"),
            ("dist/x.js", "built"),
            ("node_modules/m/i.js", "dep"),
            (".env", "API_KEY=super-secret\n# comment\nEMPTY=\n"),
            (".env.local", "LOCAL_TOKEN=nope\n"),
        ]);

        let s = select(repo.path(), &["dist"]).unwrap();

        assert_eq!(rels(&s.files), ["package-lock.json", "src/a.rs"]);
        assert_eq!(
            s.env_names,
            ["API_KEY", "EMPTY", "LOCAL_TOKEN"],
            "names only, never values"
        );
        assert!(s.git.is_none());
        assert!(s.bundle.is_none());
        assert!(!format!("{s:?}").contains("super-secret"));
    }

    #[test]
    fn git_clean_and_pushed_selects_nothing() {
        let repo = git_repo(&[("src/a.rs", "fn main() {}")]);
        let _bare = with_remote(&repo);

        let s = select(repo.path(), &[]).unwrap();

        assert!(s.files.is_empty(), "files: {:?}", rels(&s.files));
        assert!(s.bundle.is_none());
        let git = s.git.expect("git info");
        assert_eq!(git.sha.len(), 40);
        assert_eq!(git.ahead, 0);
        assert_eq!(git.branch.as_deref(), Some("main"));
        assert_eq!(git.remote.as_deref(), Some("origin"));
    }

    #[test]
    fn git_takes_changes_untracked_and_needed_ignored() {
        let repo = git_repo(&[
            ("src/a.rs", "fn main() {}"),
            (".gitignore", "local.config.json\ndist/\n"),
        ]);
        let _bare = with_remote(&repo);

        repo.write("src/a.rs", "fn main() { println!(\"changed\"); }");
        repo.write("src/b.rs", "pub fn b() {}");
        repo.write("local.config.json", "{\"local\":true}");
        repo.write("dist/x.js", "built");

        let s = select(repo.path(), &["dist"]).unwrap();

        let mut files = rels(&s.files);
        files.sort();
        assert_eq!(
            files,
            ["local.config.json", "src/a.rs", "src/b.rs",],
            "ignored build output stays out"
        );
    }

    #[test]
    fn unpushed_commits_produce_a_bundle() {
        let repo = git_repo(&[("src/a.rs", "one")]);
        let _bare = with_remote(&repo);
        repo.write("src/a.rs", "two");
        repo.commit_all("second");

        let s = select(repo.path(), &[]).unwrap();

        let bundle = s.bundle.expect("unpushed commit must be bundled");
        assert_eq!(s.git.as_ref().unwrap().ahead, 1);
        let verify = Command::new("git")
            .args(["bundle", "verify"])
            .arg(&bundle)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(
            verify.status.success(),
            "bundle invalid: {}",
            String::from_utf8_lossy(&verify.stderr)
        );
        std::fs::remove_file(bundle).ok();
    }

    #[test]
    fn detached_head_without_upstream_still_bundles() {
        let repo = git_repo(&[("src/a.rs", "one")]);
        repo.git(&["checkout", "-q", "--detach"]);

        let s = select(repo.path(), &[]).unwrap();

        let git = s.git.expect("git info");
        assert!(git.branch.is_none(), "detached HEAD has no branch");
        assert!(git.remote.is_none());
        assert!(git.ahead > 0);
        let bundle = s.bundle.expect("all commits are local-only");
        std::fs::remove_file(bundle).ok();
    }

    #[test]
    fn paths_with_spaces_and_unicode_round_trip() {
        let repo = git_repo(&[("src/my file é.ts", "const a = 1;")]);
        let _bare = with_remote(&repo);
        repo.write("src/my file é.ts", "const a = 2;");

        let s = select(repo.path(), &[]).unwrap();

        assert_eq!(rels(&s.files), ["src/my file é.ts"]);
    }
}
