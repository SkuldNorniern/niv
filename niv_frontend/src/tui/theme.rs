use crossterm::style::{Color, Stylize};
use niv_config::{ColorScheme, SyntaxColors, UiSettings, Color as ConfigColor};

/// Terminal theme for TUI rendering
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    pub colors: ColorScheme,
    pub syntax: SyntaxColors,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            colors: ColorScheme::default(),
            syntax: SyntaxColors::default(),
        }
    }
}

impl TerminalTheme {
    /// Create theme from configuration
    pub fn from_config(ui_settings: &UiSettings) -> Self {
        Self {
            colors: ColorScheme::default(),
            syntax: SyntaxColors::default(),
        }
    }

    /// Convert hex color to crossterm Color
    pub fn hex_to_color(hex: ConfigColor) -> Color {
        Color::Rgb { r: hex.r, g: hex.g, b: hex.b }
    }

    /// Get background color
    pub fn bg(&self) -> Color {
        Self::hex_to_color(self.colors.background)
    }

    /// Get foreground color
    pub fn fg(&self) -> Color {
        Self::hex_to_color(self.colors.foreground)
    }

    /// Get line number color
    pub fn line_number(&self) -> Color {
        Self::hex_to_color(self.colors.line_numbers)
    }

    /// Get cursor color
    pub fn cursor(&self) -> Color {
        Self::hex_to_color(self.colors.cursor)
    }

    /// Get selection colors
    pub fn selection_bg(&self) -> Color {
        Self::hex_to_color(self.colors.selection_bg)
    }

    pub fn selection_fg(&self) -> Color {
        Self::hex_to_color(self.colors.selection_fg)
    }

    /// Get status bar colors
    pub fn status_bg(&self) -> Color {
        Self::hex_to_color(self.colors.status_bg)
    }

    pub fn status_fg(&self) -> Color {
        Self::hex_to_color(self.colors.status_fg)
    }

    /// Get syntax colors
    pub fn keyword(&self) -> Color {
        Self::hex_to_color(self.syntax.keyword)
    }

    pub fn string(&self) -> Color {
        Self::hex_to_color(self.syntax.string)
    }

    pub fn comment(&self) -> Color {
        Self::hex_to_color(self.syntax.comment)
    }

    pub fn function(&self) -> Color {
        Self::hex_to_color(self.syntax.function)
    }

    pub fn variable(&self) -> Color {
        Self::hex_to_color(self.syntax.variable)
    }

    pub fn number(&self) -> Color {
        Self::hex_to_color(self.syntax.number)
    }

    /// Get error/warning colors
    pub fn error(&self) -> Color {
        Self::hex_to_color(self.colors.error)
    }

    pub fn warning(&self) -> Color {
        Self::hex_to_color(self.colors.warning)
    }

    pub fn info(&self) -> Color {
        Self::hex_to_color(self.colors.info)
    }
}

/// Styled text with color information
#[derive(Debug, Clone)]
pub struct StyledText {
    pub text: String,
    pub fg_color: Option<Color>,
    pub bg_color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl StyledText {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg_color: None,
            bg_color: None,
            bold: false,
            italic: false,
            underline: false,
        }
    }

    pub fn with_fg(mut self, color: Color) -> Self {
        self.fg_color = Some(color);
        self
    }

    pub fn with_bg(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Convert to crossterm styled content
    pub fn to_styled(&self) -> crossterm::style::StyledContent<String> {
        let mut styled = self.text.clone().stylize();

        if let Some(fg) = self.fg_color {
            styled = styled.with(fg);
        }

        if let Some(bg) = self.bg_color {
            styled = styled.on(bg);
        }

        if self.bold {
            styled = styled.bold();
        }

        if self.italic {
            styled = styled.italic();
        }

        if self.underline {
            styled = styled.underlined();
        }

        styled
    }
}
