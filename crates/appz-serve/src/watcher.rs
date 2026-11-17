use crate::error::Result;
use notify::{RecommendedWatcher, Watcher};
use std::path::PathBuf;
use tokio::sync::broadcast;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

impl FileWatcher {
    pub fn new(root_dir: PathBuf) -> Result<(Self, broadcast::Receiver<FileChangeEvent>)> {
        let (tx, rx) = broadcast::channel(100);
        let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(100);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let _ = watcher_tx.try_send(event);
            }
        })?;

        watcher.watch(&root_dir, notify::RecursiveMode::Recursive)?;

        let root_for_task = root_dir.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut pending_events = Vec::new();
            let mut debounce =
                tokio::time::interval(tokio::time::Duration::from_millis(200));

            loop {
                tokio::select! {
                    Some(event) = watcher_rx.recv() => {
                        if should_ignore(&event, &root_for_task) {
                            continue;
                        }
                        for path in &event.paths {
                            if !path.starts_with(&root_for_task) {
                                continue;
                            }
                            let relative_path = path
                                .strip_prefix(&root_for_task)
                                .unwrap_or(path)
                                .to_path_buf();
                            let mapped = match event.kind {
                                notify::EventKind::Create(_) => FileChangeEvent::Created(relative_path),
                                notify::EventKind::Modify(_) => FileChangeEvent::Modified(relative_path),
                                notify::EventKind::Remove(_) => FileChangeEvent::Deleted(relative_path),
                                _ => continue,
                            };
                            pending_events.push(mapped);
                        }
                    }
                    _ = debounce.tick() => {
                        for event in pending_events.drain(..) {
                            let _ = tx_clone.send(event);
                        }
                    }
                }
            }
        });

        Ok((Self { _watcher: watcher }, rx))
    }
}

fn should_ignore(event: &notify::Event, root_dir: &PathBuf) -> bool {
    for path in &event.paths {
        let relative = path.strip_prefix(root_dir).unwrap_or(path);
        let path_str = relative.to_string_lossy();
        if path_str.contains(".git/")
            || path_str.contains("node_modules/")
            || path_str.contains("target/")
            || path_str.contains(".next/")
            || path_str.contains("dist/")
            || path_str.contains("build/")
            || path_str.starts_with('.')
            || path_str.contains("__pycache__/")
            || path_str.contains(".appz/")
        {
            return true;
        }
    }
    false
}