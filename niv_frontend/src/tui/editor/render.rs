use super::{Editor, EditorMode};
use crate::tui::buffer::*;
use crossterm::{execute, style::Stylize};
use niv_config::EditorSettings;
use std::io::{self, Write};

/// Rendering state to track what needs to be redrawn
#[derive(Debug, Clone)]
pub struct RenderState {
    pub full_redraw: bool,
    pub text_area_dirty: bool,
    pub dirty_text_lines: Option<std::collections::HashSet<usize>>, // None = all lines
    pub line_numbers_dirty: bool,
    pub dirty_line_numbers: Option<std::collections::HashSet<usize>>, // None = all lines
    pub status_line_dirty: bool,
    pub command_line_dirty: bool,
    pub cursor_dirty: bool,
    pub last_cursor_x: u16,
    pub last_cursor_y: u16,
    pub last_scroll_line: usize,
    pub last_scroll_col: usize,
    pub last_content_hash: u64,
    pub last_cursor_line: usize,
    pub last_cursor_col: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            full_redraw: true,
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
    pub fn init_from_buffer(&mut self, buffer: &TextBuffer) {
        self.last_content_hash = super::Editor::simple_hash_static(&buffer.content);
        self.last_scroll_line = buffer.scroll_line;
        self.last_scroll_col = buffer.scroll_col;
        self.last_cursor_line = buffer.cursor_line;
        self.last_cursor_col = buffer.cursor_col;
    }

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

    pub fn mark_text_dirty(&mut self) {
        self.text_area_dirty = true;
        self.dirty_text_lines = None;
        self.line_numbers_dirty = true;
        self.dirty_line_numbers = None;
        self.cursor_dirty = true;
    }

    pub fn mark_text_lines_dirty(&mut self, lines: std::collections::HashSet<usize>) {
        self.text_area_dirty = true;
        if let Some(ref mut dirty_lines) = self.dirty_text_lines {
            dirty_lines.extend(lines);
        } else {
            self.dirty_text_lines = Some(lines);
        }

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

    pub fn mark_line_dirty(&mut self, line_idx: usize) {
        let mut lines = std::collections::HashSet::new();
        lines.insert(line_idx);
        self.mark_text_lines_dirty(lines);
    }

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

impl Editor {
    pub(crate) fn simple_hash_static(content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn simple_hash(&self, content: &str) -> u64 {
        Self::simple_hash_static(content)
    }

    pub(crate) fn update_render_state(&mut self) {
        if let Some(buffer) = self.buffer_manager.current() {
            let current_hash = self.simple_hash(&buffer.content);
            if current_hash != self.render_state.last_content_hash {
                self.render_state.mark_text_dirty();
                self.render_state.last_content_hash = current_hash;
            }
        }

        if let Some(buffer) = self.buffer_manager.current() {
            let layout = self.layout_manager.get_layout();
            let (cursor_x, cursor_y) =
                layout.buffer_to_screen(buffer.cursor_col as u16, buffer.cursor_line as u16);
            if cursor_x != self.render_state.last_cursor_x
                || cursor_y != self.render_state.last_cursor_y
            {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_x = cursor_x;
                self.render_state.last_cursor_y = cursor_y;
            }
            if buffer.cursor_line != self.render_state.last_cursor_line {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_line = buffer.cursor_line;
            }
            if buffer.cursor_col != self.render_state.last_cursor_col
                && buffer.cursor_line == self.render_state.last_cursor_line
            {
                self.render_state.cursor_dirty = true;
                self.render_state.last_cursor_col = buffer.cursor_col;
            }
        }

        if let Some(buffer) = self.buffer_manager.current() {
            if buffer.scroll_line != self.render_state.last_scroll_line
                || buffer.scroll_col != self.render_state.last_scroll_col
            {
                self.render_state.mark_all_dirty();
                self.render_state.last_scroll_line = buffer.scroll_line;
                self.render_state.last_scroll_col = buffer.scroll_col;
            }
        }
    }

    pub(crate) fn needs_redraw(&self) -> bool {
        self.render_state.full_redraw
            || self.render_state.text_area_dirty
            || self.render_state.line_numbers_dirty
            || self.render_state.status_line_dirty
            || self.render_state.command_line_dirty
            || self.render_state.cursor_dirty
    }

    pub(crate) fn draw(&mut self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let config = self.config_loader.get_copy();

        if self.render_state.full_redraw {
            execute!(
                io::stdout(),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            )?;
            if let Some(buffer) = self.buffer_manager.current() {
                self.draw_line_numbers(buffer, &config.editor)?;
                self.draw_text_area(buffer)?;
            }
            self.draw_status_line(&config.editor)?;
            self.draw_command_line()?;
            self.position_cursor()?;
        } else {
            if self.render_state.text_area_dirty {
                self.clear_text_area()?;
                if let Some(buffer) = self.buffer_manager.current() {
                    self.draw_text_area(buffer)?;
                }
            }
            if self.render_state.line_numbers_dirty {
                self.clear_line_numbers()?;
                if let Some(buffer) = self.buffer_manager.current() {
                    self.draw_line_numbers(buffer, &config.editor)?;
                }
            }
            if self.render_state.status_line_dirty {
                self.clear_status_line()?;
                self.draw_status_line(&config.editor)?;
            }
            if self.render_state.command_line_dirty {
                self.clear_command_line()?;
                self.draw_command_line()?;
            }
            if self.render_state.cursor_dirty {
                self.position_cursor()?;
            }
        }

        io::stdout().flush()?;
        Ok(())
    }

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

    fn draw_line_numbers(
        &self,
        buffer: &TextBuffer,
        _config: &EditorSettings,
    ) -> std::io::Result<()> {
        let line_numbers = buffer.line_numbers();
        if let Some(ref dirty_nums) = self.render_state.dirty_line_numbers {
            for &line_idx in dirty_nums {
                if line_idx < line_numbers.len() {
                    let line_num = &line_numbers[line_idx];
                    execute!(
                        io::stdout(),
                        crossterm::cursor::MoveTo(0, line_idx as u16),
                        crossterm::style::Print(line_num.clone().with(self.theme.line_number()))
                    )?;
                }
            }
        } else {
            for (i, line_num) in line_numbers.iter().enumerate() {
                execute!(
                    io::stdout(),
                    crossterm::cursor::MoveTo(0, i as u16),
                    crossterm::style::Print(line_num.clone().with(self.theme.line_number()))
                )?;
            }
        }
        Ok(())
    }

    fn draw_text_area(&self, buffer: &TextBuffer) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let lines = buffer.visible_lines();
        if let Some(ref dirty_lines) = self.render_state.dirty_text_lines {
            for &line_idx in dirty_lines {
                if line_idx < lines.len() {
                    let line = &lines[line_idx];
                    let (screen_x, screen_y) = layout.buffer_to_screen(0, line_idx as u16);
                    if screen_y < layout.text_area_height {
                        execute!(
                            io::stdout(),
                            crossterm::cursor::MoveTo(screen_x, screen_y),
                            crossterm::style::Print(line.clone().with(self.theme.fg()))
                        )?;
                    }
                }
            }
        } else {
            for (i, line) in lines.iter().enumerate() {
                let (screen_x, screen_y) = layout.buffer_to_screen(0, i as u16);
                if screen_y < layout.text_area_height {
                    execute!(
                        io::stdout(),
                        crossterm::cursor::MoveTo(screen_x, screen_y),
                        crossterm::style::Print(line.clone().with(self.theme.fg()))
                    )?;
                }
            }
        }
        Ok(())
    }

    fn draw_status_line(&self, config: &EditorSettings) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let status_rect = layout.status_line_rect();
        
        // Show message if available, otherwise show buffer status
        let (status_text, text_color) = if let Some(ref message) = self.message {
            let color = match self.message_type {
                super::MessageType::Info => self.theme.info(),
                super::MessageType::Success => self.theme.fg(),
                super::MessageType::Warning => self.theme.warning(),
                super::MessageType::Error => self.theme.error(),
            };
            (message.clone(), color)
        } else if let Some(buffer) = self.buffer_manager.current() {
            let mut text = buffer.status(config);
            if text.is_empty() { text = String::from("[No Name]"); }
            (text, self.theme.status_fg())
        } else {
            (String::from("[No Name]"), self.theme.status_fg())
        };
        
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(status_rect.x, status_rect.y),
            crossterm::style::Print(
                format!("{:width$}", status_text, width = status_rect.width as usize)
                    .with(text_color)
                    .on(self.theme.status_bg())
            )
        )?;
        Ok(())
    }

