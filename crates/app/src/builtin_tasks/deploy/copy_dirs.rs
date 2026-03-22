use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use miette::miette;

use task::{Task, TaskRegistry};

fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

pub fn register_copy_dirs(reg: &mut TaskRegistry, dirs: Vec<&'static str>) {
    let dirs_arc: Arc<[&'static str]> = dirs.into();
    reg.register(
        Task::new(
            "deploy:copy_dirs",
            { let dirs_arc = dirs_arc.clone(); Arc::new(move |ctx: std::sync::Arc<task::Context>| -> std::pin::Pin<Box<dyn std::future::Future<Output = miette::Result<()>> + Send>> {
                let dirs_arc = dirs_arc.clone();
                Box::pin(async move {
                    let prev = match ctx.get("previous_release") {
                        Some(p) => PathBuf::from(p),
                        None => return Ok(()),
                    };
                    let release = match ctx.get("release_path") {
                        Some(p) => PathBuf::from(p),
                        None => return Ok(()),
                    };

                    for raw in dirs_arc.iter() {
                        let dir = raw.trim_matches('/');
                        let src = prev.join(dir);
                        if !src.exists() {
                            continue;
                        }
                        let dst_dir = release.join(dir);
                        crate::shell::copy_path_recursive(&src, &dst_dir)
                            .map_err(|e| miette!("Failed to copy directory {}: {}", dir, e))?;
                    }
                    Ok(())
                })
            }) as task::types::AsyncTaskFn },
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
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn copies_existing_directory() {
        let prev = unique_tmp_dir("copy-prev");
        let release = unique_tmp_dir("copy-release");
        let sub = prev.join("vendor");
        fs::create_dir_all(&sub).unwrap();
        File::create(sub.join("test.txt"))
            .unwrap()
            .write_all(b"hello")
            .unwrap();

        let mut reg = TaskRegistry::new();
        register_copy_dirs(&mut reg, vec!["vendor"]);

        let ctx = Arc::new(Context::new());
        ctx.set("previous_release", prev.to_str().unwrap());
        ctx.set("release_path", release.to_str().unwrap());

        let task = reg.get("deploy:copy_dirs").unwrap();
        let result = (task.action)(ctx).await;
        assert!(result.is_ok());
        assert!(release.join("vendor/test.txt").exists());

        fs::remove_dir_all(&prev).ok();
        fs::remove_dir_all(&release).ok();
    }
}
