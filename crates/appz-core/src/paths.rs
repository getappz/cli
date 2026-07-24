use std::path::{Path, PathBuf};

/// Canonicalize a path, stripping the `\\?\` verbatim-path prefix that
/// `std::fs::canonicalize` adds on Windows. cmd.exe and npm's `.cmd` shims
/// (which `mise run` tasks go through) mishandle `\\?\`-prefixed paths — a
/// spawned `astro.cmd` was observed resolving its own directory down to a
/// bare `C:`, crashing Node's module resolution. Any canonicalized path that
/// ends up as a child process's cwd or PATH entry must go through this
/// instead of `Path::canonicalize` directly.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    Ok(strip_verbatim_prefix(canonical))
}

fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

/// Find `node_modules/.bin` (and `node_modules/bin`) directories by walking up
/// from `project_path`. Framework CLIs (astro, remix, hexo, ...) are installed
/// there as local `devDependencies`, not as mise-managed tools, so `mise run`
/// tasks that invoke them bare (`astro build`) can't find them without this.
pub fn find_node_modules_bin_paths(project_path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let start = canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
    let mut current = Some(start.as_path());

    while let Some(dir) = current {
        let bin = dir.join("node_modules").join(".bin");
        if bin.is_dir() {
            paths.push(bin);
        }
        let bin_alt = dir.join("node_modules").join("bin");
        if bin_alt.is_dir() {
            paths.push(bin_alt);
        }
        current = dir.parent();
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn strip_verbatim_prefix_removes_windows_verbatim_marker() {
        if cfg!(windows) {
            let stripped = strip_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\shiva\project"));
            assert_eq!(stripped, PathBuf::from(r"C:\Users\shiva\project"));
        }
    }

    #[test]
    fn strip_verbatim_prefix_leaves_normal_paths_untouched() {
        let plain = PathBuf::from(r"C:\Users\shiva\project");
        assert_eq!(strip_verbatim_prefix(plain.clone()), plain);
    }

    #[test]
    fn find_node_modules_bin_paths_walks_up_from_nested_dir() {
        let root = std::env::temp_dir().join("appz-paths-test-nested");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("node_modules").join(".bin")).unwrap();
        let nested = root.join("packages").join("site");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(nested.join("node_modules").join(".bin")).unwrap();

        // Canonicalize the expected dirs the same way the function under
        // test does — on some Windows runners (e.g. GitHub Actions'
        // `runneradmin` user) canonicalize resolves to the 8.3 short name,
        // so comparing against a non-canonicalized path is flaky.
        let canonical_nested = canonicalize(&nested).unwrap();
        let canonical_root = canonicalize(&root).unwrap();

        let found = find_node_modules_bin_paths(&nested);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0], canonical_nested.join("node_modules").join(".bin"));
        assert_eq!(found[1], canonical_root.join("node_modules").join(".bin"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_node_modules_bin_paths_empty_when_none_exist() {
        let dir = std::env::temp_dir();
        let found = find_node_modules_bin_paths(&dir.join("appz-paths-test-nonexistent-xyz"));
        assert!(found.is_empty());
    }
}
