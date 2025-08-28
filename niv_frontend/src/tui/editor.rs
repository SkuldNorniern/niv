use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::Stylize,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use niv_config::{Config, ConfigLoader, EditorSettings};
use crate::tui::{buffer::*, layout::*, theme::*};
use std::io::{self, Write};
use std::path::PathBuf;

/// Rendering state to track what needs to be redrawn
#[derive(Debug, Clone)]
pub struct RenderState {
    /// Whether the entire screen needs redrawing
    pub full_redraw: bool,
    /// Whether the text area needs redrawing
    pub text_area_dirty: bool,
    /// Specific lines that need redrawing in text area (None = all lines)
    pub dirty_text_lines: Option<std::collections::HashSet<usize>>,
    /// Whether line numbers need redrawing
    pub line_numbers_dirty: bool,
    /// Specific line numbers that need redrawing (None = all lines)
    pub dirty_line_numbers: Option<std::collections::HashSet<usize>>,
    /// Whether the status line needs redrawing
    pub status_line_dirty: bool,
    /// Whether the command line needs redrawing
    pub command_line_dirty: bool,
    /// Whether the cursor position needs updating
    pub cursor_dirty: bool,
    /// Last known cursor position
    pub last_cursor_x: u16,
    pub last_cursor_y: u16,
    /// Last known scroll position
    pub last_scroll_line: usize,
    pub last_scroll_col: usize,
    /// Last known buffer content hash for change detection
    pub last_content_hash: u64,
    /// Last known cursor line and column for change detection
    pub last_cursor_line: usize,
    pub last_cursor_col: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            full_redraw: true, // Start with full redraw
            text_area_dirty: true,
            dirty_text_lines: None,
            line_numbers_dirty: true,
            dirty_line_numbers: None,
            status_line_dirty: true,
            command_line_dirty: true,
            cursor_dirty: true,
            last_cursor_x: 0,
            last_cursor_y: 0,
            last_scroll_line: 0,
            last_scroll_col: 0,
            last_content_hash: 0,
            last_cursor_line: 0,
            last_cursor_col: 0,
        }
    }
}

impl RenderState {
    /// Mark everything for redrawing
    pub fn mark_all_dirty(&mut self) {
        self.full_redraw = true;
        self.text_area_dirty = true;
        self.dirty_text_lines = None;
        self.line_numbers_dirty = true;
        self.dirty_line_numbers = None;
        self.status_line_dirty = true;
        self.command_line_dirty = true;
        self.cursor_dirty = true;
    }

    /// Mark only text area for redrawing
    pub fn mark_text_dirty(&mut self) {
        self.text_area_dirty = true;
        self.dirty_text_lines = None;
        self.line_numbers_dirty = true;
        self.dirty_line_numbers = None;
        self.cursor_dirty = true;
    }

    /// Mark specific text lines for redrawing
    pub fn mark_text_lines_dirty(&mut self, lines: std::collections::HashSet<usize>) {
        self.text_area_dirty = true;
        if let Some(ref mut dirty_lines) = self.dirty_text_lines {
            dirty_lines.extend(lines);
        } else {
            self.dirty_text_lines = Some(lines);
        }

        // Also mark corresponding line numbers as dirty
        if let Some(ref dirty_lines) = self.dirty_text_lines.clone() {
            self.line_numbers_dirty = true;
            if let Some(ref mut dirty_nums) = self.dirty_line_numbers {
                dirty_nums.extend(dirty_lines.iter().cloned());
            } else {
                self.dirty_line_numbers = Some(dirty_lines.clone());
            }
        }
        self.cursor_dirty = true;
    }

    /// Mark single line for redrawing
    pub fn mark_line_dirty(&mut self, line_idx: usize) {
        let mut lines = std::collections::HashSet::new();
        lines.insert(line_idx);
        self.mark_text_lines_dirty(lines);
    }

    /// Clear all dirty flags after successful draw
    pub fn clear_dirty(&mut self) {
        self.full_redraw = false;
        self.text_area_dirty = false;
        self.dirty_text_lines = None;
        self.line_numbers_dirty = false;
        self.dirty_line_numbers = None;
        self.status_line_dirty = false;
        self.command_line_dirty = false;
        self.cursor_dirty = false;
    }
}

