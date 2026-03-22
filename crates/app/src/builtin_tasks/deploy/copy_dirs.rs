use std::{
    fs, io,
    path::{Path, PathBuf},
};

use miette::miette;

use task::{Task, TaskRegistry};

fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

// replaced by shared shell::copy_path_recursive

/// Registers `deploy:copy_dirs` which copies selected directories from previous_release to release_path.
///
/// - `dirs`: list of directory paths relative to release root (e.g., `node_modules`).
/// - Uses context variables: `previous_release` and `release_path`.
pub fn register_copy_dirs(reg: &mut TaskRegistry, dirs: Vec<&'static str>) {
    let dirs_static: &'static [&'static str] = Box::leak(dirs.into_boxed_slice());
    reg.register(
        Task::new(
            "deploy:copy_dirs",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let prev = match ctx.get("previous_release") {
                    Some(p) => PathBuf::from(p),
                    None => return Ok(()),
                };
                let release = match ctx.get("release_path") {
                    Some(p) => PathBuf::from(p),
                    None => return Ok(()),
                };

                for raw in dirs_static.iter() {
                    let dir = raw.trim_matches('/');
                    let src = prev.join(dir);
                    if !src.exists() {
                        continue;
                    }
                    // destination directory path handling
                    let dst_dir = release.join(dir);
                    crate::shell::copy_path_recursive(&src, &dst_dir)
                        .map_err(|e| miette!("Failed to copy directory {}: {}", dir, e))?;
                }
                Ok(())
            }),
        )
        .desc("Copies directories"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };
    use task::{Context, TaskRegistry};

    fn unique_tmp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}", prefix, nanos));
        p
    }

    #[test]
    fn copies_selected_directories_from_previous_to_release() {
        let prev = unique_tmp_dir("prev");
        let rel = unique_tmp_dir("rel");
        fs::create_dir_all(&prev).unwrap();
        fs::create_dir_all(&rel).unwrap();

        // Prepare source dirs
        let nm = prev.join("node_modules");
        let vend_assets = prev.join("vendor").join("assets");
        fs::create_dir_all(&nm).unwrap();
        fs::create_dir_all(&vend_assets).unwrap();
        // Create files
        let mut f1 = File::create(nm.join("a.txt")).unwrap();
        writeln!(f1, "hello").unwrap();
        let mut f2 = File::create(vend_assets.join("b.txt")).unwrap();
        writeln!(f2, "world").unwrap();

        // Register and run task
        let mut reg = TaskRegistry::new();
        register_copy_dirs(
            &mut reg,
            vec!["node_modules", "vendor/assets", "missing_dir"],
        );
        let task = reg.get("deploy:copy_dirs").unwrap().action.clone();

        let ctx = std::sync::Arc::new({
            let mut c = Context::new();
            c.set("previous_release", prev.to_string_lossy());
            c.set("release_path", rel.to_string_lossy());
            c
        });
        // Run task using tokio runtime
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            (task)(ctx.clone()).await.unwrap();
        });

        // Assert files copied
        assert!(rel.join("node_modules").join("a.txt").exists());
        assert!(rel.join("vendor").join("assets").join("b.txt").exists());
        assert!(!rel.join("missing_dir").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&prev);
        let _ = fs::remove_dir_all(&rel);
    }
}
