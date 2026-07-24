//! Resolves an `appz init` source argument (local path or remote git URL)
//! to a local directory, cloning or downloading it first if it's remote.

/// Compute the local target directory for a cloned/downloaded remote repo
/// (`./<repo>` under the current directory) and make sure it's available:
/// removes an existing directory when `force` is set, errors otherwise —
/// same semantics as `git clone`.
fn resolve_target_dir(repo: &str, force: bool) -> Result<std::path::PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot determine current dir: {e}"))?;
    let target = cwd.join(repo);
    if target.exists() {
        if force {
            std::fs::remove_dir_all(&target)
                .map_err(|e| format!("failed to remove existing '{}': {e}", target.display()))?;
        } else {
            return Err(format!(
                "'{}' already exists — use --force to overwrite",
                target.display()
            ));
        }
    }
    Ok(target)
}

/// Capture `gh`'s stdout for `args`, or `None` if `gh` is missing, not
/// authenticated, or exits non-zero. Never panics — an ownership check that
/// can't be confirmed must fail closed, not crash `appz init`.
fn gh_stdout(args: &[&str]) -> Option<String> {
    let output = command::Command::new("gh").args(args).exec().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether the authenticated `gh` user owns `owner` — either directly (their
/// own login) or via an org they belong to. Only meaningful for GitHub;
/// GitLab/Bitbucket have no equivalent check via `gh`, so they always report
/// `false` (routing the caller to the template-download path).
fn is_owned(host: appz_core::Host, owner: &str) -> bool {
    if host != appz_core::Host::GitHub {
        return false;
    }
    let Some(login) = gh_stdout(&["api", "user", "--jq", ".login"]) else {
        return false;
    };
    if login.eq_ignore_ascii_case(owner) {
        return true;
    }
    let Some(orgs) = gh_stdout(&["api", "user/orgs", "--jq", ".[].login"]) else {
        return false;
    };
    orgs.lines().any(|org| org.eq_ignore_ascii_case(owner))
}

/// `git clone <url> <target>` — a full clone (history, branches) since this
/// path is only taken for repos the user owns and may push back to.
fn clone_repo(remote: &appz_core::RemoteSource, target: &std::path::Path) -> Result<(), String> {
    let output = command::Command::new("git")
        .arg("clone")
        .arg(&remote.url)
        .arg(target)
        .exec()
        .map_err(|e| format!("failed to run git clone: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Resolve which branch to download for `remote`. GitHub: ask the API for
/// the repo's default branch, falling back to `main`/`master` if that call
/// fails. GitLab/Bitbucket: no API call available here — try `main` then
/// `master` directly, same as GitHub's own fallback.
fn candidate_refs(remote: &appz_core::RemoteSource) -> Vec<String> {
    if remote.host == appz_core::Host::GitHub
        && let Some(branch) = github_default_branch(&remote.owner, &remote.repo)
    {
        return vec![branch];
    }
    vec!["main".to_string(), "master".to_string()]
}

fn github_default_branch(owner: &str, repo: &str) -> Option<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(10))
        .build();
    let resp = agent
        .get(&format!("https://api.github.com/repos/{owner}/{repo}"))
        .set("User-Agent", "appz")
        .call()
        .ok()?;
    let json: serde_json::Value = resp.into_json().ok()?;
    json["default_branch"].as_str().map(str::to_string)
}

fn download_zip(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(60))
        .build();
    let resp = agent
        .get(url)
        .set("User-Agent", "appz")
        .call()
        .map_err(|e| format!("download failed ({url}): {e}"))?;
    let mut data = Vec::new();
    resp.into_reader()
        .read_to_end(&mut data)
        .map_err(|e| format!("read error: {e}"))?;
    Ok(data)
}

/// Extract a repo archive's zip bytes into `target`, unwrapping the single
/// top-level directory GitHub/GitLab/Bitbucket archives wrap their contents
/// in (e.g. `appz-dev-site-main/`) so `target` ends up holding the repo's
/// files directly, not one level deeper.
///
/// The actual work happens in `extract_into`, using a temp directory that
/// this function unconditionally cleans up afterwards — whether `extract_into`
/// succeeded or bailed out on its first failing step, not just on the last one.
fn extract_template(data: &[u8], target: &std::path::Path) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("appz-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let result = extract_into(data, target, &tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Does the actual extraction into `tmp`, then copies the unwrapped
/// top-level directory's contents into `target`. Split out from
/// `extract_template` so every fallible step here — however it fails —
/// still lets the caller clean up `tmp` afterwards.
fn extract_into(data: &[u8], target: &std::path::Path, tmp: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(tmp).map_err(|e| format!("create tmpdir: {e}"))?;

    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("zip error: {e}"))?;
    archive
        .extract(tmp)
        .map_err(|e| format!("extract error: {e}"))?;

    let extracted_root = std::fs::read_dir(tmp)
        .map_err(|e| format!("read tmpdir: {e}"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .ok_or_else(|| "archive did not contain a top-level directory".to_string())?;

    std::fs::create_dir_all(target).map_err(|e| format!("create target: {e}"))?;
    copy_dir_contents(&extracted_root, target)
}

fn copy_dir_contents(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(src).map_err(|e| format!("read '{}': {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            std::fs::create_dir_all(&to)
                .map_err(|e| format!("create '{}': {e}", to.display()))?;
            copy_dir_contents(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy '{}': {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Download `remote` as a template archive (no `.git`) into `target`: try
/// each candidate ref's archive URL in order, extracting the first one that
/// downloads successfully.
fn download_template(remote: &appz_core::RemoteSource, target: &std::path::Path) -> Result<(), String> {
    let refs_to_try = candidate_refs(remote);
    let mut last_err = None;
    for ref_name in &refs_to_try {
        let url = appz_core::archive_url(remote.host, &remote.owner, &remote.repo, ref_name);
        match download_zip(&url) {
            Ok(bytes) => return extract_template(&bytes, target),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        format!(
            "could not resolve a branch to download for {}/{}",
            remote.owner, remote.repo
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // `set_current_dir` mutates process-wide (not per-thread) state, and
    // `cargo test` runs tests in parallel threads by default. Serialize the
    // two tests below so they never race each other's cwd changes.
    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn resolve_target_dir_errors_when_existing_and_not_forced() {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cwd = std::env::temp_dir().join(format!("appz-init-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&cwd);
        fs::create_dir_all(&cwd).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&cwd).unwrap();

        fs::create_dir(cwd.join("existing-repo")).unwrap();
        let result = resolve_target_dir("existing-repo", false);

        std::env::set_current_dir(&prev).unwrap();
        let _ = fs::remove_dir_all(&cwd);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--force"));
    }

    #[test]
    fn resolve_target_dir_removes_existing_when_forced() {
        let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cwd = std::env::temp_dir().join(format!("appz-init-test-force-{}", std::process::id()));
        let _ = fs::remove_dir_all(&cwd);
        fs::create_dir_all(&cwd).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&cwd).unwrap();

        let existing = cwd.join("existing-repo");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("marker.txt"), "old").unwrap();
        let result = resolve_target_dir("existing-repo", true);

        std::env::set_current_dir(&prev).unwrap();

        assert!(result.is_ok());
        let target = result.unwrap();
        assert!(!target.join("marker.txt").exists());
        let _ = fs::remove_dir_all(&cwd);
    }

    #[test]
    fn extract_template_unwraps_top_level_dir_into_target() {
        // Build a minimal in-memory zip shaped like a GitHub codeload archive:
        // a single top-level "repo-main/" directory containing one file.
        let mut buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default();
            zip.add_directory("repo-main/", opts).unwrap();
            zip.start_file("repo-main/package.json", opts).unwrap();
            use std::io::Write;
            zip.write_all(b"{}").unwrap();
            zip.finish().unwrap();
        }

        let target = std::env::temp_dir().join(format!(
            "appz-init-extract-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&target);

        let result = extract_template(&buf, &target);

        assert!(result.is_ok(), "{:?}", result.err());
        assert!(target.join("package.json").exists());
        assert!(!target.join("repo-main").exists());

        let _ = std::fs::remove_dir_all(&target);
    }
}
