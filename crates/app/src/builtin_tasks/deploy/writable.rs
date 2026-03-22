use std::path::PathBuf;

use miette::miette;

use task::{Task, TaskRegistry};

#[cfg(unix)]
fn set_writable(path: &PathBuf) -> std::io::Result<()> {
    use std::{fs, os::unix::fs::PermissionsExt};
    let meta = fs::metadata(path)?;
    let mut perm = meta.permissions();
    perm.set_mode(0o775);
    fs::set_permissions(path, perm)
}

#[cfg(not(unix))]
fn set_writable(_path: &PathBuf) -> std::io::Result<()> {
    Ok(())
}

pub fn register_writable(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:writable",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let release = ctx
                    .get("release_path")
                    .ok_or_else(|| miette!("release_path not set"))?;
                let release_root = PathBuf::from(&release);
                if let Some(paths) = ctx.get("writable_dirs") {
                    for p in paths.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let dir = release_root.join(p);
                        if dir.exists() {
                            let _ = set_writable(&dir);
                        }
                    }
                }
                Ok(())
            }),
        )
        .desc("Set writable"),
    );
}
