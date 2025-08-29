use super::{Editor, EditorMode};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

impl Editor {
    pub(crate) fn handle_events(&mut self) -> std::io::Result<()> {
        // Use a longer timeout when no input is expected to reduce CPU usage
        if event::poll(std::time::Duration::from_millis(50))? {
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
                    self.render_state.mark_all_dirty();
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        Ok(())
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        // Handle ESC globally for robustness
        if matches!(key_event.code, KeyCode::Esc) {
            match self.mode {
                EditorMode::Normal => {
                    // Already in normal mode, no change needed
                }
                EditorMode::Insert | EditorMode::Visual => {
                    self.mode = EditorMode::Normal;
                    self.render_state.status_line_dirty = true;
                    self.render_state.command_line_dirty = true;
                    self.clear_message();
                }
                EditorMode::Command => {
                    self.mode = EditorMode::Normal;
                    self.command_line.clear();
                    self.render_state.command_line_dirty = true;
                    self.render_state.status_line_dirty = true;
                }
            }
            return Ok(());
        }

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
            KeyCode::Char('a') => {
                // Insert after cursor
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_right();
                }
                self.mode = EditorMode::Insert;
                self.render_state.status_line_dirty = true;
                self.render_state.cursor_dirty = true;
            }
            KeyCode::Char('A') => {
                // Insert at end of line
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_end();
                }
                self.mode = EditorMode::Insert;
                self.render_state.status_line_dirty = true;
                self.render_state.cursor_dirty = true;
            }
            KeyCode::Char('o') => {
                // Insert new line below and enter insert mode
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_end();
                    buffer.insert_newline();
                }
                self.mode = EditorMode::Insert;
                self.render_state.mark_text_dirty();
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('O') => {
                // Insert new line above and enter insert mode
                if let Some(buffer) = self.buffer_manager.current_mut() {
                    buffer.move_cursor_line_start();
                    buffer.insert_newline();
                    buffer.move_cursor_up();
                }
                self.mode = EditorMode::Insert;
                self.render_state.mark_text_dirty();
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('u') => { /* TODO: undo */ }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char('q') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
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
                // Clear any message when user starts typing
                self.clear_message();
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
            _ => {}
        }
        Ok(())
    }

    fn handle_visual_mode(&mut self, key_event: KeyEvent) -> std::io::Result<()> {
        match key_event.code {
            KeyCode::Char('y') => {
                self.mode = EditorMode::Normal;
                self.render_state.status_line_dirty = true;
            }
            KeyCode::Char('d') => {
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
            _ => {}
        }
        Ok(())
    }
}
