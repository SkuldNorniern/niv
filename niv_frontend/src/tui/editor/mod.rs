use crate::tui::{buffer::*, layout::*, theme::*};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use niv_config::{Config, ConfigLoader};
use std::io;
use std::path::PathBuf;

mod commands;
mod input;
mod render;

use render::RenderState;

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
    /// Status/error message to display to user
    message: Option<String>,
    /// Message type for color coding
    message_type: MessageType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Info,
    Success,
    Warning,
    Error,
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
        // Config loading is required at startup; if it fails we cannot proceed.
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
            message: None,
            message_type: MessageType::Info,
        }
    }

    /// Main event/render loop
    pub fn run(&mut self) -> std::io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Clear any previous output
        execute!(
            stdout,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )?;

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

        // Initialize render-state snapshot
        if let Some(buffer) = self.buffer_manager.current() {
            self.render_state.init_from_buffer(buffer);
        }

        // Main loop
        while self.running {
            // Handle events first to avoid lag
            self.handle_events()?;
            
            // Only update render state and draw if something changed
            self.update_render_state();
            if self.needs_redraw() {
                self.draw()?;
                self.render_state.clear_dirty();
            }
        }

        // Cleanup
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;
        Ok(())
    }

    /// Open a buffer from loaded file content (using niv_fs)
    pub fn open_buffer_from_content(
        &mut self,
        path: PathBuf,
        load_result: niv_fs::FileLoadResult,
    ) -> std::io::Result<()> {
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
        self.config_loader
            .reload()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let config = self.config_loader.get_copy();
        self.theme = TerminalTheme::from_config(&config.ui);
        Ok(())
    }

    /// Set a message to display to the user
    pub fn set_message(&mut self, message: String, msg_type: MessageType) {
        self.message = Some(message);
        self.message_type = msg_type;
        self.render_state.status_line_dirty = true;
    }

    /// Clear the current message
    pub fn clear_message(&mut self) {
        if self.message.is_some() {
            self.message = None;
            self.render_state.status_line_dirty = true;
        }
    }

    // The following methods are implemented in submodules:
    // - update_render_state, needs_redraw, draw, position_cursor, clear/draw helpers (render)
    // - handle_events, handle_key_event, handle_*_mode (input)
    // - execute_command (commands)
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
