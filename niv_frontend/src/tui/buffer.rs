use niv_config::EditorSettings;
use niv_fs::SaveContext;
use niv_rope::Rope;
use std::path::PathBuf;

/// Text buffer for TUI display
#[derive(Debug, Clone)]
pub struct TextBuffer {
    /// The rope data structure containing the text
    pub rope: Rope,
    /// Simple string representation for easier manipulation
    pub content: String,
    /// File path (if any)
    pub file_path: Option<PathBuf>,
    /// Save context for preserving encoding and other file properties
    pub save_context: SaveContext,
    /// Whether the buffer has unsaved changes
    pub modified: bool,
    /// Current cursor position
    pub cursor_line: usize,
    pub cursor_col: usize,
    /// Scroll position
    pub scroll_line: usize,
    pub scroll_col: usize,
    /// Buffer dimensions
    pub width: u16,
    pub height: u16,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            content: String::new(),
            file_path: None,
            save_context: SaveContext::new(),
            modified: false,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            scroll_col: 0,
            width: 80,
            height: 24,
        }
    }

    pub fn from_rope(rope: Rope) -> Self {
        // Read the rope content into a string
        let mut content = String::new();
        if rope.len() > 0 {
            let mut buf = vec![0u8; rope.len()];
            let _ = rope.read_bytes_global(0, &mut buf);
            content = String::from_utf8_lossy(&buf).to_string();
        }

        Self {
            rope,
            content,
            file_path: None,
            save_context: SaveContext::new(),
            modified: false,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            scroll_col: 0,
            width: 80,
            height: 24,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        let mut rope = Rope::new();
        let _ = rope.build_from_bytes(content.as_bytes());

        Self {
            rope,
            content: content.to_string(),
            file_path: Some(path),
            save_context: SaveContext::new(),
            modified: false,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            scroll_col: 0,
            width: 80,
            height: 24,
        }
    }

    pub fn from_file_load_result(path: PathBuf, load_result: niv_fs::FileLoadResult) -> Self {
        let mut rope = Rope::new();
        let _ = rope.build_from_bytes(load_result.content.as_bytes());

        // Create save context from load result to preserve original file properties
        let save_context = SaveContext::from_load_result(&load_result);

        Self {
            rope,
            content: load_result.content,
            file_path: Some(path),
            save_context,
            modified: false,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            scroll_col: 0,
            width: 80,
            height: 24,
        }
    }

    pub fn new_with_path(path: PathBuf) -> Self {
        Self {
            rope: Rope::new(),
            content: String::new(),
            file_path: Some(path),
            save_context: SaveContext::new(),
            modified: false,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            scroll_col: 0,
            width: 80,
            height: 24,
        }
    }

    /// Set buffer dimensions
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.adjust_scroll();
    }

    /// Save buffer to file using niv_fs
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = &self.file_path {
            niv_fs::save_file(path, &self.content, &self.save_context)?;
            Ok(())
        } else {
            Err("No file path set for buffer".into())
        }
    }

    /// Get visible lines
    pub fn visible_lines(&self) -> Vec<String> {
        let raw_lines: Vec<&str> = self.content.lines().collect();
        let has_any_content = !self.content.is_empty();

        // Treat empty buffer as a single empty line for rendering
        let lines: Vec<&str> = if raw_lines.is_empty() {
            if has_any_content { vec![self.content.as_str()] } else { vec!("") }
        } else { raw_lines };

        let start_line = self.scroll_line;
        let end_line = (start_line + self.height as usize).min(lines.len().max(1));

        let mut result_lines = Vec::new();
        for line_idx in start_line..end_line {
            let line_str = if line_idx < lines.len() { lines[line_idx] } else { "" };
            let start_col = self.scroll_col.min(line_str.len());
            let end_col = (start_col + self.width as usize).min(line_str.len());
            let visible_line = &line_str[start_col..end_col];
            result_lines.push(visible_line.to_string());
        }

        if result_lines.is_empty() { result_lines.push(String::new()); }
        result_lines
    }

    /// Get line numbers for display
    pub fn line_numbers(&self) -> Vec<String> {
        let raw_lines: Vec<&str> = self.content.lines().collect();
        let total_lines = if raw_lines.is_empty() { 1 } else { raw_lines.len() };

        let start_line = self.scroll_line;
        let visible_height = self.height as usize;
        let end_line = (start_line + visible_height).min(total_lines);

        let mut line_numbers = Vec::new();
        for i in start_line..end_line {
            line_numbers.push(format!("{:>4} ", i + 1));
        }

        // Ensure we always show at least one line number for empty buffers
        if line_numbers.is_empty() {
            line_numbers.push(format!("{:>4} ", 1));
        }

        line_numbers
    }

    /// Move cursor up
    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.adjust_cursor_to_line_length();
            self.adjust_scroll();
        }
    }

    /// Move cursor down
    pub fn move_cursor_down(&mut self) {
        let lines: Vec<&str> = self.content.lines().collect();
        let total_lines = if lines.is_empty() { 1 } else { lines.len() };
        let max_line = total_lines.saturating_sub(1);
        
        if self.cursor_line < max_line {
            self.cursor_line += 1;
            self.adjust_cursor_to_line_length();
            self.adjust_scroll();
        }
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.adjust_scroll();
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        let current_line_len = self.current_line_length();
        if self.cursor_col < current_line_len {
            self.cursor_col += 1;
            self.adjust_scroll();
        }
    }

    /// Move cursor to line start
    pub fn move_cursor_line_start(&mut self) {
        self.cursor_col = 0;
        self.adjust_scroll();
    }

    /// Move cursor to line end
    pub fn move_cursor_line_end(&mut self) {
        self.cursor_col = self.current_line_length();
        self.adjust_scroll();
    }

    /// Insert character at cursor
    pub fn insert_char(&mut self, ch: char) {
        // Work with an owned line vector, ensuring at least one line exists
        let mut lines: Vec<String> = self
            .content
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.is_empty() { lines.push(String::new()); }

        if self.cursor_line >= lines.len() { self.cursor_line = lines.len() - 1; }
        let line = &mut lines[self.cursor_line];
        if self.cursor_col > line.len() { self.cursor_col = line.len(); }
        line.insert(self.cursor_col, ch);

        self.content = lines.join("\n");
        self.cursor_col += 1;
        self.modified = true;
        self.adjust_scroll();
    }

    /// Delete character at cursor
    pub fn delete_char(&mut self) {
        let mut lines: Vec<String> = self
            .content
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.is_empty() { lines.push(String::new()); }

        if self.cursor_line >= lines.len() { return; }
        let line_len = lines[self.cursor_line].len();

        if self.cursor_col < line_len {
            // Delete within the line
            lines[self.cursor_line].remove(self.cursor_col);
        } else if self.cursor_line + 1 < lines.len() {
            // Join with next line
            let next = lines.remove(self.cursor_line + 1);
            lines[self.cursor_line].push_str(&next);
        } else {
            return;
        }

        self.content = lines.join("\n");
        self.modified = true;
    }

    /// Delete character before cursor
    pub fn backspace(&mut self) {
        let mut lines: Vec<String> = self
            .content
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.is_empty() { lines.push(String::new()); }

        if self.cursor_line >= lines.len() { return; }

        if self.cursor_col > 0 {
            // Remove character before cursor
            if self.cursor_col <= lines[self.cursor_line].len() {
                lines[self.cursor_line].remove(self.cursor_col - 1);
                self.cursor_col -= 1;
            }
        } else if self.cursor_line > 0 {
            // Merge with previous line
            let current = lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = lines[self.cursor_line].len();
            lines[self.cursor_line].push_str(&current);
            self.cursor_col = prev_len;
        } else {
            // At start of first line: nothing to do
            return;
        }

        self.content = lines.join("\n");
        self.modified = true;
        self.adjust_scroll();
    }

    /// Insert newline at cursor
    pub fn insert_newline(&mut self) {
        let mut lines: Vec<String> = self
            .content
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.is_empty() { lines.push(String::new()); }

        if self.cursor_line >= lines.len() { self.cursor_line = lines.len() - 1; }
        let current = lines[self.cursor_line].clone();
        let split_at = self.cursor_col.min(current.len());
        let before = current[..split_at].to_string();
        let after = current[split_at..].to_string();

        lines[self.cursor_line] = before;
        lines.insert(self.cursor_line + 1, after);

        self.content = lines.join("\n");
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.modified = true;
        self.adjust_scroll();
    }

    /// Get current line length
    fn current_line_length(&self) -> usize {
        let lines: Vec<&str> = self.content.lines().collect();
        if lines.is_empty() {
            // Empty buffer acts as one empty line
            if self.cursor_line == 0 { self.content.len() } else { 0 }
        } else if self.cursor_line < lines.len() {
            lines[self.cursor_line].len()
        } else {
            0
        }
    }

    /// Adjust cursor position to fit within line
    fn adjust_cursor_to_line_length(&mut self) {
        let line_len = self.current_line_length();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Adjust scroll position to keep cursor visible
    fn adjust_scroll(&mut self) {
        let cursor_screen_line = self.cursor_line.saturating_sub(self.scroll_line);
        let cursor_screen_col = self.cursor_col.saturating_sub(self.scroll_col);

        // Vertical scrolling
        if cursor_screen_line >= self.height as usize {
            self.scroll_line = self.cursor_line.saturating_sub(self.height as usize - 1);
        } else if self.cursor_line < self.scroll_line {
            self.scroll_line = self.cursor_line;
        }

        // Horizontal scrolling
        if cursor_screen_col >= self.width as usize {
            self.scroll_col = self.cursor_col.saturating_sub(self.width as usize - 1);
        } else if self.cursor_col < self.scroll_col {
            self.scroll_col = self.cursor_col;
        }
    }

    /// Get buffer status string
    pub fn status(&self, _config: &EditorSettings) -> String {
        let file_name = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[No Name]".to_string());

        let modified_indicator = if self.modified { " [+]" } else { "" };
        let line_info = format!("{}:{}", self.cursor_line + 1, self.cursor_col + 1);
        
        // Calculate total lines for display
        let lines_count = if self.content.is_empty() { 1 } else { 
            let line_count = self.content.lines().count();
            if line_count == 0 { 1 } else { line_count }
        };
        
        format!("{}{} - {}/{} lines", file_name, modified_indicator, line_info, lines_count)
    }
}

/// Buffer manager for multiple buffers
pub struct BufferManager {
    buffers: Vec<TextBuffer>,
    current_buffer: usize,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            current_buffer: 0,
        }
    }

    pub fn current(&self) -> Option<&TextBuffer> {
        self.buffers.get(self.current_buffer)
    }

    pub fn current_mut(&mut self) -> Option<&mut TextBuffer> {
        self.buffers.get_mut(self.current_buffer)
    }

    pub fn add_buffer(&mut self, buffer: TextBuffer) {
        self.buffers.push(buffer);
        self.current_buffer = self.buffers.len() - 1;
    }

    pub fn switch_buffer(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.current_buffer = index;
        }
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    pub fn close_current_buffer(&mut self) -> bool {
        if self.buffers.len() > 1 {
            self.buffers.remove(self.current_buffer);
            if self.current_buffer >= self.buffers.len() {
                self.current_buffer = self.buffers.len() - 1;
            }
            true
        } else {
            false
        }
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}
