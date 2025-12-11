use crate::error::Result;
use notify::{Event, RecommendedWatcher, Watcher};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tracing::{debug, info};

/// File system watcher that broadcasts changes to subscribers
pub struct FileWatcher {
    root_dir: PathBuf,
    tx: broadcast::Sender<FileChangeEvent>,
    _watcher: RecommendedWatcher,
}

/// Event representing a file system change
#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    /// File was created
    Created(PathBuf),
    /// File was modified
    Modified(PathBuf),
    /// File was deleted
    Deleted(PathBuf),
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(root_dir: PathBuf) -> Result<(Self, broadcast::Receiver<FileChangeEvent>)> {
        let (tx, rx) = broadcast::channel(100);
        let root_dir_clone = root_dir.clone();

        // Create watcher with async channel
        let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(100);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = watcher_tx.try_send(event);
            }
        })?;

        // Watch the root directory recursively
        watcher.watch(&root_dir, notify::RecursiveMode::Recursive)?;

        info!("Watching directory: {}", root_dir.display());

        // Spawn task to process watcher events with debouncing
        let tx_clone = tx.clone();
        let root_dir_for_task = root_dir.clone();
        tokio::spawn(async move {
            let mut pending_events = Vec::new();
            let mut debounce_timer = tokio::time::interval(tokio::time::Duration::from_millis(200));

            loop {
                tokio::select! {
                    Some(event) = watcher_rx.recv() => {
                        // Filter out ignored paths
                        if Self::should_ignore(&event, &root_dir_for_task) {
                            continue;
                        }

                        // Collect events for debouncing
                        for path in &event.paths {
                            if path.starts_with(&root_dir_for_task) {
                                let relative_path = path.strip_prefix(&root_dir_for_task)
                                    .unwrap_or(path)
                                    .to_path_buf();

                                match event.kind {
                                    notify::EventKind::Create(_) => {
                                        pending_events.push(FileChangeEvent::Created(relative_path));
                                    }
                                    notify::EventKind::Modify(_) => {
                                        pending_events.push(FileChangeEvent::Modified(relative_path));
                                    }
                                    notify::EventKind::Remove(_) => {
                                        pending_events.push(FileChangeEvent::Deleted(relative_path));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ = debounce_timer.tick() => {
                        // Send all pending events
                        for event in pending_events.drain(..) {
                            if tx_clone.send(event.clone()).is_ok() {
                                debug!("File change event: {:?}", event);
                            }
                        }
                    }
                }
            }
        });

        Ok((
            Self {
                root_dir,
                tx,
                _watcher: watcher,
            },
            rx,
        ))
    }

    /// Subscribe to file change events
    pub fn subscribe(&self) -> broadcast::Receiver<FileChangeEvent> {
        self.tx.subscribe()
    }

    /// Check if a path should be ignored
    fn should_ignore(event: &notify::Event, root_dir: &PathBuf) -> bool {
        for path in &event.paths {
            let relative = path.strip_prefix(root_dir).unwrap_or(path);
            let path_str = relative.to_string_lossy();

            // Ignore common directories and files
            if path_str.contains(".git/")
                || path_str.contains("node_modules/")
                || path_str.contains("target/")
                || path_str.contains(".next/")
                || path_str.contains("dist/")
                || path_str.contains("build/")
                || path_str.starts_with(".")
                || path_str.contains("__pycache__/")
            {
                return true;
            }
        }
        false
    }
}
