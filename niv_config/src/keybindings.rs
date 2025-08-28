use crate::error::ConfigResult;
use crate::toml_parser::TomlValue;
use std::collections::HashMap;

/// Key modifier flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool, // Windows key, Command key, etc.
}

impl KeyModifiers {
    pub fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    pub fn alt() -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: false,
            meta: false,
        }
    }

    pub fn shift() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: true,
            meta: false,
        }
    }

    pub fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: true,
            meta: false,
        }
    }

    pub fn ctrl_alt() -> Self {
        Self {
            ctrl: true,
            alt: true,
            shift: false,
            meta: false,
        }
    }
}

/// Key code representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
}

/// Complete key combination
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub modifiers: KeyModifiers,
    pub key: KeyCode,
}

impl KeyBinding {
    pub fn new(modifiers: KeyModifiers, key: KeyCode) -> Self {
        Self { modifiers, key }
    }

    pub fn simple(key: KeyCode) -> Self {
        Self::new(KeyModifiers::none(), key)
    }

    pub fn ctrl(key: KeyCode) -> Self {
        Self::new(KeyModifiers::ctrl(), key)
    }

    pub fn alt(key: KeyCode) -> Self {
        Self::new(KeyModifiers::alt(), key)
    }

    pub fn shift(key: KeyCode) -> Self {
        Self::new(KeyModifiers::shift(), key)
    }
}

/// Editor command/action
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditorCommand {
    // Navigation
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveLineStart,
    MoveLineEnd,
    MovePageUp,
    MovePageDown,
    MoveWordNext,
    MoveWordPrev,
    MoveToLine,

    // Editing
    Insert,
    InsertLineAbove,
    InsertLineBelow,
    Delete,
    DeleteLine,
    DeleteWord,
    DeleteToEnd,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,

    // Search
    Search,
    SearchNext,
    SearchPrev,
    Replace,

    // Files
    Save,
    SaveAs,
    Open,
    New,
    Quit,
    ForceQuit,

    // Windows/Splits
    SplitVertical,
    SplitHorizontal,
    CloseSplit,
    NextSplit,
    PrevSplit,

    // Modes
    NormalMode,
    InsertMode,
    VisualMode,
    CommandMode,

    // Custom command
    Custom(String),
}

/// Keybinding configuration
#[derive(Debug, Clone)]
pub struct KeyBindingConfig {
    /// Normal mode keybindings
    pub normal: HashMap<KeyBinding, EditorCommand>,
    /// Insert mode keybindings
    pub insert: HashMap<KeyBinding, EditorCommand>,
    /// Visual mode keybindings
    pub visual: HashMap<KeyBinding, EditorCommand>,
    /// Command mode keybindings
    pub command: HashMap<KeyBinding, EditorCommand>,
    /// Global keybindings (work in all modes)
    pub global: HashMap<KeyBinding, EditorCommand>,
}

impl Default for KeyBindingConfig {
    fn default() -> Self {
        let mut config = Self {
            normal: HashMap::new(),
            insert: HashMap::new(),
            visual: HashMap::new(),
            command: HashMap::new(),
            global: HashMap::new(),
        };

        // Default vim-like keybindings
        config.setup_default_bindings();
        config
    }
}

