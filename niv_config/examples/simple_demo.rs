use niv_config::*;
use std::path::PathBuf;

fn main() -> ConfigResult<()> {
    println!("=== niv Config System Demo ===\n");

    // Create a configuration loader
    let mut loader = ConfigLoader::new();

    // Try to load configuration (will use defaults if no config file exists)
    println!("Loading configuration...");
    loader.load()?;

    // Get current configuration
    let config = loader.get_copy();
    println!("Configuration loaded successfully!\n");

    // Display editor settings
    println!("Editor Settings:");
    println!("  Line numbers: {}", config.editor.line_numbers);
    println!("  Tab width: {}", config.editor.tab_width);
    println!("  Expand tabs: {}", config.editor.expand_tab);
    println!("  Syntax highlighting: {}", config.editor.syntax);
    println!("  Auto indent: {}", config.editor.auto_indent);
    println!();

    // Display UI settings
    println!("UI Settings:");
    println!("  Color scheme: {}", config.ui.color_scheme);
    println!("  Font family: {}", config.ui.font_family);
    println!("  Font size: {}", config.ui.font_size);
    println!("  Status line: {}", config.ui.status_line);
    println!();

    // Display extension settings
    println!("Extension Settings:");
    println!("  Auto load: {}", config.extensions.auto_load);
    println!("  Allow network: {}", config.extensions.allow_network);
    println!("  Update policy: {:?}", config.extensions.update_policy);
    println!("  Extension directories: {}", config.extensions.directories.len());
    println!();

    // Demonstrate setting values
    println!("Demonstrating configuration updates...");

    // Update some settings using the loader
    loader.set_value("editor.line_numbers", TomlValue::Bool(false))?;
    loader.set_value("editor.tab_width", TomlValue::Integer(2))?;
    loader.set_value("ui.font_size", TomlValue::Integer(14))?;

    // Add a custom setting
    loader.set_value("custom.theme", TomlValue::String("dark".to_string()))?;

    println!("Settings updated!\n");

    // Show the updated configuration
    let updated_config = loader.get_copy();
    println!("Updated Editor Settings:");
    println!("  Line numbers: {}", updated_config.editor.line_numbers);
    println!("  Tab width: {}", updated_config.editor.tab_width);
    println!("  Font size: {}", updated_config.ui.font_size);

    if let Some(theme) = updated_config.get_custom("theme") {
        if let Ok(theme_str) = theme.as_string() {
            println!("  Custom theme: {}", theme_str);
        }
    }
    println!();

    // Demonstrate saving configuration
    println!("Saving configuration...");
    let save_path = PathBuf::from("demo_config.toml");
    loader.save_to(&save_path)?;
    println!("Configuration saved to: {}", save_path.display());

    // Show what the saved configuration looks like
    println!("\nGenerated TOML configuration:");
    println!("{}", updated_config.to_toml_string());

    // Clean up demo file
    if save_path.exists() {
        std::fs::remove_file(save_path)?;
    }

    println!("\n=== Demo completed successfully! ===");

    Ok(())
}
