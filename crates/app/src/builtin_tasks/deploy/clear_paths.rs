use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;

use miette::miette;

use task::{Task, TaskRegistry};

fn remove_path(path: &PathBuf) -> miette::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        starbase_fs::remove_dir_all(path)
            .map_err(|e| miette!("Failed to remove directory: {}", e))?;
    } else {
        let _ = starbase_fs::remove_file(path);
    }
    Ok(())
}

/// Registers `deploy:clear_paths` which removes configured paths under `release_path`.
///
/// - `paths`: list of file/dir paths relative to `release_path` to remove.
pub fn register_clear_paths(reg: &mut TaskRegistry, paths: Vec<&'static str>) {
    let paths_static: &'static [&'static str] = Box::leak(paths.into_boxed_slice());
    reg.register(
        Task::new(
            "deploy:clear_paths",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let release = match ctx.get("release_path") {
                    Some(p) => PathBuf::from(p),
                    None => return Ok(()),
                };
                for raw in paths_static.iter() {
                    let rel = raw.trim_matches('/');
                    let target = release.join(rel);
                    remove_path(&target)?;
                }
                Ok(())
            }),
        )
        .desc("Cleanup files and/or directories"),
    );
}
