use crossterm::terminal;

/// Screen layout dimensions
#[derive(Debug, Clone)]
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub text_area_width: u16,
    pub text_area_height: u16,
    pub status_line_row: u16,
    pub line_number_width: u16,
    pub text_start_col: u16,
}

impl Layout {
    pub fn new(width: u16, height: u16) -> Self {
        let line_number_width = 5; // " 123 "
        let text_start_col = line_number_width;
        let text_area_width = width.saturating_sub(text_start_col);
        let text_area_height = height.saturating_sub(2); // -1 for status line, -1 for command line
        let status_line_row = height.saturating_sub(2);

        Self {
            width,
            height,
            text_area_width,
            text_area_height,
            status_line_row,
            line_number_width,
            text_start_col,
        }
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        *self = Self::new(width, height);
    }

    /// Get the row for command line
    pub fn command_line_row(&self) -> u16 {
        self.height.saturating_sub(1)
    }

    /// Check if a position is within the text area
    pub fn is_in_text_area(&self, col: u16, row: u16) -> bool {
        col >= self.text_start_col
            && col < self.width
            && row < self.text_area_height
    }

    /// Convert screen coordinates to text buffer coordinates
    pub fn screen_to_buffer(&self, screen_col: u16, screen_row: u16) -> (u16, u16) {
        let buffer_col = screen_col.saturating_sub(self.text_start_col);
        let buffer_row = screen_row;
        (buffer_col, buffer_row)
    }

    /// Convert text buffer coordinates to screen coordinates
    pub fn buffer_to_screen(&self, buffer_col: u16, buffer_row: u16) -> (u16, u16) {
        let screen_col = buffer_col + self.text_start_col;
        let screen_row = buffer_row;
        (screen_col, screen_row)
    }

    /// Get the rectangle for the text area
    pub fn text_area_rect(&self) -> Rect {
        Rect {
            x: self.text_start_col,
            y: 0,
            width: self.text_area_width,
            height: self.text_area_height,
        }
    }

    /// Get the rectangle for line numbers
    pub fn line_number_rect(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: self.line_number_width,
            height: self.text_area_height,
        }
    }

    /// Get the rectangle for status line
    pub fn status_line_rect(&self) -> Rect {
        Rect {
            x: 0,
            y: self.status_line_row,
            width: self.width,
            height: 1,
        }
    }

    /// Get the rectangle for command line
    pub fn command_line_rect(&self) -> Rect {
        Rect {
            x: 0,
            y: self.command_line_row(),
            width: self.width,
            height: 1,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

/// Rectangle for layout positioning
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width &&
        y >= self.y && y < self.y + self.height
    }

    pub fn right(&self) -> u16 {
        self.x + self.width
    }

    pub fn bottom(&self) -> u16 {
        self.y + self.height
    }
}

/// Screen position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    pub fn origin() -> Self {
        Self::new(0, 0)
    }
}

/// Layout manager for the TUI
pub struct LayoutManager {
    layout: Layout,
}

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            layout: Layout::default(),
        }
    }

    pub fn get_layout(&self) -> &Layout {
        &self.layout
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        self.layout.update_size(width, height);
    }

    /// Get terminal size and update layout
    pub fn update_from_terminal(&mut self) -> std::io::Result<()> {
        let (width, height) = terminal::size()?;
        self.update_size(width, height);
        Ok(())
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Viewport state for scrolling
#[derive(Debug, Clone)]
pub struct Viewport {
    pub top_line: usize,
    pub left_col: usize,
    pub width: usize,
    pub height: usize,
}

impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            top_line: 0,
            left_col: 0,
            width,
            height,
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.top_line += lines;
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.top_line = self.top_line.saturating_sub(lines);
    }

    pub fn scroll_right(&mut self, cols: usize) {
        self.left_col += cols;
    }

    pub fn scroll_left(&mut self, cols: usize) {
        self.left_col = self.left_col.saturating_sub(cols);
    }

    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.top_line && line < self.top_line + self.height
    }

    pub fn contains_col(&self, col: usize) -> bool {
        col >= self.left_col && col < self.left_col + self.width
    }

    pub fn line_to_screen(&self, line: usize) -> Option<usize> {
        if self.contains_line(line) {
            Some(line - self.top_line)
        } else {
            None
        }
    }

    pub fn col_to_screen(&self, col: usize) -> Option<usize> {
        if self.contains_col(col) {
            Some(col - self.left_col)
        } else {
            None
        }
    }
}