    fn draw_command_line(&self) -> std::io::Result<()> {
        let layout = self.layout_manager.get_layout();
        let command_rect = layout.command_line_rect();
        
        let (prompt, prompt_color) = match self.mode {
            EditorMode::Normal => ("", self.theme.fg()),
            EditorMode::Insert => ("-- INSERT --", self.theme.info()),
            EditorMode::Visual => ("-- VISUAL --", self.theme.warning()),
            EditorMode::Command => (":", self.theme.fg()),
        };
        
        let command_text = if self.mode == EditorMode::Command {
            format!("{}{}", prompt, self.command_line)
        } else {
            prompt.to_string()
        };
        
        execute!(
            io::stdout(),
            crossterm::cursor::MoveTo(command_rect.x, command_rect.y),
            crossterm::style::Print(
                format!(
                    "{:width$}",
                    command_text,
                    width = command_rect.width as usize
                )
                .with(prompt_color)
            )
        )?;
        Ok(())
    }

    pub(crate) fn position_cursor(&self) -> std::io::Result<()> {
        if let Some(buffer) = self.buffer_manager.current() {
            let layout = self.layout_manager.get_layout();
            
            // Calculate relative position within the visible area
            let relative_col = buffer.cursor_col.saturating_sub(buffer.scroll_col);
            let relative_row = buffer.cursor_line.saturating_sub(buffer.scroll_line);
            
            // Convert to screen coordinates (accounting for line numbers)
            let screen_x = layout.line_number_width + relative_col as u16;
            let screen_y = relative_row as u16;
            
            // Only position cursor if it's within the visible text area
            if screen_y < layout.text_area_height && relative_col < layout.text_area_width as usize {
                execute!(io::stdout(), crossterm::cursor::MoveTo(screen_x, screen_y))?;
            }
        }
        Ok(())
    }
}
