use crate::error::{ConfigError, ConfigResult};
use std::collections::HashMap;

/// Simple TOML parser for configuration files
/// Supports basic TOML syntax needed for niv config
pub struct TomlParser;

impl TomlParser {
    /// Parse a TOML string into a HashMap
    pub fn parse(content: &str) -> ConfigResult<HashMap<String, TomlValue>> {
        let mut result = HashMap::new();
        let mut current_section = String::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Handle section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            // Parse key-value pairs
            if let Some((key, value)) = Self::parse_key_value(line) {
                let full_key = if current_section.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", current_section, key)
                };

                let toml_value = Self::parse_value(value)?;
                result.insert(full_key, toml_value);
            } else if !line.is_empty() {
                return Err(ConfigError::Toml(format!(
                    "Invalid line {}: '{}'",
                    line_num + 1,
                    line
                )));
            }
        }

        Ok(result)
    }

    fn parse_key_value(line: &str) -> Option<(&str, &str)> {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            Some((parts[0].trim(), parts[1].trim()))
        } else {
            None
        }
    }

    fn parse_value(value: &str) -> ConfigResult<TomlValue> {
        let value = value.trim();

        // String (quoted)
        if value.starts_with('"') && value.ends_with('"') {
            let content = &value[1..value.len() - 1];
            Ok(TomlValue::String(content.to_string()))
        }
        // Boolean
        else if value == "true" {
            Ok(TomlValue::Bool(true))
        } else if value == "false" {
            Ok(TomlValue::Bool(false))
        }
        // Integer
        else if let Ok(int_val) = value.parse::<i64>() {
            Ok(TomlValue::Integer(int_val))
        }
        // Float
        else if let Ok(float_val) = value.parse::<f64>() {
            Ok(TomlValue::Float(float_val))
        }
        // Array
        else if value.starts_with('[') && value.ends_with(']') {
            let content = &value[1..value.len() - 1];
            let mut array = Vec::new();

            if !content.trim().is_empty() {
                for item in content.split(',') {
                    let trimmed = item.trim();
                    if !trimmed.is_empty() {
                        array.push(Self::parse_value(trimmed)?);
                    }
                }
            }

            Ok(TomlValue::Array(array))
        }
        // Unquoted string (identifier-like)
        else if !value.is_empty() && !value.contains('"') {
            Ok(TomlValue::String(value.to_string()))
        } else {
            Err(ConfigError::Toml(format!("Unsupported value: '{}'", value)))
        }
    }
}

/// TOML value types supported by our parser
#[derive(Debug, Clone)]
pub enum TomlValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<TomlValue>),
}

impl TomlValue {
    /// Get value as string or return error
    pub fn as_string(&self) -> ConfigResult<&str> {
        match self {
            TomlValue::String(s) => Ok(s),
            _ => Err(ConfigError::Validation("Expected string value".to_string())),
        }
    }

    /// Get value as integer or return error
    pub fn as_integer(&self) -> ConfigResult<i64> {
        match self {
            TomlValue::Integer(i) => Ok(*i),
            _ => Err(ConfigError::Validation(
                "Expected integer value".to_string(),
            )),
        }
    }

    /// Get value as float or return error
    pub fn as_float(&self) -> ConfigResult<f64> {
        match self {
            TomlValue::Float(f) => Ok(*f),
            _ => Err(ConfigError::Validation("Expected float value".to_string())),
        }
    }

    /// Get value as boolean or return error
    pub fn as_bool(&self) -> ConfigResult<bool> {
        match self {
            TomlValue::Bool(b) => Ok(*b),
            _ => Err(ConfigError::Validation(
                "Expected boolean value".to_string(),
            )),
        }
    }

    /// Get value as array or return error
    pub fn as_array(&self) -> ConfigResult<&[TomlValue]> {
        match self {
            TomlValue::Array(arr) => Ok(arr),
            _ => Err(ConfigError::Validation("Expected array value".to_string())),
        }
    }
}
