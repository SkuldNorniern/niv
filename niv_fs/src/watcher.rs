//! External file change detection and conflict resolution
//!
//! This module provides:
//! - Cross-platform file system watching (polling-based for no external deps)
//! - Debounced change detection
//! - Three-way merge conflict resolution
//! - Auto-reload for clean buffers
//! - Rename/move following using file identity

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::file::identity::{FileIdentity, FileIdentityConfig};

/// Errors that can occur during file watching operations
#[derive(Debug)]
pub enum WatcherError {
    Io(io::Error),
    PathError(String),
    WatcherStopped,
    ConflictResolutionFailed(String),
    IdentityError(String),
}

impl std::fmt::Display for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatcherError::Io(err) => write!(f, "I/O error: {}", err),
            WatcherError::PathError(msg) => write!(f, "Path error: {}", msg),
            WatcherError::WatcherStopped => write!(f, "Watcher has been stopped"),
            WatcherError::ConflictResolutionFailed(msg) => {
                write!(f, "Conflict resolution failed: {}", msg)
            }
            WatcherError::IdentityError(msg) => write!(f, "Identity error: {}", msg),
        }
    }
}

impl std::error::Error for WatcherError {}

impl From<io::Error> for WatcherError {
    fn from(err: io::Error) -> Self {
        WatcherError::Io(err)
    }
}

pub type WatcherResult<T> = Result<T, WatcherError>;

/// Configuration for file watching behavior
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// How often to poll for file changes
    pub poll_interval: Duration,
    /// Delay before considering a change stable (debouncing)
    pub debounce_delay: Duration,
    /// Whether to auto-reload when buffer is clean
    pub auto_reload: bool,
    /// Maximum number of snapshots to keep per file
    pub max_snapshots: usize,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(500),
            debounce_delay: Duration::from_millis(100),
            auto_reload: true,
            max_snapshots: 10,
        }
    }
}

/// Types of file changes
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// File content was modified
    Modified,
    /// File was created
    Created,
    /// File was deleted
    Deleted,
    /// File was renamed/moved
    Renamed,
}

/// File change information
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub timestamp: SystemTime,
    pub old_identity: Option<FileIdentity>,
    pub new_identity: Option<FileIdentity>,
}

/// Snapshot of file state for three-way merge
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub content: String,
    pub identity: FileIdentity,
    pub timestamp: SystemTime,
}

/// Current state of a watched file
#[derive(Debug, Clone)]
pub struct FileState {
    pub buffer_content: String,
    pub disk_content: String,
    pub base_content: String,
    pub identity: FileIdentity,
    pub is_dirty: bool,
    pub last_modified: SystemTime,
    pub snapshots: Vec<FileSnapshot>,
}

/// Three-way merge conflict information
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub file_path: PathBuf,
    pub buffer_content: String,
    pub disk_content: String,
    pub base_content: String,
    pub buffer_identity: FileIdentity,
    pub disk_identity: FileIdentity,
    pub base_identity: FileIdentity,
}

/// How to resolve a merge conflict
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    /// Use the buffer content (overwrite disk)
    UseBuffer,
    /// Use the disk content (discard buffer changes)
    UseDisk,
    /// Keep both files (buffer as new file, disk unchanged)
    KeepBoth,
    /// Manual resolution (return conflict for user)
    Manual,
}

/// Events emitted by the file watcher
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// File was changed externally
    FileChanged(FileChange),
    /// File was deleted
    FileDeleted(PathBuf),
    /// File was created
    FileCreated(PathBuf),
    /// File was renamed/moved
    FileRenamed { from: PathBuf, to: PathBuf },
    /// Merge conflict detected
    ConflictDetected(MergeConflict),
    /// File was auto-reloaded
    AutoReloaded { path: PathBuf, content: String },
}

/// File watcher for external change detection and conflict resolution
pub struct FileWatcher {
    config: WatcherConfig,
    watched_files: Arc<Mutex<HashMap<PathBuf, FileState>>>,
    event_callbacks: Arc<Mutex<Vec<Box<dyn Fn(WatchEvent) + Send + Sync>>>>,
    event_sender: Option<Sender<WatchEvent>>,
    event_receiver: Option<Receiver<WatchEvent>>,
    is_running: Arc<AtomicBool>,
    identity_config: FileIdentityConfig,
}

