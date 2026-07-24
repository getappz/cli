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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_target_dir_errors_when_existing_and_not_forced() {
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
