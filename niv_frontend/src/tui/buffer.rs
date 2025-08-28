use niv_rope::Rope;
use niv_config::EditorSettings;
use niv_fs::SaveContext;
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
        let lines: Vec<&str> = self.content.lines().collect();

        // If content is empty, return empty vec
        if lines.is_empty() && self.content.is_empty() {
            return vec![String::new()];
        }

        // If content has lines but split gave empty vec, it means one line with no newline
        let lines = if lines.is_empty() && !self.content.is_empty() {
            vec![self.content.as_str()]
        } else {
            lines
        };

        let start_line = self.scroll_line;
        let end_line = (start_line + self.height as usize).min(lines.len());

        let mut result_lines = Vec::new();
        for line_idx in start_line..end_line {
            let line_str = lines[line_idx];
            let start_col = self.scroll_col;
            let end_col = (start_col + self.width as usize).min(line_str.len());
            let visible_line = &line_str[start_col..end_col];
            result_lines.push(visible_line.to_string());
        }

        // If no lines were added but we have content, add at least one line
        if result_lines.is_empty() && !lines.is_empty() {
            let line_str = lines[0];
            let start_col = self.scroll_col;
            let end_col = (start_col + self.width as usize).min(line_str.len());
            let visible_line = &line_str[start_col..end_col];
            result_lines.push(visible_line.to_string());
        }

        result_lines
    }

    /// Get line numbers for display
    pub fn line_numbers(&self) -> Vec<String> {
        let lines: Vec<&str> = self.content.lines().collect();

        // Handle single line case
        let lines = if lines.is_empty() && !self.content.is_empty() {
            vec![self.content.as_str()]
        } else if lines.is_empty() {
            vec![]
        } else {
            lines
        };

        let start_line = self.scroll_line;
        let end_line = (start_line + self.height as usize).min(lines.len());

        (start_line..end_line)
            .map(|i| format!("{:>4} ", i + 1))
            .collect()
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
        let max_line = lines.len().saturating_sub(1);
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
        let lines: Vec<&str> = self.content.lines().collect();
        if self.cursor_line >= lines.len() {
            return;
        }

        let line = lines[self.cursor_line];
        let before = &line[..self.cursor_col];
        let after = &line[self.cursor_col..];

        let new_line = format!("{}{}{}", before, ch, after);

        // Rebuild content
        let mut new_content = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == self.cursor_line {
                new_content.push_str(&new_line);
            } else {
                new_content.push_str(line);
            }
            if i < lines.len() - 1 {
                new_content.push('\n');
            }
        }

        self.content = new_content;
        self.cursor_col += 1;
        self.modified = true;
        self.adjust_scroll();
    }

    /// Delete character at cursor
    pub fn delete_char(&mut self) {
        let lines: Vec<&str> = self.content.lines().collect();
        if self.cursor_line >= lines.len() {
            return;
        }

        let line = lines[self.cursor_line];
        if self.cursor_col >= line.len() {
            return;
        }

        let before = &line[..self.cursor_col];
        let after = &line[self.cursor_col + 1..];

        let new_line = format!("{}{}", before, after);

        // Rebuild content
        let mut new_content = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == self.cursor_line {
                new_content.push_str(&new_line);
            } else {
                new_content.push_str(line);
            }
            if i < lines.len() - 1 {
                new_content.push('\n');
            }
        }

        self.content = new_content;
        self.modified = true;
    }

    /// Delete character before cursor
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let lines: Vec<&str> = self.content.lines().collect();
            if self.cursor_line >= lines.len() {
                return;
            }

            let line = lines[self.cursor_line];
            let before = &line[..self.cursor_col - 1];
            let after = &line[self.cursor_col..];

            let new_line = format!("{}{}", before, after);

            // Rebuild content
            let mut new_content = String::new();
            for (i, line) in lines.iter().enumerate() {
                if i == self.cursor_line {
                    new_content.push_str(&new_line);
                } else {
                    new_content.push_str(line);
                }
                if i < lines.len() - 1 {
                    new_content.push('\n');
                }
            }

            self.content = new_content;
            self.cursor_col -= 1;
            self.modified = true;
            self.adjust_scroll();
        }
    }

    /// Insert newline at cursor
    pub fn insert_newline(&mut self) {
        let lines: Vec<&str> = self.content.lines().collect();
        if self.cursor_line >= lines.len() {
            return;
        }

        let line = lines[self.cursor_line];
        let before = &line[..self.cursor_col];
        let after = &line[self.cursor_col..];

        // Rebuild content
        let mut new_content = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == self.cursor_line {
                new_content.push_str(before);
                new_content.push('\n');
                new_content.push_str(after);
            } else {
                new_content.push_str(line);
            }
            if i < lines.len() - 1 || i == self.cursor_line {
                new_content.push('\n');
            }
        }

        self.content = new_content;
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.modified = true;
        self.adjust_scroll();
    }

    /// Get current line length
    fn current_line_length(&self) -> usize {
        let lines: Vec<&str> = self.content.lines().collect();
        if self.cursor_line < lines.len() {
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
    pub fn status(&self, config: &EditorSettings) -> String {
        let file_name = self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[No Name]".to_string());

        let modified_indicator = if self.modified { "[+]" } else { "" };
        let line_info = format!("{}:{}", self.cursor_line + 1, self.cursor_col + 1);

        format!("{} {}    {}", file_name, modified_indicator, line_info)
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
