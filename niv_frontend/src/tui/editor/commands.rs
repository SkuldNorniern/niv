use super::{Editor, MessageType};

impl Editor {
    pub(crate) fn execute_command(&mut self) -> std::io::Result<()> {
        let command = self.command_line.trim().to_string();
        self.command_line.clear();

        match command.as_str() {
            "q" | "quit" => {
                self.running = false;
            }
            "wq" | "x" => {
                if let Some(buffer) = self.buffer_manager.current() {
                    match buffer.save() {
                        Ok(()) => {
                            self.set_message("File saved".to_string(), MessageType::Success);
                            self.running = false;
                        }
                        Err(e) => {
                            self.set_message(format!("Save failed: {}", e), MessageType::Error);
                        }
                    }
                } else {
                    self.set_message("No buffer to save".to_string(), MessageType::Warning);
                }
            }
            "w" => {
                if let Some(buffer) = self.buffer_manager.current() {
                    match buffer.save() {
                        Ok(()) => {
                            if let Some(buffer) = self.buffer_manager.current_mut() {
                                buffer.modified = false;
                            }
                            self.set_message("File saved".to_string(), MessageType::Success);
                            self.render_state.status_line_dirty = true;
                        }
                        Err(e) => {
                            self.set_message(format!("Save failed: {}", e), MessageType::Error);
                        }
                    }
                } else {
                    self.set_message("No buffer to save".to_string(), MessageType::Warning);
                }
            }
            "q!" | "quit!" => {
                self.running = false;
            }
            cmd if cmd.starts_with("e ") => {
                self.set_message("File opening not implemented yet".to_string(), MessageType::Info);
            }
            _ => {
                if !command.is_empty() {
                    self.set_message(format!("Unknown command: {}", command), MessageType::Warning);
                }
            }
        }

        Ok(())
    }
}
