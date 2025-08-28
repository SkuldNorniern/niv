# niv_config

A comprehensive, extensible configuration system for the niv editor with TOML support.

## Features

- **TOML-based configuration** - Human-readable and writable configuration files
- **Vim-like editor settings** - Familiar options for editor behavior
- **Portable UI configuration** - Works with both terminal and GUI interfaces
- **Customizable keybindings** - Full keybinding customization with vim-like defaults
- **Extension/plugin support** - Comprehensive plugin management system
- **Hot-reload** - Automatic configuration reloading when files change
- **No external dependencies** - Pure Rust implementation (except tokio-level libs)
- **Type-safe** - Compile-time guarantees for configuration structure
- **Extensible** - Easy to add custom configuration sections

## Configuration Sections

### [editor] - Editor Behavior Settings
Vim-like editor options that control how text is edited and displayed:

```toml
[editor]
line_numbers = true          # Show line numbers
relative_numbers = false     # Relative line numbers
tab_width = 4               # Tab width in spaces
expand_tab = true           # Use spaces instead of tabs
auto_indent = true          # Auto indent new lines
smart_indent = true         # Smart indent based on syntax
syntax = true               # Enable syntax highlighting
incsearch = true            # Incremental search
hlsearch = true             # Highlight all search matches
wrap = true                 # Enable word wrapping
scrolloff = 5               # Scroll offset from top/bottom
mouse = false               # Enable mouse support
backup = false              # Backup files before writing
swapfile = true             # Use swap files for crash recovery
undolevels = 1000           # Number of undo levels
autoread = true             # Auto read file when changed externally
```

### [ui] - User Interface Settings
Appearance and layout settings that work across different UI implementations:

```toml
[ui]
color_scheme = "default"    # Color scheme name
font_family = "monospace"   # Font family (for GUI)
font_size = 12              # Font size (for GUI)
terminal_theme = "dark"     # Terminal theme
status_line = true          # Show status line
command_line = true         # Show command line
tab_bar = true              # Show tab bar
transparency = 100          # Window transparency (0-100)
minimap = false             # Show minimap
file_tree = false           # Show file tree
```

### [keybindings] - Custom Keybindings
Fully customizable keybindings with vim-like defaults:

```toml
[keybindings]
# Movement
normal.h = "move_left"
normal.j = "move_down"
normal.k = "move_up"
normal.l = "move_right"
normal.0 = "move_line_start"
normal.$ = "move_line_end"

# Editing
normal.i = "insert"
normal.O = "insert_line_above"
normal.o = "insert_line_below"
normal.x = "delete"
normal.u = "undo"

# Global shortcuts
global.Ctrl+s = "save"
global.Ctrl+q = "force_quit"

# Custom commands
normal.Ctrl+p = "custom:fuzzy_file_open"
normal.Ctrl+t = "custom:file_tree_toggle"
```

### [extensions] - Plugin/Extension Management
Comprehensive extension management system:

```toml
[extensions]
auto_load = true
allow_network = true
update_policy = "stable"

directories = [
    "~/.niv/extensions",
    "~/.local/share/niv/extensions"
]

trusted_sources = [
    "https://github.com/niv-editor/extensions"
]

# Example extensions
[extensions.rust_analyzer]
enabled = true
version = "latest"
settings.lsp_enabled = true
settings.formatting_enabled = true

[extensions.theme_dark]
enabled = true
settings.variant = "dark_plus"
```

### [custom] - User-Defined Settings
Space for custom configuration values:

```toml
[custom]
project_name = "my_project"
project_version = "1.0.0"
author = "Your Name"

debug_mode = true
log_level = "info"
max_open_files = 50

experimental_features = ["lsp", "tree_sitter"]
disabled_features = ["minimap"]
```

## Usage

### Basic Usage

```rust
use niv_config::{Config, ConfigLoader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration with automatic path discovery
    let config = Config::load()?;

    // Access editor settings
    println!("Tab width: {}", config.editor.tab_width);
    println!("Line numbers: {}", config.editor.line_numbers);

    // Access UI settings
    println!("Font size: {}", config.ui.font_size);
    println!("Color scheme: {}", config.ui.color_scheme);

    Ok(())
}
```

### Using ConfigLoader for Hot-Reload

