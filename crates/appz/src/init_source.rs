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
}