/// Main TUI editor
pub struct Editor {
    config_loader: ConfigLoader,
    layout_manager: LayoutManager,
    theme: TerminalTheme,
    pub buffer_manager: BufferManager,
    command_line: String,
    mode: EditorMode,
    running: bool,
    /// Rendering state for selective updates
    render_state: RenderState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
    Command,
}

impl Editor {
    pub fn new() -> Self {
        let mut config_loader = ConfigLoader::new();
        config_loader.load().expect("Failed to load configuration");

        let config = config_loader.get_copy();
        let theme = TerminalTheme::from_config(&config.ui);

        Self {
            config_loader,
            layout_manager: LayoutManager::new(),
            theme,
            buffer_manager: BufferManager::new(),
            command_line: String::new(),
            mode: EditorMode::Normal,
            running: true,
            render_state: RenderState::default(),
        }
    }

    /// Simple hash function for content change detection
    fn simple_hash(content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Update render state based on current editor state
    fn update_render_state(&mut self) {
        // Check if buffer content has changed
        if let Some(buffer) = self.buffer_manager.current() {
            let current_hash = Self::simple_hash(&buffer.content);
            if current_hash != self.render_state.last_content_hash {
                self.render_state.mark_text_dirty();
                self.render_state.last_content_hash = current_hash;
            }
        }

        // Check if cursor position has changed
        if let Some(buffer) = self.buffer_manager.current() {
            let layout = self.layout_manager.get_layout();
            let (cursor_x, cursor_y) = layout.buffer_to_screen(buffer.cursor_col as u16, buffer.cursor_line as u16);

            // Check if cursor moved to different screen position
            if cursor_x != self.render_state.last_cursor_x || cursor_y != self.render_state.last_cursor_y {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_x = cursor_x;
                self.render_state.last_cursor_y = cursor_y;
            }

            // Check if cursor moved to different line (for partial rendering)
            if buffer.cursor_line != self.render_state.last_cursor_line {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_line = buffer.cursor_line;
            }

            // Check if cursor moved within same line (only cursor needs updating)
            if buffer.cursor_col != self.render_state.last_cursor_col && buffer.cursor_line == self.render_state.last_cursor_line {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_col = buffer.cursor_col;
            }
        }

        // Check if scroll position has changed
        if let Some(buffer) = self.buffer_manager.current() {
            if buffer.scroll_line != self.render_state.last_scroll_line ||
               buffer.scroll_col != self.render_state.last_scroll_col {
                // When scroll changes, we need to redraw everything visible
                self.render_state.mark_all_dirty();
                self.render_state.last_scroll_line = buffer.scroll_line;
                self.render_state.last_scroll_col = buffer.scroll_col;
            }
        }
    }

    /// Check if anything needs to be redrawn
    fn needs_redraw(&self) -> bool {
        self.render_state.full_redraw
            || self.render_state.text_area_dirty
            || self.render_state.line_numbers_dirty
            || self.render_state.status_line_dirty
            || self.render_state.command_line_dirty
            || self.render_state.cursor_dirty
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Clear any previous output
        execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

        // Initialize layout
        self.layout_manager.update_from_terminal()?;

        // Create a default buffer only if no buffers exist
        if self.buffer_manager.buffer_count() == 0 {
            let mut buffer = TextBuffer::new();
            buffer.set_size(
                self.layout_manager.get_layout().text_area_width,
                self.layout_manager.get_layout().text_area_height,
            );
            self.buffer_manager.add_buffer(buffer);
        } else {
            // Update existing buffer size
            if let Some(buffer) = self.buffer_manager.current_mut() {
                buffer.set_size(
                    self.layout_manager.get_layout().text_area_width,
                    self.layout_manager.get_layout().text_area_height,
                );
            }
        }

        // Initialize content hash and scroll position for first buffer if it exists
        if let Some(buffer) = self.buffer_manager.current() {
            self.render_state.last_content_hash = Self::simple_hash(&buffer.content);
            self.render_state.last_scroll_line = buffer.scroll_line;
            self.render_state.last_scroll_col = buffer.scroll_col;
            self.render_state.last_cursor_line = buffer.cursor_line;
            self.render_state.last_cursor_col = buffer.cursor_col;
        }

        // Main event loop with optimized rendering
        while self.running {
            // Update render state based on current editor state
            self.update_render_state();

            // Only draw if something has changed
            if self.needs_redraw() {
                self.draw()?;
                self.render_state.clear_dirty();
            }

            // Handle events (this is blocking, so we always do it)
            self.handle_events()?;
        }

        // Cleanup
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;

        Ok(())
    }

    fn draw(&mut self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let config = self.config_loader.get_copy();

        // Full redraw: clear entire screen and redraw everything
        if self.render_state.full_redraw {
            execute!(io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

            // Draw everything
            if let Some(buffer) = self.buffer_manager.current() {
                self.draw_line_numbers(buffer, &config.editor)?;
                self.draw_text_area(buffer)?;
            }
            self.draw_status_line(&config.editor)?;
            self.draw_command_line()?;
            self.position_cursor()?;
        } else {
            // Selective redraw: only redraw changed components

            // Clear and redraw text area if needed
            if self.render_state.text_area_dirty {
                self.clear_text_area()?;
                if let Some(buffer) = self.buffer_manager.current() {
                    self.draw_text_area(buffer)?;
                }
            }

            // Clear and redraw line numbers if needed
            if self.render_state.line_numbers_dirty {
                self.clear_line_numbers()?;
                if let Some(buffer) = self.buffer_manager.current() {
                    self.draw_line_numbers(buffer, &config.editor)?;
                }
            }

            // Clear and redraw status line if needed
            if self.render_state.status_line_dirty {
                self.clear_status_line()?;
                self.draw_status_line(&config.editor)?;
            }

            // Clear and redraw command line if needed
            if self.render_state.command_line_dirty {
                self.clear_command_line()?;
                self.draw_command_line()?;
            }

            // Update cursor position if needed
            if self.render_state.cursor_dirty {
                self.position_cursor()?;
            }
        }

        io::stdout().flush()?;
        Ok(())
    }

    /// Clear only the text area region
    fn clear_text_area(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let height = layout.text_area_height;

        for y in 0..height {
            let screen_x = layout.line_number_width;
            let screen_y = y;
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(screen_x, screen_y),
                crossterm::style::Print(" ".repeat(layout.text_area_width as usize))
            )?;
        }
        Ok(())
    }

    /// Clear only the line numbers region
    fn clear_line_numbers(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let height = layout.text_area_height;
        let width = layout.line_number_width;

        for y in 0..height {
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(0, y),
                crossterm::style::Print(" ".repeat(width as usize))
            )?;
        }
        Ok(())
    }

