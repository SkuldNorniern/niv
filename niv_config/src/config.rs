use crate::error::{ConfigError, ConfigResult};
use crate::settings::EditorSettings;
use crate::ui::UiSettings;
use crate::keybindings::KeyBindingConfig;
use crate::extensions::ExtensionManagerConfig;
use crate::toml_parser::{TomlParser, TomlValue};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

/// Main configuration structure for niv editor
#[derive(Debug, Clone)]
pub struct Config {
    /// Editor behavior settings
    pub editor: EditorSettings,
    /// UI appearance settings
    pub ui: UiSettings,
    /// Keybinding configuration
    pub keybindings: KeyBindingConfig,
    /// Extension/plugin configuration
    pub extensions: ExtensionManagerConfig,
    /// Custom configuration values
    pub custom: HashMap<String, TomlValue>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: EditorSettings::default(),
            ui: UiSettings::default(),
            keybindings: KeyBindingConfig::default(),
            extensions: ExtensionManagerConfig::default(),
            custom: HashMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file(path: &Path) -> ConfigResult<Self> {
        let content = fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }

    /// Load configuration from TOML string
    pub fn from_toml_str(content: &str) -> ConfigResult<Self> {
        let values = TomlParser::parse(content)?;

        Ok(Self {
            editor: EditorSettings::from_toml(&values)?,
            ui: UiSettings::from_toml(&values)?,
            keybindings: KeyBindingConfig::from_toml(&values)?,
            extensions: ExtensionManagerConfig::from_toml(&values)?,
            custom: values.into_iter()
                .filter(|(k, _)| {
                    !k.starts_with("editor.") &&
                    !k.starts_with("ui.") &&
                    !k.starts_with("keybindings.") &&
                    !k.starts_with("extensions.")
                })
                .collect(),
        })
    }

    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &Path) -> ConfigResult<()> {
        let toml_content = self.to_toml_string();
        fs::write(path, toml_content)?;
        Ok(())
    }

    /// Export configuration as TOML string
    pub fn to_toml_string(&self) -> String {
        let mut all_values = HashMap::new();

        // Merge all configuration sections
        all_values.extend(self.editor.to_toml());
        all_values.extend(self.ui.to_toml());
        all_values.extend(self.extensions.to_toml());
        all_values.extend(self.custom.clone());

        Self::format_toml(&all_values)
    }

    /// Format HashMap of TOML values as TOML string
    fn format_toml(values: &HashMap<String, TomlValue>) -> String {
        let mut output = String::new();
        let mut sections = HashMap::new();

        // Group values by section
        for (key, value) in values {
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() >= 2 {
                let section = parts[0];
                let subsection = parts[1..].join(".");
                sections.entry(section).or_insert_with(Vec::new).push((subsection, value.clone()));
            } else {
                sections.entry("").or_insert_with(Vec::new).push((key.clone(), value.clone()));
            }
        }

        // Format each section
        for (section, section_values) in sections.iter().filter(|(k, _)| !k.is_empty()) {
            output.push_str(&format!("\n[{}]\n", section));

            for (key, value) in section_values {
                let value_str = Self::format_toml_value(value);
                output.push_str(&format!("{} = {}\n", key, value_str));
            }
        }

        // Format root level values
        if let Some(root_values) = sections.get("") {
            for (key, value) in root_values {
                let value_str = Self::format_toml_value(value);
                output.push_str(&format!("{} = {}\n", key, value_str));
            }
        }

        output
    }

    fn format_toml_value(value: &TomlValue) -> String {
        match value {
            TomlValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            TomlValue::Integer(i) => format!("{}", i),
            TomlValue::Float(f) => format!("{}", f),
            TomlValue::Bool(b) => format!("{}", b),
            TomlValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(Self::format_toml_value).collect();
                format!("[{}]", items.join(", "))
            }
        }
    }

    /// Get configuration file search paths
    pub fn config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // User-specific config
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(&home).join(".niv").join("config.toml"));
            paths.push(PathBuf::from(&home).join(".config").join("niv").join("config.toml"));
        }

        // System-wide config
        paths.push(PathBuf::from("/etc/niv/config.toml"));
        paths.push(PathBuf::from("/usr/local/etc/niv/config.toml"));

        // Current directory
        if let Ok(current_dir) = std::env::current_dir() {
            paths.push(current_dir.join(".niv.toml"));
            paths.push(current_dir.join("niv.toml"));
        }

        paths
    }

    /// Load configuration with automatic path discovery
    pub fn load() -> ConfigResult<Self> {
        for path in Self::config_paths() {
            if path.exists() {
                return Self::from_file(&path);
            }
        }

        // Return default configuration if no config file found
        Ok(Self::default())
    }

    /// Load configuration with custom search paths
    pub fn load_with_paths(paths: &[PathBuf]) -> ConfigResult<Self> {
        for path in paths {
            if path.exists() {
                return Self::from_file(path);
            }
        }

        // Return default configuration if no config file found
        Ok(Self::default())
    }

    /// Create a new configuration file with default settings
    pub fn create_default_config(path: &Path) -> ConfigResult<()> {
        let config = Self::default();
        config.save_to_file(path)
    }

    /// Merge another configuration into this one
    pub fn merge(&mut self, other: &Config) {
        // For now, we'll replace each section entirely
        // In the future, this could be more granular
        self.editor = other.editor.clone();
        self.ui = other.ui.clone();
        self.keybindings = other.keybindings.clone();
        self.extensions = other.extensions.clone();

        // Merge custom values
        for (key, value) in &other.custom {
            self.custom.insert(key.clone(), value.clone());
        }
    }

    /// Get a custom configuration value
    pub fn get_custom(&self, key: &str) -> Option<&TomlValue> {
        self.custom.get(key)
    }

    /// Set a custom configuration value
    pub fn set_custom(&mut self, key: String, value: TomlValue) {
        self.custom.insert(key, value);
    }

    /// Validate the configuration
    pub fn validate(&self) -> ConfigResult<()> {
        // Validate editor settings
        if self.editor.tab_width == 0 {
            return Err(ConfigError::Validation("Tab width must be greater than 0".to_string()));
        }
        if self.editor.scrolloff > 100 {
            return Err(ConfigError::Validation("Scroll offset too large".to_string()));
        }

        // Validate UI settings
        if self.ui.transparency > 100 {
            return Err(ConfigError::Validation("Transparency must be between 0 and 100".to_string()));
        }

        // Configuration is valid
        Ok(())
    }
}

/// Configuration builder for creating custom configurations
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn editor<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut EditorSettings),
    {
        f(&mut self.config.editor);
        self
    }

    pub fn ui<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut UiSettings),
    {
        f(&mut self.config.ui);
        self
    }

    pub fn extensions<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut ExtensionManagerConfig),
    {
        f(&mut self.config.extensions);
        self
    }

    pub fn custom<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<TomlValue>,
    {
        self.config.custom.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}
