use crate::error::ConfigResult;
use crate::toml_parser::TomlValue;
use std::collections::HashMap;

/// Extension/plugin configuration
#[derive(Debug, Clone)]
pub struct ExtensionConfig {
    /// Extension name
    pub name: String,
    /// Extension version requirement
    pub version: Option<String>,
    /// Whether the extension is enabled
    pub enabled: bool,
    /// Extension-specific settings
    pub settings: HashMap<String, TomlValue>,
    /// Path to extension (for local extensions)
    pub path: Option<String>,
    /// Extension repository URL
    pub repository: Option<String>,
}

/// Extension manager configuration
#[derive(Debug, Clone)]
pub struct ExtensionManagerConfig {
    /// Extension directories to search
    pub directories: Vec<String>,
    /// Auto-load extensions
    pub auto_load: bool,
    /// Allow network access for extension downloads
    pub allow_network: bool,
    /// Trusted extension sources
    pub trusted_sources: Vec<String>,
    /// Extension update policy
    pub update_policy: UpdatePolicy,
    /// Loaded extensions
    pub extensions: HashMap<String, ExtensionConfig>,
}

/// Extension update policy
#[derive(Debug, Clone)]
pub enum UpdatePolicy {
    /// Never update extensions
    Never,
    /// Only update to stable versions
    Stable,
    /// Update to latest versions (including pre-release)
    Latest,
    /// Ask user before updating
    Prompt,
}

impl Default for ExtensionManagerConfig {
    fn default() -> Self {
        Self {
            directories: vec![
                "~/.niv/extensions".to_string(),
                "~/.local/share/niv/extensions".to_string(),
                "/usr/local/share/niv/extensions".to_string(),
            ],
            auto_load: true,
            allow_network: true,
            trusted_sources: vec!["https://github.com/niv-editor/extensions".to_string()],
            update_policy: UpdatePolicy::Stable,
            extensions: HashMap::new(),
        }
    }
}