```rust
use niv_config::{ConfigLoader, ConfigResult};
use std::time::Duration;

fn main() -> ConfigResult<()> {
    // Create loader with hot-reload capability
    let mut loader = ConfigLoader::new()
        .with_reload_interval(Duration::from_secs(2));

    // Load initial configuration
    loader.load()?;

    // Check for configuration changes periodically
    loop {
        if loader.check_reload()? {
            println!("Configuration reloaded!");
            let config = loader.get_copy();
            // Apply new configuration...
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
```

### Programmatic Configuration

```rust
use niv_config::{ConfigBuilder, ConfigResult};

fn main() -> ConfigResult<()> {
    let config = ConfigBuilder::new()
        .editor(|editor| {
            editor.line_numbers = true;
            editor.tab_width = 4;
            editor.syntax = true;
        })
        .ui(|ui| {
            ui.color_scheme = "dark".to_string();
            ui.font_size = 12;
        })
        .custom("theme", "dark")
        .build();

    // Save configuration
    config.save_to_file("my_config.toml")?;

    Ok(())
}
```

### Custom Configuration Values

```rust
use niv_config::{Config, ConfigResult, TomlValue};

fn main() -> ConfigResult<()> {
    let mut config = Config::load()?;

    // Set custom values
    config.set_custom("my_setting".to_string(), TomlValue::String("value".to_string()));
    config.set_custom("debug_mode".to_string(), TomlValue::Bool(true));
    config.set_custom("max_files".to_string(), TomlValue::Integer(100));

    // Get custom values
    if let Some(value) = config.get_custom("my_setting") {
        if let Some(string_val) = value.as_string() {
            println!("Custom setting: {}", string_val);
        }
    }

    Ok(())
}
```

## Configuration File Locations

The system searches for configuration files in the following order:

1. `~/.niv/config.toml` (user-specific)
2. `~/.config/niv/config.toml` (XDG config directory)
3. `/etc/niv/config.toml` (system-wide)
4. `/usr/local/etc/niv/config.toml` (system-wide alternative)
5. `./.niv.toml` or `./niv.toml` (project-specific)

## Keybinding Syntax

Keybindings use a modifier+key syntax:

- `Ctrl+S` - Control + S
- `Alt+F1` - Alt + F1
- `Shift+Enter` - Shift + Enter
- `F12` - Just F12
- `g` - Just the 'g' key

Available modifiers: `Ctrl`, `Control`, `Alt`, `Shift`, `Meta`, `Super`, `Win`, `Cmd`

## Extension System

Extensions can be:
- **Local**: Stored in extension directories
- **Remote**: Downloaded from trusted sources
- **Inline**: Defined directly in configuration

Each extension can have:
- Custom settings
- Version requirements
- Enable/disable toggle
- Hook points for editor events

## Architecture

The configuration system is built with several layers:

1. **TOML Parser** - Custom TOML parser (no external dependencies)
2. **Settings Modules** - Type-safe configuration structures
3. **Config Core** - Main configuration management
4. **Loader** - Hot-reload capable configuration loader
5. **Manager** - Multi-configuration management

## Error Handling

All configuration operations return `ConfigResult<T>` which contains detailed error information:

- `ConfigError::Io` - File I/O errors
- `ConfigError::Toml` - TOML parsing errors
- `ConfigError::Validation` - Configuration validation errors
- `ConfigError::Path` - Path resolution errors
- `ConfigError::Permission` - Permission errors

## Performance

- Lazy loading of configuration files
- Efficient TOML parsing
- Minimal memory footprint
- Hot-reload with configurable intervals
- Thread-safe configuration access

## Examples

See the `examples/` directory for complete usage examples:

- `editor.toml` - Complete configuration example
- `simple_demo.rs` - Basic usage demonstration

## Building

The configuration system has no external dependencies (except for tokio-level major libs as requested), making it lightweight and secure.

```bash
cargo build --release
```

## Contributing

The configuration system is designed to be extensible. To add new configuration sections:

1. Create a new module with your configuration structure
2. Implement `From<TomlValue>` and `To<TomlValue>` traits
3. Add the section to the main `Config` struct
4. Update the loader to handle the new section

This ensures type safety while maintaining flexibility for future enhancements.
