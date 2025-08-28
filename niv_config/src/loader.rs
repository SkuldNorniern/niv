use crate::config::Config;
use crate::error::{ConfigError, ConfigResult};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Configuration loader with caching and hot-reload capabilities
pub struct ConfigLoader {
    /// Current configuration
    config: Arc<RwLock<Config>>,
    /// Configuration file paths
    paths: Vec<PathBuf>,
    /// Last modification times for each path
    last_modified: Vec<Option<std::time::SystemTime>>,
    /// Auto-reload interval
    reload_interval: Duration,
    /// Last reload time
    last_reload: Instant,
}

impl ConfigLoader {
    /// Create a new configuration loader with default paths
    pub fn new() -> Self {
        Self::with_paths(Config::config_paths())
    }

    /// Create a new configuration loader with custom paths
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        let len = paths.len();
        Self {
            config: Arc::new(RwLock::new(Config::default())),
            paths,
            last_modified: vec![None; len],
            reload_interval: Duration::from_secs(1),
            last_reload: Instant::now(),
        }
    }

    /// Set auto-reload interval
    pub fn with_reload_interval(mut self, interval: Duration) -> Self {
        self.reload_interval = interval;
        self
    }

    /// Load configuration from the first available path
    pub fn load(&mut self) -> ConfigResult<()> {
        let config = Config::load_with_paths(&self.paths)?;
        *self.config.write().unwrap() = config;

        // Update modification times
        for (i, path) in self.paths.iter().enumerate() {
            if path.exists() {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        self.last_modified[i] = Some(modified);
                    }
                }
            }
        }

        self.last_reload = Instant::now();
        Ok(())
    }

    /// Get current configuration (read-only)
    pub fn get(&self) -> Arc<RwLock<Config>> {
        Arc::clone(&self.config)
    }

    /// Get a copy of the current configuration
    pub fn get_copy(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    /// Check if configuration files have been modified and reload if necessary
    pub fn check_reload(&mut self) -> ConfigResult<bool> {
        if self.last_reload.elapsed() < self.reload_interval {
            return Ok(false);
        }

        let mut needs_reload = false;

        for (i, path) in self.paths.iter().enumerate() {
            if path.exists() {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Some(last_mod) = self.last_modified[i] {
                            if modified > last_mod {
                                needs_reload = true;
                                break;
                            }
                        } else {
                            // File exists but we didn't have a modification time before
                            needs_reload = true;
                            break;
                        }
                    }
                }
            } else if self.last_modified[i].is_some() {
                // File was deleted
                needs_reload = true;
                break;
            }
        }

        if needs_reload {
            self.load()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Manually reload configuration
    pub fn reload(&mut self) -> ConfigResult<()> {
        self.load()
    }

    /// Save current configuration to the first path
    pub fn save(&self) -> ConfigResult<()> {
        if let Some(path) = self.paths.first() {
            let config = self.config.read().unwrap();
            config.save_to_file(path)?;
            Ok(())
        } else {
            Err(ConfigError::Path(
                "No configuration paths available".to_string(),
            ))
        }
    }

    /// Save configuration to a specific path
    pub fn save_to(&self, path: &PathBuf) -> ConfigResult<()> {
        let config = self.config.read().unwrap();
        config.save_to_file(path)
    }

    /// Update configuration with a function
    pub fn update<F, R>(&self, updater: F) -> ConfigResult<R>
    where
        F: FnOnce(&mut Config) -> ConfigResult<R>,
    {
        let mut config = self.config.write().unwrap();
        updater(&mut config)
    }

    /// Get configuration value with dot notation (e.g., "editor.line_numbers")
    pub fn get_value(&self, key: &str) -> Option<crate::toml_parser::TomlValue> {
        let config = self.config.read().unwrap();
        let values = self.config_to_values(&config);

        // Navigate through dot notation
        let parts: Vec<&str> = key.split('.').collect();

        if parts.len() == 1 {
            values.get(key).cloned()
        } else {
            values.get(key).cloned()
        }
    }

    /// Set configuration value with dot notation
    pub fn set_value(&self, key: &str, value: crate::toml_parser::TomlValue) -> ConfigResult<()> {
        self.update(|config| {
            // Parse the key to determine which section to update
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() < 2 {
                // Root level custom value
                config.set_custom(key.to_string(), value);
                return Ok(());
            }

            match parts[0] {
                "editor" => {
                    // For editor settings, we need to reload the entire editor config
                    // This is a simplified approach - in practice you might want more granular updates
                    let mut values = config.editor.to_toml();
                    values.insert(key.to_string(), value);
                    config.editor = crate::settings::EditorSettings::from_toml(&values)?;
                }
                "ui" => {
                    let mut values = config.ui.to_toml();
                    values.insert(key.to_string(), value);
                    config.ui = crate::ui::UiSettings::from_toml(&values)?;
                }
                "extensions" => {
                    let mut values = config.extensions.to_toml();
                    values.insert(key.to_string(), value);
                    config.extensions =
                        crate::extensions::ExtensionManagerConfig::from_toml(&values)?;
                }
                _ => {
                    // Custom value
                    config.set_custom(key.to_string(), value);
                }
            }

            Ok(())
        })
    }

    /// Get all configuration paths
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Add a new configuration path
    pub fn add_path(&mut self, path: PathBuf) {
        self.paths.push(path);
        self.last_modified.push(None);
    }

    /// Remove a configuration path
    pub fn remove_path(&mut self, index: usize) {
        if index < self.paths.len() {
            self.paths.remove(index);
            self.last_modified.remove(index);
        }
    }

    /// Helper method to convert config to values map
    fn config_to_values(
        &self,
        config: &Config,
    ) -> std::collections::HashMap<String, crate::toml_parser::TomlValue> {
        let mut values = std::collections::HashMap::new();

        // Add editor settings
        values.extend(config.editor.to_toml());
        values.extend(config.ui.to_toml());
        values.extend(config.extensions.to_toml());
        values.extend(config.custom.clone());

        values
    }
}

/// Global configuration manager
pub struct ConfigManager {
    loaders: std::collections::HashMap<String, ConfigLoader>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            loaders: std::collections::HashMap::new(),
        }
    }

    /// Register a new configuration loader
    pub fn register(&mut self, name: &str, loader: ConfigLoader) {
        self.loaders.insert(name.to_string(), loader);
    }

    /// Get a configuration loader by name
    pub fn get(&self, name: &str) -> Option<&ConfigLoader> {
        self.loaders.get(name)
    }

    /// Get a mutable configuration loader by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ConfigLoader> {
        self.loaders.get_mut(name)
    }

    /// Load all registered configurations
    pub fn load_all(&mut self) -> ConfigResult<()> {
        for loader in self.loaders.values_mut() {
            loader.load()?;
        }
        Ok(())
    }

    /// Check for reloads on all configurations
    pub fn check_all_reloads(&mut self) -> ConfigResult<bool> {
        let mut any_reloaded = false;
        for loader in self.loaders.values_mut() {
            if loader.check_reload()? {
                any_reloaded = true;
            }
        }
        Ok(any_reloaded)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
