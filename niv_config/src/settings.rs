use crate::error::ConfigResult;
use crate::toml_parser::TomlValue;
use std::collections::HashMap;

/// Editor behavior settings (vim-like options)
#[derive(Debug, Clone)]
pub struct EditorSettings {
    /// Enable line numbers
    pub line_numbers: bool,
    /// Enable relative line numbers
    pub relative_numbers: bool,
    /// Tab width in spaces
    pub tab_width: u32,
    /// Use spaces instead of tabs
    pub expand_tab: bool,
    /// Auto indent new lines
    pub auto_indent: bool,
    /// Smart indent based on syntax
    pub smart_indent: bool,
    /// Highlight current line
    pub cursor_line: bool,
    /// Show matching brackets
    pub show_match: bool,
    /// Enable syntax highlighting
    pub syntax: bool,
    /// Enable incremental search
    pub incsearch: bool,
    /// Highlight all search matches
    pub hlsearch: bool,
    /// Case insensitive search
    pub ignorecase: bool,
    /// Smart case sensitivity
    pub smartcase: bool,
    /// Enable word wrapping
    pub wrap: bool,
    /// Show line breaks
    pub line_break: bool,
    /// Scroll offset from top/bottom
    pub scrolloff: u32,
    /// Side scroll offset
    pub sidescrolloff: u32,
    /// Enable mouse support
    pub mouse: bool,
    /// Backup files before writing
    pub backup: bool,
    /// Write backup files
    pub writebackup: bool,
    /// Swap file for crash recovery
    pub swapfile: bool,
    /// Undo levels
    pub undolevels: u32,
    /// Persistent undo
    pub undofile: bool,
    /// Auto read file when changed externally
    pub autoread: bool,
    /// Auto write file when changed
    pub autowrite: bool,
    /// Confirm before quitting with unsaved changes
    pub confirm: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            line_numbers: true,
            relative_numbers: false,
            tab_width: 4,
            expand_tab: true,
            auto_indent: true,
            smart_indent: false,
            cursor_line: false,
            show_match: true,
            syntax: true,
            incsearch: true,
            hlsearch: true,
            ignorecase: false,
            smartcase: true,
            wrap: true,
            line_break: false,
            scrolloff: 5,
            sidescrolloff: 10,
            mouse: false,
            backup: false,
            writebackup: true,
            swapfile: true,
            undolevels: 1000,
            undofile: true,
            autoread: true,
            autowrite: false,
            confirm: true,
        }
    }
}

impl EditorSettings {
    /// Load settings from TOML values
    pub fn from_toml(values: &HashMap<String, TomlValue>) -> ConfigResult<Self> {
        let mut settings = Self::default();

        // Helper macro to load boolean settings
        macro_rules! load_bool {
            ($field:ident, $key:expr) => {
                if let Some(value) = values.get($key) {
                    settings.$field = value.as_bool()?;
                }
            };
        }

        // Helper macro to load integer settings
        macro_rules! load_int {
            ($field:ident, $key:expr) => {
                if let Some(value) = values.get($key) {
                    settings.$field = value.as_integer()? as u32;
                }
            };
        }

        // Load boolean settings
        load_bool!(line_numbers, "editor.line_numbers");
        load_bool!(relative_numbers, "editor.relative_numbers");
        load_bool!(expand_tab, "editor.expand_tab");
        load_bool!(auto_indent, "editor.auto_indent");
        load_bool!(smart_indent, "editor.smart_indent");
        load_bool!(cursor_line, "editor.cursor_line");
        load_bool!(show_match, "editor.show_match");
        load_bool!(syntax, "editor.syntax");
        load_bool!(incsearch, "editor.incsearch");
        load_bool!(hlsearch, "editor.hlsearch");
        load_bool!(ignorecase, "editor.ignorecase");
        load_bool!(smartcase, "editor.smartcase");
        load_bool!(wrap, "editor.wrap");
        load_bool!(line_break, "editor.line_break");
        load_bool!(mouse, "editor.mouse");
        load_bool!(backup, "editor.backup");
        load_bool!(writebackup, "editor.writebackup");
        load_bool!(swapfile, "editor.swapfile");
        load_bool!(undofile, "editor.undofile");
        load_bool!(autoread, "editor.autoread");
        load_bool!(autowrite, "editor.autowrite");
        load_bool!(confirm, "editor.confirm");

        // Load integer settings
        load_int!(tab_width, "editor.tab_width");
        load_int!(scrolloff, "editor.scrolloff");
        load_int!(sidescrolloff, "editor.sidescrolloff");
        load_int!(undolevels, "editor.undolevels");

        Ok(settings)
    }

    /// Export settings to TOML format
    pub fn to_toml(&self) -> HashMap<String, TomlValue> {
        let mut values = HashMap::new();

        // Helper macro to export boolean settings
        macro_rules! export_bool {
            ($field:ident, $key:expr) => {
                values.insert($key.to_string(), TomlValue::Bool(self.$field));
            };
        }

        // Helper macro to export integer settings
        macro_rules! export_int {
            ($field:ident, $key:expr) => {
                values.insert($key.to_string(), TomlValue::Integer(self.$field as i64));
            };
        }

        // Export boolean settings
        export_bool!(line_numbers, "editor.line_numbers");
        export_bool!(relative_numbers, "editor.relative_numbers");
        export_bool!(expand_tab, "editor.expand_tab");
        export_bool!(auto_indent, "editor.auto_indent");
        export_bool!(smart_indent, "editor.smart_indent");
        export_bool!(cursor_line, "editor.cursor_line");
        export_bool!(show_match, "editor.show_match");
        export_bool!(syntax, "editor.syntax");
        export_bool!(incsearch, "editor.incsearch");
        export_bool!(hlsearch, "editor.hlsearch");
        export_bool!(ignorecase, "editor.ignorecase");
        export_bool!(smartcase, "editor.smartcase");
        export_bool!(wrap, "editor.wrap");
        export_bool!(line_break, "editor.line_break");
        export_bool!(mouse, "editor.mouse");
        export_bool!(backup, "editor.backup");
        export_bool!(writebackup, "editor.writebackup");
        export_bool!(swapfile, "editor.swapfile");
        export_bool!(undofile, "editor.undofile");
        export_bool!(autoread, "editor.autoread");
        export_bool!(autowrite, "editor.autowrite");
        export_bool!(confirm, "editor.confirm");

        // Export integer settings
        export_int!(tab_width, "editor.tab_width");
        export_int!(scrolloff, "editor.scrolloff");
        export_int!(sidescrolloff, "editor.sidescrolloff");
        export_int!(undolevels, "editor.undolevels");

        values
    }
}

/// File type specific settings
#[derive(Debug, Clone)]
pub struct FileTypeSettings {
    /// File extensions for this type
    pub extensions: Vec<String>,
    /// Tab width for this file type
    pub tab_width: Option<u32>,
    /// Use spaces instead of tabs
    pub expand_tab: Option<bool>,
    /// Auto indent
    pub auto_indent: Option<bool>,
    /// Syntax highlighting
    pub syntax: Option<bool>,
    /// Comment string
    pub comment_string: Option<String>,
}

impl Default for FileTypeSettings {
    fn default() -> Self {
        Self {
            extensions: Vec::new(),
            tab_width: None,
            expand_tab: None,
            auto_indent: None,
            syntax: None,
            comment_string: None,
        }
    }
}