impl KeyBindingConfig {
    fn setup_default_bindings(&mut self) {
        // Normal mode
        let normal = &mut self.normal;

        // Movement
        normal.insert(KeyBinding::simple(KeyCode::Char('h')), EditorCommand::MoveLeft);
        normal.insert(KeyBinding::simple(KeyCode::Char('j')), EditorCommand::MoveDown);
        normal.insert(KeyBinding::simple(KeyCode::Char('k')), EditorCommand::MoveUp);
        normal.insert(KeyBinding::simple(KeyCode::Char('l')), EditorCommand::MoveRight);

        normal.insert(KeyBinding::simple(KeyCode::Char('0')), EditorCommand::MoveLineStart);
        normal.insert(KeyBinding::simple(KeyCode::Char('$')), EditorCommand::MoveLineEnd);
        normal.insert(KeyBinding::ctrl(KeyCode::Char('b')), EditorCommand::MovePageUp);
        normal.insert(KeyBinding::ctrl(KeyCode::Char('f')), EditorCommand::MovePageDown);
        normal.insert(KeyBinding::simple(KeyCode::Char('w')), EditorCommand::MoveWordNext);
        normal.insert(KeyBinding::simple(KeyCode::Char('b')), EditorCommand::MoveWordPrev);

        // Editing
        normal.insert(KeyBinding::simple(KeyCode::Char('i')), EditorCommand::Insert);
        normal.insert(KeyBinding::simple(KeyCode::Char('O')), EditorCommand::InsertLineAbove);
        normal.insert(KeyBinding::simple(KeyCode::Char('o')), EditorCommand::InsertLineBelow);
        normal.insert(KeyBinding::simple(KeyCode::Char('x')), EditorCommand::Delete);
        normal.insert(KeyBinding::simple(KeyCode::Char('d')), EditorCommand::DeleteLine);
        normal.insert(KeyBinding::simple(KeyCode::Char('u')), EditorCommand::Undo);
        normal.insert(KeyBinding::ctrl(KeyCode::Char('r')), EditorCommand::Redo);

        // Search
        normal.insert(KeyBinding::simple(KeyCode::Char('/')), EditorCommand::Search);
        normal.insert(KeyBinding::simple(KeyCode::Char('n')), EditorCommand::SearchNext);
        normal.insert(KeyBinding::simple(KeyCode::Char('N')), EditorCommand::SearchPrev);

        // Files
        normal.insert(KeyBinding::ctrl(KeyCode::Char('s')), EditorCommand::Save);
        normal.insert(KeyBinding::simple(KeyCode::Char(':')), EditorCommand::CommandMode);

        // Modes
        normal.insert(KeyBinding::simple(KeyCode::Char('v')), EditorCommand::VisualMode);
        normal.insert(KeyBinding::simple(KeyCode::Escape), EditorCommand::NormalMode);

        // Global bindings
        self.global.insert(KeyBinding::ctrl(KeyCode::Char('c')), EditorCommand::Quit);
        self.global.insert(KeyBinding::ctrl(KeyCode::Char('q')), EditorCommand::ForceQuit);

        // Insert mode
        self.insert.insert(KeyBinding::simple(KeyCode::Escape), EditorCommand::NormalMode);
        self.insert.insert(KeyBinding::ctrl(KeyCode::Char('c')), EditorCommand::NormalMode);

        // Visual mode
        self.visual.insert(KeyBinding::simple(KeyCode::Char('y')), EditorCommand::Copy);
        self.visual.insert(KeyBinding::simple(KeyCode::Char('d')), EditorCommand::Cut);
        self.visual.insert(KeyBinding::simple(KeyCode::Escape), EditorCommand::NormalMode);
    }

    /// Parse keybinding from string (e.g., "Ctrl+S", "F1", "g")
    pub fn parse_keybinding(key_str: &str) -> ConfigResult<KeyBinding> {
        let parts: Vec<&str> = key_str.split('+').collect();
        let mut modifiers = KeyModifiers::none();

        let (key_part, mods) = if parts.len() > 1 {
            let key_part = parts.last().unwrap();
            let mods = &parts[..parts.len() - 1];

            for mod_str in mods {
                match mod_str.to_lowercase().as_str() {
                    "ctrl" | "control" => modifiers.ctrl = true,
                    "alt" => modifiers.alt = true,
                    "shift" => modifiers.shift = true,
                    "meta" | "super" | "win" | "cmd" => modifiers.meta = true,
                    _ => return Err(crate::error::ConfigError::Validation(
                        format!("Unknown modifier: {}", mod_str)
                    )),
                }
            }

            (*key_part, modifiers)
        } else {
            (key_str, modifiers)
        };

        let key = Self::parse_keycode(key_part)?;
        Ok(KeyBinding::new(mods, key))
    }