    /// Clear only the status line region
    fn clear_status_line(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let width = layout.width;
        let y = layout.status_line_row;

        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(0, y),
            crossterm::style::Print(" ".repeat(width as usize))
        )?;
        Ok(())
    }

    /// Clear only the command line region
    fn clear_command_line(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let width = layout.width;
        let y = layout.height - 1;

        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(0, y),
            crossterm::style::Print(" ".repeat(width as usize))
        )?;
        Ok(())
    }

    fn draw_line_numbers(&self, buffer: &TextBuffer, config: &EditorSettings) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let line_numbers = buffer.line_numbers();

        // If we have specific dirty line numbers, only redraw those
        if let Some(ref dirty_nums) = self.render_state.dirty_line_numbers {
            for &line_idx in dirty_nums {
                if line_idx < line_numbers.len() {
                    let line_num = &line_numbers[line_idx];
                    execute!(
                        io::stdout(),
                        crossterm::cursor::MoveTo(0, line_idx as u16),
                        crossterm::style::Print(
                            line_num.clone().with(self.theme.line_number())
                        )
                    )?;
                }
            }
        } else {
            // Redraw all line numbers
            for (i, line_num) in line_numbers.iter().enumerate() {
                execute!(
                    io::stdout(),
                    crossterm::cursor::MoveTo(0, i as u16),
                    crossterm::style::Print(
                        line_num.clone().with(self.theme.line_number())
                    )
                )?;
            }
        }

        Ok(())
    }

    fn draw_text_area(&self, buffer: &TextBuffer) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let lines = buffer.visible_lines();

        // If we have specific dirty lines, only redraw those
        if let Some(ref dirty_lines) = self.render_state.dirty_text_lines {
            for &line_idx in dirty_lines {
                if line_idx < lines.len() {
                    let line = &lines[line_idx];
                    let (screen_x, screen_y) = layout.buffer_to_screen(0, line_idx as u16);

                    // Only draw if within bounds
                    if screen_y < layout.text_area_height {
                        execute!(
                            io::stdout(),
                            crossterm::cursor::MoveTo(screen_x, screen_y),
                            crossterm::style::Print(
                                line.clone().with(self.theme.fg())
                            )
                        )?;
                    }
                }
            }
        } else {
            // Redraw all visible lines
            for (i, line) in lines.iter().enumerate() {
                let (screen_x, screen_y) = layout.buffer_to_screen(0, i as u16);

                // Only draw if within bounds
                if screen_y < layout.text_area_height {
                    execute!(
                        io::stdout(),
                        crossterm::cursor::MoveTo(screen_x, screen_y),
                        crossterm::style::Print(
                            line.clone().with(self.theme.fg())
                        )
                    )?;
                }
            }
        }

        Ok(())
    }

    fn draw_status_line(&self, config: &EditorSettings) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let status_rect = layout.status_line_rect();

        if let Some(buffer) = self.buffer_manager.current() {
            let status_text = buffer.status(config);

            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(status_rect.x, status_rect.y),
                crossterm::style::Print(
                    format!("{:width$}", status_text, width = status_rect.width as usize)
                        .with(self.theme.status_fg())
                        .on(self.theme.status_bg())
                )
            )?;
        }

        Ok(())
    }

    fn draw_command_line(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let command_rect = layout.command_line_rect();

        let prompt = match self.mode {
            EditorMode::Normal => "",
            EditorMode::Insert => "-- INSERT --",
            EditorMode::Visual => "-- VISUAL --",
            EditorMode::Command => ":",
        };

        let command_text = format!("{}{}", prompt, self.command_line);

        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(command_rect.x, command_rect.y),
            crossterm::style::Print(
                format!("{:width$}", command_text, width = command_rect.width as usize)
                    .with(self.theme.fg())
            )
        )?;

        Ok(())
    }

    fn position_cursor(&self) -> std::io::Result<()> {
        if let Some(buffer) = self.buffer_manager.current() {
            let layout = self.layout_manager.get_layout();

            let (screen_x, screen_y) = layout.buffer_to_screen(
                (buffer.cursor_col - buffer.scroll_col) as u16,
                (buffer.cursor_line - buffer.scroll_line) as u16,
            );

            execute!(io::stdout(), crossterm::cursor::MoveTo(screen_x, screen_y))?;
        }

        Ok(())
    }

    fn handle_events(&mut self) -> std::io::Result<()> {
        // Use poll with a short timeout to avoid blocking
        if event::poll(std::time::Duration::from_millis(10))? {
            match event::read() {
                Ok(Event::Key(key_event)) => {
                    self.handle_key_event(key_event)?;
                }
                Ok(Event::Resize(width, height)) => {
                    self.layout_manager.update_size(width, height);
                    if let Some(buffer) = self.buffer_manager.current_mut() {
                        buffer.set_size(
                            self.layout_manager.get_layout().text_area_width,
                            self.layout_manager.get_layout().text_area_height,
                        );
                    }
                    self.render_state.mark_all_dirty(); // Full redraw needed on resize
                }
                Ok(_) => {} // Other events
                Err(_) => {} // Event read error
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match self.mode {
            EditorMode::Normal => self.handle_normal_mode(key_event),
            EditorMode::Insert => self.handle_insert_mode(key_event),
            EditorMode::Visual => self.handle_visual_mode(key_event),
            EditorMode::Command => self.handle_command_mode(key_event),
        }
    }

    fn handle_normal_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char('i') => {
                self.mode = EditorMode::Insert;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('v') => {
                self.mode = EditorMode::Visual;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
                self.command_line.clear();
                self.render_state.command_line_dirty = true;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_left();
                    // Only mark cursor as dirty since we're just moving within the same view
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_down();
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_up();
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_right();
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('0') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_start();
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('$') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_end();
                    self.render_state.cursor_dirty = true;
                }
            }
            KeyCode::Char('x') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.delete_char();
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Char('u') => {
                // TODO: Implement undo
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char(ch) => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.insert_char(ch);
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Enter => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.insert_newline();
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Backspace => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.backspace();
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Delete => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.delete_char();
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Tab => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.insert_char('\t');
                    self.render_state.mark_text_dirty();
                }
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.render_state.status_line_dirty = true; // Mode changed
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_visual_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char('y') => {
                // TODO: Implement yank (copy)
                self.mode = EditorMode::Normal;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('d') => {
                // TODO: Implement delete (cut)
                self.mode = EditorMode::Normal;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.render_state.status_line_dirty = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char(ch) => {
                self.command_line.push(ch);
                self.render_state.command_line_dirty = true;
            }
            KeyCode::Backspace => {
                self.command_line.pop();
                self.render_state.command_line_dirty = true;
            }
            KeyCode::Enter => {
                self.execute_command()?;
                self.mode = EditorMode::Normal;
                self.render_state.command_line_dirty = true;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.command_line.clear();
                self.render_state.command_line_dirty = true;
                self.render_state.status_line_dirty = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_command(&mut self) -> std::io::Result<()> {
        let command = self.command_line.trim().to_string();
        self.command_line.clear();

        match command.as_str() {
            "q" | "quit" => {
                self.running = false;
            }
                          "wq" | "x" => {
                 // Save and quit
                 if let Some(buffer) = self.buffer_manager.current() {
                     match buffer.save() {
                         Ok(()) => {
                             self.running = false;
                         }
                         Err(e) => {
                             // TODO: Show error message to user
                             eprintln!("Save failed: {}", e);
                         }
                     }
                 }
              }
            "w" => {
                // Save
                if let Some(buffer) = self.buffer_manager.current() {
                    match buffer.save() {
                        Ok(()) => {
                            // Mark buffer as not modified
                            if let Some(buffer) = self.buffer_manager.current_mut() {
                                buffer.modified = false;
                            }
                            // TODO: Show success message to user
                        }
                        Err(e) => {
                            // TODO: Show error message to user
                            eprintln!("Save failed: {}", e);
                        }
                    }
                }
            }
            "q!" | "quit!" => {
                // TODO: Force quit without saving
                self.running = false;
            }
            cmd if cmd.starts_with("e ") => {
                // TODO: Open file
            }
            _ => {
                // Unknown command
            }
        }

        Ok(())
    }

    /// Open a buffer from loaded file content (using niv_fs)
    pub fn open_buffer_from_content(&mut self, path: PathBuf, load_result: niv_fs::FileLoadResult) -> std::io::Result<()> {
        let buffer = TextBuffer::from_file_load_result(path, load_result);
        self.buffer_manager.add_buffer(buffer);
        Ok(())
    }

    /// Create a new empty buffer
    pub fn create_new_buffer(&mut self, path: PathBuf) -> std::io::Result<()> {
        let buffer = TextBuffer::new_with_path(path);
        self.buffer_manager.add_buffer(buffer);
        Ok(())
    }

    /// Get current editor mode
    pub fn mode(&self) -> EditorMode {
        self.mode
    }

    /// Get current configuration
    pub fn config(&self) -> Config {
        self.config_loader.get_copy()
    }

    /// Reload configuration
    pub fn reload_config(&mut self) -> std::io::Result<()> {
        self.config_loader.reload().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let config = self.config_loader.get_copy();
        self.theme = TerminalTheme::from_config(&config.ui);
        Ok(())
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
