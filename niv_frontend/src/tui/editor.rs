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

/// Main TUI editor
pub struct Editor {
    config_loader: ConfigLoader,
    layout_manager: LayoutManager,
    theme: TerminalTheme,
    pub buffer_manager: BufferManager,
    command_line: String,
    mode: EditorMode,
    running: bool,
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
        }
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

        // Main event loop
        while self.running {
            self.draw()?;
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

        // Clear screen
        execute!(io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

        // Draw line numbers and text
        if let Some(buffer) = self.buffer_manager.current() {
            self.draw_line_numbers(buffer, &config.editor)?;
            self.draw_text_area(buffer)?;
        }

        // Draw status line
        self.draw_status_line(&config.editor)?;

        // Draw command line
        self.draw_command_line()?;

        // Position cursor
        self.position_cursor()?;

        io::stdout().flush()?;
        Ok(())
    }

    fn draw_line_numbers(&self, buffer: &TextBuffer, config: &EditorSettings) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let line_numbers = buffer.line_numbers();

        for (i, line_num) in line_numbers.iter().enumerate() {
            execute!(
                io::stdout(),
                crossterm::cursor::MoveTo(0, i as u16),
                crossterm::style::Print(
                    line_num.clone().with(self.theme.line_number())
                )
            )?;
        }

        Ok(())
    }

    fn draw_text_area(&self, buffer: &TextBuffer) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let lines = buffer.visible_lines();

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
            }
            KeyCode::Char('v') => {
                self.mode = EditorMode::Visual;
            }
            KeyCode::Char(':') => {
                self.mode = EditorMode::Command;
                self.command_line.clear();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_left();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_up();
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_right();
                }
            }
            KeyCode::Char('0') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_start();
                }
            }
            KeyCode::Char('$') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_end();
                }
            }
            KeyCode::Char('x') => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.delete_char();
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
                }
            }
            KeyCode::Enter => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.insert_newline();
                }
            }
            KeyCode::Backspace => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.backspace();
                }
            }
            KeyCode::Delete => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.delete_char();
                }
            }
            KeyCode::Tab => {
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.insert_char('\t');
                }
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
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
            }
            KeyCode::Char('d') => {
                // TODO: Implement delete (cut)
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char(ch) => {
                self.command_line.push(ch);
            }
            KeyCode::Backspace => {
                self.command_line.pop();
            }
            KeyCode::Enter => {
                self.execute_command()?;
                self.mode = EditorMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.command_line.clear();
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