    fn parse_keycode(key_str: &str) -> ConfigResult<KeyCode> {
        match key_str.to_lowercase().as_str() {
            "escape" | "esc" => Ok(KeyCode::Escape),
            "enter" | "return" => Ok(KeyCode::Enter),
            "tab" => Ok(KeyCode::Tab),
            "backspace" => Ok(KeyCode::Backspace),
            "delete" | "del" => Ok(KeyCode::Delete),
            "insert" | "ins" => Ok(KeyCode::Insert),
            "home" => Ok(KeyCode::Home),
            "end" => Ok(KeyCode::End),
            "pageup" | "pgup" => Ok(KeyCode::PageUp),
            "pagedown" | "pgdown" => Ok(KeyCode::PageDown),
            "uparrow" | "up" => Ok(KeyCode::ArrowUp),
            "downarrow" | "down" => Ok(KeyCode::ArrowDown),
            "leftarrow" | "left" => Ok(KeyCode::ArrowLeft),
            "rightarrow" | "right" => Ok(KeyCode::ArrowRight),
            "space" => Ok(KeyCode::Space),
            "f1" => Ok(KeyCode::F1),
            "f2" => Ok(KeyCode::F2),
            "f3" => Ok(KeyCode::F3),
            "f4" => Ok(KeyCode::F4),
            "f5" => Ok(KeyCode::F5),
            "f6" => Ok(KeyCode::F6),
            "f7" => Ok(KeyCode::F7),
            "f8" => Ok(KeyCode::F8),
            "f9" => Ok(KeyCode::F9),
            "f10" => Ok(KeyCode::F10),
            "f11" => Ok(KeyCode::F11),
            "f12" => Ok(KeyCode::F12),
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                Ok(KeyCode::Char(ch))
            }
            _ => Err(crate::error::ConfigError::Validation(
                format!("Unknown key: {}", key_str)
            )),
        }
    }

    /// Load keybindings from TOML values
    pub fn from_toml(values: &HashMap<String, TomlValue>) -> ConfigResult<Self> {
        let mut config = Self::default();

        // Load keybindings for each mode
        Self::load_mode_bindings(&mut config.normal, values, "keybindings.normal")?;
        Self::load_mode_bindings(&mut config.insert, values, "keybindings.insert")?;
        Self::load_mode_bindings(&mut config.visual, values, "keybindings.visual")?;
        Self::load_mode_bindings(&mut config.command, values, "keybindings.command")?;
        Self::load_mode_bindings(&mut config.global, values, "keybindings.global")?;

        Ok(config)
    }

    fn load_mode_bindings(
        bindings: &mut HashMap<KeyBinding, EditorCommand>,
        values: &HashMap<String, TomlValue>,
        prefix: &str,
    ) -> ConfigResult<()> {
        for (key, value) in values {
            if let Some(binding_key) = key.strip_prefix(prefix) {
                if let Ok(command_str) = value.as_string() {
                    let keybinding = Self::parse_keybinding(binding_key)?;
                    let command = Self::parse_command(command_str)?;
                    bindings.insert(keybinding, command);
                }
            }
        }
        Ok(())
    }

    fn parse_command(command_str: &str) -> ConfigResult<EditorCommand> {
        match command_str {
            "move_up" => Ok(EditorCommand::MoveUp),
            "move_down" => Ok(EditorCommand::MoveDown),
            "move_left" => Ok(EditorCommand::MoveLeft),
            "move_right" => Ok(EditorCommand::MoveRight),
            "move_line_start" => Ok(EditorCommand::MoveLineStart),
            "move_line_end" => Ok(EditorCommand::MoveLineEnd),
            "move_page_up" => Ok(EditorCommand::MovePageUp),
            "move_page_down" => Ok(EditorCommand::MovePageDown),
            "move_word_next" => Ok(EditorCommand::MoveWordNext),
            "move_word_prev" => Ok(EditorCommand::MoveWordPrev),
            "move_to_line" => Ok(EditorCommand::MoveToLine),
            "insert" => Ok(EditorCommand::Insert),
            "insert_line_above" => Ok(EditorCommand::InsertLineAbove),
            "insert_line_below" => Ok(EditorCommand::InsertLineBelow),
            "delete" => Ok(EditorCommand::Delete),
            "delete_line" => Ok(EditorCommand::DeleteLine),
            "delete_word" => Ok(EditorCommand::DeleteWord),
            "delete_to_end" => Ok(EditorCommand::DeleteToEnd),
            "undo" => Ok(EditorCommand::Undo),
            "redo" => Ok(EditorCommand::Redo),
            "copy" => Ok(EditorCommand::Copy),
            "cut" => Ok(EditorCommand::Cut),
            "paste" => Ok(EditorCommand::Paste),
            "search" => Ok(EditorCommand::Search),
            "search_next" => Ok(EditorCommand::SearchNext),
            "search_prev" => Ok(EditorCommand::SearchPrev),
            "replace" => Ok(EditorCommand::Replace),
            "save" => Ok(EditorCommand::Save),
            "save_as" => Ok(EditorCommand::SaveAs),
            "open" => Ok(EditorCommand::Open),
            "new" => Ok(EditorCommand::New),
            "quit" => Ok(EditorCommand::Quit),
            "force_quit" => Ok(EditorCommand::ForceQuit),
            "split_vertical" => Ok(EditorCommand::SplitVertical),
            "split_horizontal" => Ok(EditorCommand::SplitHorizontal),
            "close_split" => Ok(EditorCommand::CloseSplit),
            "next_split" => Ok(EditorCommand::NextSplit),
            "prev_split" => Ok(EditorCommand::PrevSplit),
            "normal_mode" => Ok(EditorCommand::NormalMode),
            "insert_mode" => Ok(EditorCommand::InsertMode),
            "visual_mode" => Ok(EditorCommand::VisualMode),
            "command_mode" => Ok(EditorCommand::CommandMode),
            cmd if cmd.starts_with("custom:") => {
                let custom_cmd = cmd.strip_prefix("custom:").unwrap().to_string();
                Ok(EditorCommand::Custom(custom_cmd))
            }
            _ => Err(crate::error::ConfigError::Validation(
                format!("Unknown command: {}", command_str)
            )),
        }
    }
}