impl ExtensionManagerConfig {
    /// Load extension configuration from TOML values
    pub fn from_toml(values: &HashMap<String, TomlValue>) -> ConfigResult<Self> {
        let mut config = Self::default();

        // Load basic settings
        if let Some(value) = values.get("extensions.auto_load") {
            config.auto_load = value.as_bool()?;
        }
        if let Some(value) = values.get("extensions.allow_network") {
            config.allow_network = value.as_bool()?;
        }

        // Load directories
        if let Some(value) = values.get("extensions.directories") {
            if let Ok(array) = value.as_array() {
                config.directories = array
                    .iter()
                    .filter_map(|v| v.as_string().ok())
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        // Load trusted sources
        if let Some(value) = values.get("extensions.trusted_sources") {
            if let Ok(array) = value.as_array() {
                config.trusted_sources = array
                    .iter()
                    .filter_map(|v| v.as_string().ok())
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        // Load update policy
        if let Some(value) = values.get("extensions.update_policy") {
            if let Ok(policy_str) = value.as_string() {
                config.update_policy = match policy_str {
                    "never" => UpdatePolicy::Never,
                    "stable" => UpdatePolicy::Stable,
                    "latest" => UpdatePolicy::Latest,
                    "prompt" => UpdatePolicy::Prompt,
                    _ => {
                        return Err(crate::error::ConfigError::Validation(format!(
                            "Unknown update policy: {}",
                            policy_str
                        )));
                    }
                };
            }
        }

        // Load individual extensions
        let mut extensions = HashMap::new();
        for (key, value) in values {
            if let Some(ext_name) = key.strip_prefix("extensions.") {
                if !ext_name.contains('.') && matches!(value, TomlValue::Array(_)) {
                    // This is an extension array, load its settings
                    let ext_config = Self::load_extension_config(ext_name, value)?;
                    extensions.insert(ext_name.to_string(), ext_config);
                }
            }
        }

        config.extensions = extensions;
        Ok(config)
    }

    fn load_extension_config(name: &str, value: &TomlValue) -> ConfigResult<ExtensionConfig> {
        let mut ext_config = ExtensionConfig {
            name: name.to_string(),
            version: None,
            enabled: true,
            settings: HashMap::new(),
            path: None,
            repository: None,
        };

        // If it's a simple boolean, just set enabled/disabled
        if let Ok(enabled) = value.as_bool() {
            ext_config.enabled = enabled;
            return Ok(ext_config);
        }

        // If it's an array, load extension settings
        if let Ok(array) = value.as_array() {
            for item in array {
                if let Ok(table_str) = item.as_string() {
                    // Parse simple key=value format
                    if let Some((key, val)) = table_str.split_once('=') {
                        let key = key.trim();
                        let val = val.trim();

                        match key {
                            "version" => ext_config.version = Some(val.to_string()),
                            "enabled" => ext_config.enabled = val.parse().unwrap_or(true),
                            "path" => ext_config.path = Some(val.to_string()),
                            "repository" => ext_config.repository = Some(val.to_string()),
                            _ => {
                                ext_config
                                    .settings
                                    .insert(key.to_string(), TomlValue::String(val.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Ok(ext_config)
    }

    /// Export extension configuration to TOML format
    pub fn to_toml(&self) -> HashMap<String, TomlValue> {
        let mut values = HashMap::new();

        // Export basic settings
        values.insert(
            "extensions.auto_load".to_string(),
            TomlValue::Bool(self.auto_load),
        );
        values.insert(
            "extensions.allow_network".to_string(),
            TomlValue::Bool(self.allow_network),
        );

        // Export update policy
        let policy_str = match self.update_policy {
            UpdatePolicy::Never => "never",
            UpdatePolicy::Stable => "stable",
            UpdatePolicy::Latest => "latest",
            UpdatePolicy::Prompt => "prompt",
        };
        values.insert(
            "extensions.update_policy".to_string(),
            TomlValue::String(policy_str.to_string()),
        );

        // Export directories
        let dir_array = self
            .directories
            .iter()
            .map(|d| TomlValue::String(d.clone()))
            .collect();
        values.insert(
            "extensions.directories".to_string(),
            TomlValue::Array(dir_array),
        );

        // Export trusted sources
        let source_array = self
            .trusted_sources
            .iter()
            .map(|s| TomlValue::String(s.clone()))
            .collect();
        values.insert(
            "extensions.trusted_sources".to_string(),
            TomlValue::Array(source_array),
        );

        // Export extensions
        for (name, ext_config) in &self.extensions {
            let mut ext_settings = Vec::new();

            if let Some(version) = &ext_config.version {
                ext_settings.push(TomlValue::String(format!("version={}", version)));
            }
            ext_settings.push(TomlValue::String(format!("enabled={}", ext_config.enabled)));
            if let Some(path) = &ext_config.path {
                ext_settings.push(TomlValue::String(format!("path={}", path)));
            }
            if let Some(repo) = &ext_config.repository {
                ext_settings.push(TomlValue::String(format!("repository={}", repo)));
            }

            // Add custom settings
            for (key, value) in &ext_config.settings {
                if let TomlValue::String(val_str) = value {
                    ext_settings.push(TomlValue::String(format!("{}={}", key, val_str)));
                }
            }

            values.insert(
                format!("extensions.{}", name),
                TomlValue::Array(ext_settings),
            );
        }

        values
    }

    /// Enable an extension
    pub fn enable_extension(&mut self, name: &str) -> bool {
        if let Some(ext) = self.extensions.get_mut(name) {
            ext.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable an extension
    pub fn disable_extension(&mut self, name: &str) -> bool {
        if let Some(ext) = self.extensions.get_mut(name) {
            ext.enabled = false;
            true
        } else {
            false
        }
    }

    /// Add a new extension
    pub fn add_extension(&mut self, config: ExtensionConfig) {
        self.extensions.insert(config.name.clone(), config);
    }

    /// Remove an extension
    pub fn remove_extension(&mut self, name: &str) -> bool {
        self.extensions.remove(name).is_some()
    }

    /// Get all enabled extensions
    pub fn enabled_extensions(&self) -> Vec<&ExtensionConfig> {
        self.extensions.values().filter(|ext| ext.enabled).collect()
    }
}

/// Extension hook points
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtensionHook {
    /// Before file is opened
    PreOpen,
    /// After file is opened
    PostOpen,
    /// Before file is saved
    PreSave,
    /// After file is saved
    PostSave,
    /// Before buffer is modified
    PreModify,
    /// After buffer is modified
    PostModify,
    /// On editor startup
    Startup,
    /// On editor shutdown
    Shutdown,
    /// On command execution
    Command(String),
}

/// Extension capabilities
#[derive(Debug, Clone)]
pub struct ExtensionCapabilities {
    /// Supported file types
    pub file_types: Vec<String>,
    /// Supported hooks
    pub hooks: Vec<ExtensionHook>,
    /// Custom commands provided
    pub commands: Vec<String>,
    /// Syntax highlighting support
    pub syntax_support: bool,
    /// LSP support
    pub lsp_support: bool,
    /// Formatter support
    pub formatter_support: bool,
}

impl Default for ExtensionCapabilities {
    fn default() -> Self {
        Self {
            file_types: Vec::new(),
            hooks: Vec::new(),
            commands: Vec::new(),
            syntax_support: false,
            lsp_support: false,
            formatter_support: false,
        }
    }
}
