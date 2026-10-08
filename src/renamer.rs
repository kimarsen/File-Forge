use std::fs;
use std::path::Path;

pub fn rename_with_prefix(input_path: &Path, prefix: &str) -> Result<(), String> {
    let parent = input_path.parent().unwrap_or(Path::new(""));
    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    if file_name.starts_with(prefix) {
        return Ok(());
    }

    let new_name = format!("{}{}", prefix, file_name);
    let new_path = parent.join(new_name);

    println!("Renaming: {:?} -> {:?}", input_path, new_path);

    fs::rename(input_path, &new_path)
        .map_err(|e| format!("Failed to rename file: {}", e))?;
    
    println!("Successfully renamed file to {:?}", new_path);
    Ok(())
}