use niv_frontend::Editor;
use niv_fs::load_file;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("NIV Editor starting...");
    let args: Vec<String> = env::args().collect();
    println!("Args: {:?}", args);

    println!("Creating editor...");
    let mut editor = Editor::new();
    println!("Editor created successfully");

    // Open file if provided as argument
    if args.len() > 1 {
        let file_path = PathBuf::from(&args[1]);

        if file_path.exists() {
            // Load file using niv_fs
            match load_file(&file_path) {
                Ok(load_result) => {
                    let line_count = load_result.content.lines().count();
                    // Pass the loaded content to the editor
                    editor.open_buffer_from_content(file_path.clone(), load_result)?;
                    println!(
                        "Loaded file: {} ({} lines)",
                        file_path.display(),
                        line_count
                    );
                }
                Err(e) => {
                    eprintln!("Failed to load file '{}': {}", file_path.display(), e);
                    return Err(e.into());
                }
            }
        } else {
            // For new files, create an empty buffer
            editor.create_new_buffer(file_path.clone())?;
            println!("Created new file: {}", file_path.display());
        }
    }

    // Run the TUI editor
    editor.run()?;

    Ok(())
}
