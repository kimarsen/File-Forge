use std::fs::{self, File};
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

fn clean_file_stem(stem: &str) -> String {
    if let Some(pos) = stem.rfind('(') {
        if stem.ends_with(')') {
            let inside = &stem[pos + 1..stem.len() - 1];
            if !inside.is_empty() && inside.chars().all(|c| c.is_ascii_digit()) {
                return stem[..pos].trim_end().to_string();
            }
        }
    }
    stem.to_string()
}

fn wait_until_ready(path: &Path) -> Result<(), String> {
    for _ in 0..10 {
        if let Ok(file) = File::open(path) {
            if let Ok(metadata) = file.metadata() {
                let initial_size = metadata.len();
                if initial_size > 0 {
                    sleep(Duration::from_millis(300));
                    if let Ok(new_metadata) = file.metadata() {
                        if new_metadata.len() == initial_size {
                            return Ok(());
                        }
                    }
                }
            }
        }
        sleep(Duration::from_millis(300));
    }
    Err("File is empty or still being written".to_string())
}

pub fn convert(input_path: &Path, target_format: &str) -> Result<(), String> {
    wait_until_ready(input_path)?;

    let parent = input_path.parent().unwrap_or(Path::new(""));
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    let clean_name = clean_file_stem(stem);
    let output_file_name = format!("{}.{}", clean_name, target_format);
    let output_path = parent.join(output_file_name);

    println!("Converting: {:?} -> {:?}", input_path, output_path);

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg(&output_path)
        .status()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if status.success() {
        println!("Successfully converted to {:?}", output_path);

        if let Err(e) = fs::remove_file(input_path) {
            eprintln!("Failed to delete original file: {}", e);
        } else {
            println!("Deleted original file: {:?}", input_path);
        }
        Ok(())
    } else {
        Err(format!("ffmpeg finished with error code: {:?}", status.code()))
    }
}