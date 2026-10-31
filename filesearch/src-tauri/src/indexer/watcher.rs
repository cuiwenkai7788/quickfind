use crate::FileEntry;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// File change event
#[derive(Debug, Clone, serde::Serialize)]
pub enum FileChange {
    Created(String),
    Modified(String),
    Removed(String),
    Renamed { old_path: String, new_path: String },
}

/// File system watcher for real-time updates
pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    event_sender: Option<Sender<FileChange>>,
    is_running: Arc<parking_lot::RwLock<bool>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            event_sender: None,
            is_running: Arc::new(parking_lot::RwLock::new(false)),
        }
    }

    /// Start watching specified directories
    pub fn start(&mut self, paths: &[String], callback: impl Fn(FileChange) + Send + 'static) -> Result<(), String> {
        if *self.is_running.read() {
            return Err("Watcher already running".to_string());
        }

        let (event_tx, event_rx): (Sender<FileChange>, Receiver<FileChange>) = channel();

        // Create watcher
        let config = Config::default()
            .with_poll_interval(Duration::from_millis(500));

        let event_tx_clone = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if let Some(change) = Self::convert_event(&event) {
                        let _ = event_tx_clone.send(change);
                    }
                }
            },
            config,
        ).map_err(|e| format!("Failed to create watcher: {}", e))?;

        self.watcher = Some(watcher);
        self.event_sender = Some(event_tx);

        // Watch each path
        for path in paths {
            if let Some(ref mut watcher) = self.watcher {
                watcher.watch(PathBuf::from(path), RecursiveMode::Recursive)
                    .map_err(|e| format!("Failed to watch {}: {}", path, e))?;
                log::info!("Watching {}", path);
            }
        }

        *self.is_running.write() = true;

        // Spawn callback thread
        let is_running = self.is_running.clone();
        thread::spawn(move || {
            while *is_running.read() {
                match event_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(change) => {
                        callback(change);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Continue
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        *self.is_running.write() = false;

        if let Some(mut watcher) = self.watcher.take() {
            watcher.stop().ok();
        }

        self.event_sender = None;
        log::info!("File watcher stopped");
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        *self.is_running.read()
    }

    /// Convert notify event to FileChange
    fn convert_event(event: &Event) -> Option<FileChange> {
        // Skip common non-interesting events
        let path = event.paths.first()?;

        // Skip hidden and system files
        let file_name = path.file_name()?.to_string_lossy();
        if file_name.starts_with('.') || file_name.starts_with('$') {
            return None;
        }

        let path_str = path.to_string_lossy().to_string();

        match event.kind {
            EventKind::Create(_) => Some(FileChange::Created(path_str)),
            EventKind::Modify(_) => Some(FileChange::Modified(path_str)),
            EventKind::Remove(_) => Some(FileChange::Removed(path_str)),
            EventKind::Any => {
                // Generic change, treat as modify
                Some(FileChange::Modified(path_str))
            }
            _ => None,
        }
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}