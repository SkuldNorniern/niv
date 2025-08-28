use crate::error::ConfigResult;
use crate::toml_parser::TomlValue;
use std::collections::HashMap;

/// Color definition (RGB values)
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse color from hex string (e.g., "#FF0000" or "FF0000")
    pub fn from_hex(hex: &str) -> ConfigResult<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Err(crate::error::ConfigError::Validation(
                "Color must be 6 hex digits".to_string(),
            ));
        }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| {
            crate::error::ConfigError::Validation("Invalid red component".to_string())
        })?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| {
            crate::error::ConfigError::Validation("Invalid green component".to_string())
        })?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| {
            crate::error::ConfigError::Validation("Invalid blue component".to_string())
        })?;

        Ok(Self::new(r, g, b))
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// UI color scheme
#[derive(Debug, Clone)]
pub struct ColorScheme {
    /// Background color
    pub background: Color,
    /// Foreground/text color
    pub foreground: Color,
    /// Line number color
    pub line_numbers: Color,
    /// Cursor color
    pub cursor: Color,
    /// Selection background
    pub selection_bg: Color,
    /// Selection foreground
    pub selection_fg: Color,
    /// Search highlight
    pub search_highlight: Color,
    /// Syntax colors
    pub syntax: SyntaxColors,
    /// Status line background
    pub status_bg: Color,
    /// Status line foreground
    pub status_fg: Color,
    /// Error color
    pub error: Color,
    /// Warning color
    pub warning: Color,
    /// Info color
    pub info: Color,
}

/// Syntax highlighting colors
#[derive(Debug, Clone)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub string: Color,
    pub comment: Color,
    pub function: Color,
    pub variable: Color,
    pub type_name: Color,
    pub number: Color,
    pub operator: Color,
    pub preprocessor: Color,
}

/// UI layout and appearance settings
#[derive(Debug, Clone)]
pub struct UiSettings {
    /// Color scheme name
    pub color_scheme: String,
    /// Font family (for GUI)
    pub font_family: String,
    /// Font size (for GUI)
    pub font_size: u32,
    /// Terminal theme (for terminal UI)
    pub terminal_theme: TerminalTheme,
    /// Show status line
    pub status_line: bool,
    /// Show command line
    pub command_line: bool,
    /// Show tab bar
    pub tab_bar: bool,
    /// Window transparency (0-100, for GUI)
    pub transparency: u8,
    /// Show minimap
    pub minimap: bool,
    /// Show file tree
    pub file_tree: bool,
    /// Split pane settings
    pub splits: SplitSettings,
}

/// Terminal color themes
#[derive(Debug, Clone)]
pub enum TerminalTheme {
    Default,
    Dark,
    Light,
    Custom(ColorScheme),
}

/// Split pane configuration
#[derive(Debug, Clone)]
pub struct SplitSettings {
    /// Vertical split character
    pub vertical_char: char,
    /// Horizontal split character
    pub horizontal_char: char,
    /// Split border color
    pub border_color: Color,
    /// Split border style
    pub border_style: BorderStyle,
}

/// Border styles for splits
#[derive(Debug, Clone)]
pub enum BorderStyle {
    Single,
    Double,
    Rounded,
    Bold,
    Custom(String),
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            background: Color::from_hex("1E1E1E").unwrap(),
            foreground: Color::from_hex("D4D4D4").unwrap(),
            line_numbers: Color::from_hex("858585").unwrap(),
            cursor: Color::from_hex("FFFFFF").unwrap(),
            selection_bg: Color::from_hex("264F78").unwrap(),
            selection_fg: Color::from_hex("FFFFFF").unwrap(),
            search_highlight: Color::from_hex("FFD700").unwrap(),
            syntax: SyntaxColors::default(),
            status_bg: Color::from_hex("007ACC").unwrap(),
            status_fg: Color::from_hex("FFFFFF").unwrap(),
            error: Color::from_hex("F44747").unwrap(),
            warning: Color::from_hex("FFA500").unwrap(),
            info: Color::from_hex("00BFFF").unwrap(),
        }
    }
}

impl Default for SyntaxColors {
    fn default() -> Self {
        Self {
            keyword: Color::from_hex("569CD6").unwrap(),
            string: Color::from_hex("CE9178").unwrap(),
            comment: Color::from_hex("6A9955").unwrap(),
            function: Color::from_hex("DCDCAA").unwrap(),
            variable: Color::from_hex("9CDCFE").unwrap(),
            type_name: Color::from_hex("4EC9B0").unwrap(),
            number: Color::from_hex("B5CEA8").unwrap(),
            operator: Color::from_hex("D4D4D4").unwrap(),
            preprocessor: Color::from_hex("C586C0").unwrap(),
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            color_scheme: "default".to_string(),
            font_family: "monospace".to_string(),
            font_size: 12,
            terminal_theme: TerminalTheme::Dark,
            status_line: true,
            command_line: true,
            tab_bar: true,
            transparency: 100,
            minimap: false,
            file_tree: false,
            splits: SplitSettings::default(),
        }
    }
}

impl Default for SplitSettings {
    fn default() -> Self {
        Self {
            vertical_char: '|',
            horizontal_char: '-',
            border_color: Color::from_hex("555555").unwrap(),
            border_style: BorderStyle::Single,
        }
    }
}

impl UiSettings {
    /// Load UI settings from TOML values
    pub fn from_toml(values: &HashMap<String, TomlValue>) -> ConfigResult<Self> {
        let mut settings = Self::default();

        // Load basic settings
        if let Some(value) = values.get("ui.color_scheme") {
            settings.color_scheme = value.as_string()?.to_string();
        }
        if let Some(value) = values.get("ui.font_family") {
            settings.font_family = value.as_string()?.to_string();
        }
        if let Some(value) = values.get("ui.font_size") {
            settings.font_size = value.as_integer()? as u32;
        }

        // Load boolean settings
        macro_rules! load_bool {
            ($field:ident, $key:expr) => {
                if let Some(value) = values.get($key) {
                    settings.$field = value.as_bool()?;
                }
            };
        }

        load_bool!(status_line, "ui.status_line");
        load_bool!(command_line, "ui.command_line");
        load_bool!(tab_bar, "ui.tab_bar");
        load_bool!(minimap, "ui.minimap");
        load_bool!(file_tree, "ui.file_tree");

        // Load transparency
        if let Some(value) = values.get("ui.transparency") {
            settings.transparency = value.as_integer()?.clamp(0, 100) as u8;
        }

        Ok(settings)
    }

    /// Export UI settings to TOML format
    pub fn to_toml(&self) -> HashMap<String, TomlValue> {
        let mut values = HashMap::new();

        values.insert(
            "ui.color_scheme".to_string(),
            TomlValue::String(self.color_scheme.clone()),
        );
        values.insert(
            "ui.font_family".to_string(),
            TomlValue::String(self.font_family.clone()),
        );
        values.insert(
            "ui.font_size".to_string(),
            TomlValue::Integer(self.font_size as i64),
        );
        values.insert(
            "ui.transparency".to_string(),
            TomlValue::Integer(self.transparency as i64),
        );

        // Export boolean settings
        macro_rules! export_bool {
            ($field:ident, $key:expr) => {
                values.insert($key.to_string(), TomlValue::Bool(self.$field));
            };
        }

        export_bool!(status_line, "ui.status_line");
        export_bool!(command_line, "ui.command_line");
        export_bool!(tab_bar, "ui.tab_bar");
        export_bool!(minimap, "ui.minimap");
        export_bool!(file_tree, "ui.file_tree");

        values
    }
}
