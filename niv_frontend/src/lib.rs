pub mod tui;

pub use tui::*;

#[cfg(test)]
mod tests {
    use super::*;
    use niv_config::Config;

    #[test]
    fn test_editor_creation() {
        // Test that we can create an editor without panicking
        let _editor = Editor::new();
    }

    #[test]
    fn test_config_loading() {
        // Test that configuration loads correctly
        let config = Config::load().unwrap();
        assert!(config.editor.tab_width > 0);
        assert!(config.editor.line_numbers);
    }
}
