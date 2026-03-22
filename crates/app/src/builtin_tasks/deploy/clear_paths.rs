use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;
use std::sync::Arc;

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

pub fn register_clear_paths(reg: &mut TaskRegistry, paths: Vec<&'static str>) {
    let paths_arc: Arc<[&'static str]> = paths.into();
    reg.register(
        Task::new(
            "deploy:clear_paths",
            { let paths_arc = paths_arc.clone(); Arc::new(move |ctx: std::sync::Arc<task::Context>| -> std::pin::Pin<Box<dyn std::future::Future<Output = miette::Result<()>> + Send>> {
                let paths_arc = paths_arc.clone();
                Box::pin(async move {
                    let release = match ctx.get("release_path") {
                        Some(p) => PathBuf::from(p),
                        None => return Ok(()),
                    };
                    for raw in paths_arc.iter() {
                        let rel = raw.trim_matches('/');
                        let target = release.join(rel);
                        remove_path(&target)?;
                    }
                    Ok(())
                })
            }) as task::types::AsyncTaskFn },
        )
        .desc("Cleanup files and/or directories"),
    );
}