impl FileWatcher {
    pub fn new(config: WatcherConfig) -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            config,
            watched_files: Arc::new(Mutex::new(HashMap::new())),
            event_callbacks: Arc::new(Mutex::new(Vec::new())),
            event_sender: Some(tx),
            event_receiver: Some(rx),
            is_running: Arc::new(AtomicBool::new(false)),
            identity_config: FileIdentityConfig::default(),
        }
    }

    /// Start the file watcher
    pub fn start(&self) -> WatcherResult<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);

        let watched_files = Arc::clone(&self.watched_files);
        let event_sender = self.event_sender.as_ref().unwrap().clone();
        let config = self.config.clone();
        let is_running = Arc::clone(&self.is_running);

        thread::spawn(move || {
            Self::watcher_thread(watched_files, event_sender, config, is_running);
        });

        Ok(())
    }

    /// Stop the file watcher
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Add an event callback
    pub fn add_callback<F>(&self, callback: Box<F>) -> WatcherResult<()>
    where
        F: Fn(WatchEvent) + Send + Sync + 'static,
    {
        let mut callbacks = self.event_callbacks.lock().unwrap();
        callbacks.push(callback);
        Ok(())
    }

    /// Process pending events and call callbacks
    pub fn process_events(&self) -> WatcherResult<()> {
        if let Some(receiver) = &self.event_receiver {
            // Process all pending events
            while let Ok(event) = receiver.try_recv() {
                // Call all registered callbacks
                let callbacks = self.event_callbacks.lock().unwrap();
                for callback in &*callbacks {
                    callback(event.clone());
                }
            }
        }
        Ok(())
    }

    /// Watch a file with initial content and identity
    pub fn watch_file(
        &self,
        path: &Path,
        initial_content: &str,
        initial_identity: FileIdentity,
    ) -> WatcherResult<()> {
        let mut watched_files = self.watched_files.lock().unwrap();

        let file_state = FileState {
            buffer_content: initial_content.to_string(),
            disk_content: initial_content.to_string(),
            base_content: initial_content.to_string(),
            identity: initial_identity,
            is_dirty: false,
            last_modified: SystemTime::now(),
            snapshots: Vec::new(),
        };

        watched_files.insert(path.to_path_buf(), file_state);
        Ok(())
    }

    /// Update buffer content and mark as dirty
    pub fn update_buffer(&self, path: &Path, content: &str) -> WatcherResult<()> {
        let mut watched_files = self.watched_files.lock().unwrap();

        if let Some(file_state) = watched_files.get_mut(path) {
            file_state.buffer_content = content.to_string();
            file_state.is_dirty = true;

            // Take snapshot for three-way merge
            let snapshot = FileSnapshot {
                content: file_state.disk_content.clone(),
                identity: file_state.identity.clone(),
                timestamp: SystemTime::now(),
            };

            file_state.snapshots.push(snapshot);
            if file_state.snapshots.len() > self.config.max_snapshots {
                file_state.snapshots.remove(0);
            }
        }

        Ok(())
    }

    /// Check for external changes to a file
    pub fn check_external_changes(&self, path: &Path) -> WatcherResult<Option<FileChange>> {
        let watched_files = self.watched_files.lock().unwrap();

        if let Some(file_state) = watched_files.get(path) {
            // Check if file exists and get current identity
            let current_identity = match FileIdentity::from_path(path, &self.identity_config) {
                Ok(identity) => identity,
                Err(_) => {
                    // File was deleted
                    return Ok(Some(FileChange {
                        path: path.to_path_buf(),
                        change_type: ChangeType::Deleted,
                        timestamp: SystemTime::now(),
                        old_identity: Some(file_state.identity.clone()),
                        new_identity: None,
                    }));
                }
            };

            // Check if identity changed (file was modified or replaced)
            if current_identity != file_state.identity {
                return Ok(Some(FileChange {
                    path: path.to_path_buf(),
                    change_type: ChangeType::Modified,
                    timestamp: SystemTime::now(),
                    old_identity: Some(file_state.identity.clone()),
                    new_identity: Some(current_identity),
                }));
            }
        }

        Ok(None)
    }

    /// Handle potential merge conflict
    pub fn handle_conflict(
        &self,
        path: &Path,
        buffer_content: &str,
    ) -> WatcherResult<Option<MergeConflict>> {
        let watched_files = self.watched_files.lock().unwrap();

        if let Some(file_state) = watched_files.get(path) {
            if file_state.is_dirty {
                // Read current disk content
                let disk_content = match fs::read_to_string(path) {
                    Ok(content) => content,
                    Err(_) => return Ok(None), // File doesn't exist
                };

                let disk_identity = match FileIdentity::from_path(path, &self.identity_config) {
                    Ok(identity) => identity,
                    Err(_) => return Ok(None),
                };

                // Find base content (last common ancestor)
                let base_content = file_state
                    .snapshots
                    .last()
                    .map(|s| s.content.clone())
                    .unwrap_or_else(|| file_state.base_content.clone());

                let base_identity = file_state
                    .snapshots
                    .last()
                    .map(|s| s.identity.clone())
                    .unwrap_or_else(|| file_state.identity.clone());

                // Check if there are actual differences
                if buffer_content != disk_content && disk_content != base_content {
                    let conflict = MergeConflict {
                        file_path: path.to_path_buf(),
                        buffer_content: buffer_content.to_string(),
                        disk_content,
                        base_content,
                        buffer_identity: file_state.identity.clone(),
                        disk_identity,
                        base_identity,
                    };

                    return Ok(Some(conflict));
                }
            }
        }

        Ok(None)
    }

    /// Resolve a merge conflict
    pub fn resolve_conflict(
        &self,
        conflict: &MergeConflict,
        resolution: ConflictResolution,
    ) -> WatcherResult<String> {
        match resolution {
            ConflictResolution::UseBuffer => {
                // Overwrite disk with buffer content
                fs::write(&conflict.file_path, &conflict.buffer_content)?;
                Ok(conflict.buffer_content.clone())
            }
            ConflictResolution::UseDisk => {
                // Keep disk content, buffer will be updated on next load
                Ok(conflict.disk_content.clone())
            }
            ConflictResolution::KeepBoth => {
                // Save buffer content as new file
                let new_path = conflict.file_path.with_extension("buffer");
                fs::write(&new_path, &conflict.buffer_content)?;
                Ok(conflict.disk_content.clone())
            }
            ConflictResolution::Manual => Err(WatcherError::ConflictResolutionFailed(
                "Manual resolution required".to_string(),
            )),
        }
    }

    /// Follow a file rename/move using identity
    pub fn follow_rename(&self, old_path: &Path, new_path: &Path) -> WatcherResult<bool> {
        let mut watched_files = self.watched_files.lock().unwrap();

        if let Some(file_state) = watched_files.remove(old_path) {
            // Check if the new path has the same identity
            let new_identity = match FileIdentity::from_path(new_path, &self.identity_config) {
                Ok(identity) => identity,
                Err(_) => return Ok(false),
            };

            if new_identity == file_state.identity {
                // Same file, just update path
                watched_files.insert(new_path.to_path_buf(), file_state);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Watcher thread function
    fn watcher_thread(
        watched_files: Arc<Mutex<HashMap<PathBuf, FileState>>>,
        event_sender: Sender<WatchEvent>,
        config: WatcherConfig,
        is_running: Arc<AtomicBool>,
    ) {
        let mut last_check = HashMap::new();
        let mut pending_changes = HashMap::new();

        while is_running.load(Ordering::Relaxed) {
            thread::sleep(config.poll_interval);

            let files_to_check: Vec<PathBuf> = {
                let watched = watched_files.lock().unwrap();
                watched.keys().cloned().collect()
            };

            for file_path in files_to_check {
                let change = Self::check_file_change(&file_path, &last_check);

                match change {
                    Ok(Some(file_change)) => {
                        // Debounce: store change and wait for debounce delay
                        let now = Instant::now();
                        pending_changes.insert(file_path.clone(), (file_change, now));

                        // Send event after debounce delay
                        if let Some((change, timestamp)) = pending_changes.get(&file_path) {
                            if now.duration_since(*timestamp) >= config.debounce_delay {
                                let event = match change.change_type {
                                    ChangeType::Modified => WatchEvent::FileChanged(change.clone()),
                                    ChangeType::Created => {
                                        WatchEvent::FileCreated(change.path.clone())
                                    }
                                    ChangeType::Deleted => {
                                        WatchEvent::FileDeleted(change.path.clone())
                                    }
                                    ChangeType::Renamed => WatchEvent::FileRenamed {
                                        from: change.path.clone(),
                                        to: change.path.clone(),
                                    },
                                };

                                let _ = event_sender.send(event);
                                pending_changes.remove(&file_path);
                            }
                        }
                    }
                    Ok(None) => {
                        // File unchanged, remove from pending if present
                        pending_changes.remove(&file_path);
                    }
                    Err(_) => {
                        // File error, send deleted event
                        let _ = event_sender.send(WatchEvent::FileDeleted(file_path.clone()));
                    }
                }

                last_check.insert(file_path, SystemTime::now());
            }
        }
    }

    /// Check if a file has changed
    fn check_file_change(
        file_path: &Path,
        last_check: &HashMap<PathBuf, SystemTime>,
    ) -> WatcherResult<Option<FileChange>> {
        let metadata = match fs::metadata(file_path) {
            Ok(meta) => meta,
            Err(_) => {
                // File doesn't exist or can't be accessed
                return Ok(Some(FileChange {
                    path: file_path.to_path_buf(),
                    change_type: ChangeType::Deleted,
                    timestamp: SystemTime::now(),
                    old_identity: None,
                    new_identity: None,
                }));
            }
        };

        let current_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        if let Some(&last_modified) = last_check.get(file_path) {
            if current_modified > last_modified {
                return Ok(Some(FileChange {
                    path: file_path.to_path_buf(),
                    change_type: ChangeType::Modified,
                    timestamp: current_modified,
                    old_identity: None,
                    new_identity: None,
                }));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> WatcherConfig {
        WatcherConfig {
            poll_interval: Duration::from_millis(50),
            debounce_delay: Duration::from_millis(10),
            auto_reload: true,
            max_snapshots: 5,
        }
    }

    #[test]
    fn test_watcher_creation() {
        let config = test_config();
        let watcher = FileWatcher::new(config);
        assert!(!watcher.is_running.load(Ordering::Relaxed));
    }
}
