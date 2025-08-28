//! Swap file management for crash recovery and periodic saves
//!
//! This module provides:
//! - Swap files for each buffer (.~filename.swp or app cache)
//! - Periodic saves (every N edits or idle timeout)
//! - Cursor and viewport state preservation
//! - Crash recovery detection and restoration prompts
//! - Draft management for untitled buffers with UUIDs

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Errors that can occur during swap operations
#[derive(Debug)]
pub enum SwapError {
    Io(io::Error),
    Serialization(String),
    Deserialization(String),
    PathError(String),
    RecoveryFailed(String),
}

impl std::fmt::Display for SwapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwapError::Io(err) => write!(f, "I/O error: {}", err),
            SwapError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            SwapError::Deserialization(msg) => write!(f, "Deserialization error: {}", msg),
            SwapError::PathError(msg) => write!(f, "Path error: {}", msg),
            SwapError::RecoveryFailed(msg) => write!(f, "Recovery failed: {}", msg),
        }
    }
}

impl std::error::Error for SwapError {}

impl From<io::Error> for SwapError {
    fn from(err: io::Error) -> Self {
        SwapError::Io(err)
    }
}

/// Errors that can occur during draft operations
#[derive(Debug)]
pub enum DraftError {
    Io(io::Error),
    UuidError(String),
    PathError(String),
}

impl std::fmt::Display for DraftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DraftError::Io(err) => write!(f, "I/O error: {}", err),
            DraftError::UuidError(msg) => write!(f, "UUID error: {}", msg),
            DraftError::PathError(msg) => write!(f, "Path error: {}", msg),
        }
    }
}

impl std::error::Error for DraftError {}

impl From<io::Error> for DraftError {
    fn from(err: io::Error) -> Self {
        DraftError::Io(err)
    }
}

pub type SwapResult<T> = Result<T, SwapError>;
pub type DraftResult<T> = Result<T, DraftError>;

/// Configuration for swap file behavior
#[derive(Debug, Clone)]
pub struct SwapConfig {
    /// Directory for swap files (defaults to system temp)
    pub swap_dir: PathBuf,
    /// Directory for draft files
    pub draft_dir: PathBuf,
    /// Save swap after this many edits
    pub edits_threshold: usize,
    /// Save swap after this idle duration
    pub idle_timeout: Duration,
    /// Include cursor position in swap
    pub save_cursor: bool,
    /// Include viewport state in swap
    pub save_viewport: bool,
    /// Maximum swap file age before cleanup (days)
    pub max_age_days: u64,
}

impl Default for SwapConfig {
    fn default() -> Self {
        Self {
            swap_dir: std::env::temp_dir().join("niv_swap"),
            draft_dir: std::env::temp_dir().join("niv_swap").join("drafts"),
            edits_threshold: 10,
            idle_timeout: Duration::from_secs(5),
            save_cursor: true,
            save_viewport: true,
            max_age_days: 7,
        }
    }
}

/// Status of a swap file
#[derive(Debug, Clone, PartialEq)]
pub enum SwapStatus {
    /// Swap file is current and valid
    Current,
    /// Swap file exists but may be stale
    Stale,
    /// Swap file doesn't exist
    Missing,
    /// Swap file is corrupted
    Corrupted,
}

/// Information about a swap file
#[derive(Debug, Clone)]
pub struct SwapFile {
    pub original_path: PathBuf,
    pub swap_path: PathBuf,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub status: SwapStatus,
    pub edit_count: usize,
}

/// Cursor position information
#[derive(Debug, Clone)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

/// Viewport state information
#[derive(Debug, Clone)]
pub struct ViewportState {
    pub top_line: usize,
    pub visible_lines: usize,
    pub horizontal_offset: usize,
}

/// Swap file content structure
#[derive(Debug, Clone)]
pub struct SwapContent {
    pub content: String,
    pub original_path: Option<PathBuf>,
    pub edit_count: usize,
    pub cursor_position: Option<CursorPosition>,
    pub viewport_state: Option<ViewportState>,
    pub timestamp: u64,
}

/// Swap manager for handling swap files and crash recovery
pub struct SwapManager {
    config: SwapConfig,
    active_swaps: HashMap<PathBuf, SwapContent>,
    last_save: HashMap<PathBuf, Instant>,
    edit_counts: HashMap<PathBuf, usize>,
    is_running: Arc<AtomicBool>,
}

impl SwapManager {
    pub fn new(config: SwapConfig) -> SwapResult<Self> {
        // Create swap directories
        fs::create_dir_all(&config.swap_dir)?;
        fs::create_dir_all(&config.draft_dir)?;

        Ok(Self {
            config,
            active_swaps: HashMap::new(),
            last_save: HashMap::new(),
            edit_counts: HashMap::new(),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Start the periodic swap writer
    pub fn start_periodic_save(&self) -> SwapResult<()> {
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        thread::spawn(move || {
            while is_running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));

                // Clean old swap files
                if let Err(e) = Self::cleanup_old_swaps(&config) {
                    eprintln!("Swap cleanup error: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Stop the periodic swap writer
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Register a file for swap management
    pub fn register_file(&mut self, file_path: &Path, initial_content: &str) -> SwapResult<()> {
        let swap_content = SwapContent {
            content: initial_content.to_string(),
            original_path: Some(file_path.to_path_buf()),
            edit_count: 0,
            cursor_position: None,
            viewport_state: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        self.active_swaps
            .insert(file_path.to_path_buf(), swap_content);
        self.edit_counts.insert(file_path.to_path_buf(), 0);
        self.last_save
            .insert(file_path.to_path_buf(), Instant::now());

        Ok(())
    }

    /// Update file content and check if swap should be saved
    pub fn update_content(
        &mut self,
        file_path: &Path,
        new_content: &str,
        cursor: Option<CursorPosition>,
        viewport: Option<ViewportState>,
    ) -> SwapResult<bool> {
        let edit_count = self.edit_counts.entry(file_path.to_path_buf()).or_insert(0);
        *edit_count += 1;

        if let Some(swap_content) = self.active_swaps.get_mut(file_path) {
            swap_content.content = new_content.to_string();
            swap_content.edit_count = *edit_count;

            if self.config.save_cursor {
                swap_content.cursor_position = cursor;
            }

            if self.config.save_viewport {
                swap_content.viewport_state = viewport;
            }

            swap_content.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        // Check if we should save swap
        let should_save = self.should_save_swap(file_path);
        if should_save {
            self.save_swap(file_path)?;
            self.last_save
                .insert(file_path.to_path_buf(), Instant::now());
        }

        Ok(should_save)
    }

    /// Check if swap should be saved based on edits or idle time
    fn should_save_swap(&self, file_path: &Path) -> bool {
        let edit_count = self.edit_counts.get(file_path).unwrap_or(&0);
        let last_save = self.last_save.get(file_path);

        // Save if edit threshold reached
        if *edit_count >= self.config.edits_threshold {
            return true;
        }

        // Save if idle timeout reached
        if let Some(last_save_time) = last_save {
            if last_save_time.elapsed() >= self.config.idle_timeout {
                return true;
            }
        }

        false
    }

    /// Save swap file for the given path
    pub fn save_swap(&mut self, file_path: &Path) -> SwapResult<()> {
        if let Some(swap_content) = self.active_swaps.get(file_path) {
            let swap_path = self.get_swap_path(file_path)?;
            let serialized = self.serialize_swap_content(swap_content)?;

            // Write to temporary file first, then rename for atomicity
            let temp_path = swap_path.with_extension("tmp");
            fs::write(&temp_path, serialized)?;
            fs::rename(&temp_path, &swap_path)?;

            // Reset edit count after successful save
            if let Some(edit_count) = self.edit_counts.get_mut(file_path) {
                *edit_count = 0;
            }
        }

        Ok(())
    }

    /// Check if swap file exists for the given path
    pub fn has_swap(&self, file_path: &Path) -> SwapResult<bool> {
        let swap_path = self.get_swap_path(file_path)?;
        Ok(swap_path.exists())
    }

    /// Get swap file information
    pub fn get_swap_info(&self, file_path: &Path) -> SwapResult<Option<SwapFile>> {
        let swap_path = self.get_swap_path(file_path)?;

        if !swap_path.exists() {
            return Ok(None);
        }

        let metadata = fs::metadata(&swap_path)?;
        let created = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        // Check if swap is current by reading it
        let status = match self.read_swap(&swap_path) {
            Ok(_) => SwapStatus::Current,
            Err(_) => SwapStatus::Corrupted,
        };

        let edit_count = self.edit_counts.get(file_path).copied().unwrap_or(0);

        Ok(Some(SwapFile {
            original_path: file_path.to_path_buf(),
            swap_path,
            created,
            modified,
            status,
            edit_count,
        }))
    }

    /// Read swap content from file
    pub fn read_swap(&self, swap_path: &Path) -> SwapResult<SwapContent> {
        let content = fs::read_to_string(swap_path)?;
        let swap_content = self.deserialize_swap_content(&content)?;
        Ok(swap_content)
    }

    /// Recover from swap file
    pub fn recover_swap(&self, file_path: &Path) -> SwapResult<SwapContent> {
        let swap_path = self.get_swap_path(file_path)?;
        self.read_swap(&swap_path)
    }

    /// Delete swap file
    pub fn delete_swap(&mut self, file_path: &Path) -> SwapResult<()> {
        let swap_path = self.get_swap_path(file_path)?;
        if swap_path.exists() {
            fs::remove_file(&swap_path)?;
        }
        self.active_swaps.remove(file_path);
        self.edit_counts.remove(file_path);
        self.last_save.remove(file_path);
        Ok(())
    }

    /// Get the swap file path for a given file
    fn get_swap_path(&self, file_path: &Path) -> SwapResult<PathBuf> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SwapError::PathError("Invalid file name".to_string()))?;

        let swap_name = format!(".~{}", file_name);
        Ok(self.config.swap_dir.join(swap_name))
    }

    /// Serialize swap content to a simple text format
    fn serialize_swap_content(&self, content: &SwapContent) -> SwapResult<String> {
        let mut result = String::new();

        result.push_str(&format!("timestamp={}\n", content.timestamp));
        result.push_str(&format!("edit_count={}\n", content.edit_count));

        if let Some(path) = &content.original_path {
            result.push_str(&format!("path={}\n", path.display()));
        } else {
            result.push_str("path=\n");
        }

        if let Some(cursor) = &content.cursor_position {
            result.push_str(&format!(
                "cursor={},{},{}\n",
                cursor.line, cursor.column, cursor.offset
            ));
        } else {
            result.push_str("cursor=\n");
        }

        if let Some(viewport) = &content.viewport_state {
            result.push_str(&format!(
                "viewport={},{},{}\n",
                viewport.top_line, viewport.visible_lines, viewport.horizontal_offset
            ));
        } else {
            result.push_str("viewport=\n");
        }

        result.push_str("---CONTENT---\n");
        result.push_str(&content.content);

        Ok(result)
    }

    /// Deserialize swap content from text format
    fn deserialize_swap_content(&self, data: &str) -> SwapResult<SwapContent> {
        let mut lines = data.lines();
        let mut timestamp = 0u64;
        let mut edit_count = 0usize;
        let mut original_path: Option<PathBuf> = None;
        let mut cursor_position: Option<CursorPosition> = None;
        let mut viewport_state: Option<ViewportState> = None;
        let mut content = String::new();

        while let Some(line) = lines.next() {
            if line == "---CONTENT---" {
                // Collect remaining lines as content
                let remaining: Vec<&str> = lines.collect();
                content = remaining.join("\n");
                break;
            } else if line.starts_with("timestamp=") {
                timestamp = line[11..].parse().unwrap_or(0);
            } else if line.starts_with("edit_count=") {
                edit_count = line[12..].parse().unwrap_or(0);
            } else if line.starts_with("path=") {
                let path_str = &line[5..];
                if !path_str.is_empty() {
                    original_path = Some(PathBuf::from(path_str));
                }
            } else if line.starts_with("cursor=") {
                let cursor_str = &line[7..];
                if !cursor_str.is_empty() {
                    let parts: Vec<&str> = cursor_str.split(',').collect();
                    if parts.len() == 3 {
                        if let (Ok(line), Ok(column), Ok(offset)) =
                            (parts[0].parse(), parts[1].parse(), parts[2].parse())
                        {
                            cursor_position = Some(CursorPosition {
                                line,
                                column,
                                offset,
                            });
                        }
                    }
                }
            } else if line.starts_with("viewport=") {
                let viewport_str = &line[9..];
                if !viewport_str.is_empty() {
                    let parts: Vec<&str> = viewport_str.split(',').collect();
                    if parts.len() == 3 {
                        if let (Ok(top_line), Ok(visible_lines), Ok(horizontal_offset)) =
                            (parts[0].parse(), parts[1].parse(), parts[2].parse())
                        {
                            viewport_state = Some(ViewportState {
                                top_line,
                                visible_lines,
                                horizontal_offset,
                            });
                        }
                    }
                }
            }
        }

        Ok(SwapContent {
            content,
            original_path,
            edit_count,
            cursor_position,
            viewport_state,
            timestamp,
        })
    }

    /// Clean up old swap files
    fn cleanup_old_swaps(config: &SwapConfig) -> SwapResult<()> {
        if !config.swap_dir.exists() {
            return Ok(());
        }

        let max_age = Duration::from_secs(config.max_age_days * 24 * 60 * 60);
        let now = SystemTime::now();

        for entry in fs::read_dir(&config.swap_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = fs::remove_file(&path); // Ignore errors
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Manager for untitled buffer drafts
pub struct DraftManager {
    config: SwapConfig,
}

impl DraftManager {
    pub fn new(config: SwapConfig) -> Self {
        Self { config }
    }

    /// Save untitled buffer as draft
    pub fn save_draft(
        &self,
        content: &str,
        cursor: Option<CursorPosition>,
        viewport: Option<ViewportState>,
    ) -> DraftResult<PathBuf> {
        // Generate UUID-like identifier
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let draft_name = format!("draft_{}.txt", timestamp);
        let draft_path = self.config.draft_dir.join(draft_name);

        let draft_content = SwapContent {
            content: content.to_string(),
            original_path: None, // Untitled buffer
            edit_count: 0,
            cursor_position: cursor,
            viewport_state: viewport,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let serialized = self.serialize_swap_content(&draft_content)?;
        fs::write(&draft_path, serialized)?;
        Ok(draft_path)
    }

    /// List available drafts
    pub fn list_drafts(&self) -> DraftResult<Vec<(PathBuf, SwapContent)>> {
        if !self.config.draft_dir.exists() {
            return Ok(Vec::new());
        }

        let mut drafts = Vec::new();

        for entry in fs::read_dir(&self.config.draft_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("txt") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(draft_content) = self.deserialize_swap_content(&content) {
                        drafts.push((path, draft_content));
                    }
                }
            }
        }

        // Sort by timestamp (most recent first)
        drafts.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

        Ok(drafts)
    }

    /// Load draft content
    pub fn load_draft(&self, draft_path: &Path) -> DraftResult<SwapContent> {
        let content = fs::read_to_string(draft_path)?;
        let draft_content = self.deserialize_swap_content(&content)?;
        Ok(draft_content)
    }

    /// Serialize swap content to a simple text format
    fn serialize_swap_content(&self, content: &SwapContent) -> DraftResult<String> {
        let mut result = String::new();

        result.push_str(&format!("timestamp={}\n", content.timestamp));
        result.push_str(&format!("edit_count={}\n", content.edit_count));

        if let Some(path) = &content.original_path {
            result.push_str(&format!("path={}\n", path.display()));
        } else {
            result.push_str("path=\n");
        }

        if let Some(cursor) = &content.cursor_position {
            result.push_str(&format!(
                "cursor={},{},{}\n",
                cursor.line, cursor.column, cursor.offset
            ));
        } else {
            result.push_str("cursor=\n");
        }

        if let Some(viewport) = &content.viewport_state {
            result.push_str(&format!(
                "viewport={},{},{}\n",
                viewport.top_line, viewport.visible_lines, viewport.horizontal_offset
            ));
        } else {
            result.push_str("viewport=\n");
        }

        result.push_str("---CONTENT---\n");
        result.push_str(&content.content);

        Ok(result)
    }

    /// Deserialize swap content from text format
    fn deserialize_swap_content(&self, data: &str) -> DraftResult<SwapContent> {
        let mut lines = data.lines();
        let mut timestamp = 0u64;
        let mut edit_count = 0usize;
        let mut original_path: Option<PathBuf> = None;
        let mut cursor_position: Option<CursorPosition> = None;
        let mut viewport_state: Option<ViewportState> = None;
        let mut content = String::new();

        while let Some(line) = lines.next() {
            if line == "---CONTENT---" {
                // Collect remaining lines as content
                let remaining: Vec<&str> = lines.collect();
                content = remaining.join("\n");
                break;
            } else if line.starts_with("timestamp=") {
                timestamp = line[11..].parse().unwrap_or(0);
            } else if line.starts_with("edit_count=") {
                edit_count = line[12..].parse().unwrap_or(0);
            } else if line.starts_with("path=") {
                let path_str = &line[5..];
                if !path_str.is_empty() {
                    original_path = Some(PathBuf::from(path_str));
                }
            } else if line.starts_with("cursor=") {
                let cursor_str = &line[7..];
                if !cursor_str.is_empty() {
                    let parts: Vec<&str> = cursor_str.split(',').collect();
                    if parts.len() == 3 {
                        if let (Ok(line), Ok(column), Ok(offset)) =
                            (parts[0].parse(), parts[1].parse(), parts[2].parse())
                        {
                            cursor_position = Some(CursorPosition {
                                line,
                                column,
                                offset,
                            });
                        }
                    }
                }
            } else if line.starts_with("viewport=") {
                let viewport_str = &line[9..];
                if !viewport_str.is_empty() {
                    let parts: Vec<&str> = viewport_str.split(',').collect();
                    if parts.len() == 3 {
                        if let (Ok(top_line), Ok(visible_lines), Ok(horizontal_offset)) =
                            (parts[0].parse(), parts[1].parse(), parts[2].parse())
                        {
                            viewport_state = Some(ViewportState {
                                top_line,
                                visible_lines,
                                horizontal_offset,
                            });
                        }
                    }
                }
            }
        }

        Ok(SwapContent {
            content,
            original_path,
            edit_count,
            cursor_position,
            viewport_state,
            timestamp,
        })
    }

    /// Delete draft
    pub fn delete_draft(&self, draft_path: &Path) -> DraftResult<()> {
        if draft_path.exists() {
            fs::remove_file(draft_path)?;
        }
        Ok(())
    }

    /// Clean old drafts (older than max_age_days)
    pub fn cleanup_old_drafts(&self) -> DraftResult<()> {
        if !self.config.draft_dir.exists() {
            return Ok(());
        }

        let max_age = Duration::from_secs(self.config.max_age_days * 24 * 60 * 60);
        let now = SystemTime::now();

        for entry in fs::read_dir(&self.config.draft_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = fs::remove_file(&path); // Ignore errors
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_config() -> SwapConfig {
        let temp_dir = env::temp_dir().join("niv_swap_test");
        SwapConfig {
            swap_dir: temp_dir.clone(),
            draft_dir: temp_dir.join("drafts"),
            edits_threshold: 5,
            idle_timeout: Duration::from_millis(100),
            save_cursor: true,
            save_viewport: true,
            max_age_days: 1,
        }
    }

    #[test]
    fn test_swap_manager_creation() {
        let config = test_config();
        let manager = SwapManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_draft_manager_creation() {
        let config = test_config();
        let _manager = DraftManager::new(config);
        // DraftManager has no fallible operations in constructor
    }
}
