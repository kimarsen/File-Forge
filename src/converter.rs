use std::path::Path;
use std::process::Command;

pub fn convert(input_path: &Path, target_format: &str) -> Result<(), String> {
    let output_path = input_path.with_extension(target_format);

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
            Ok(())
    } else {
        Err(format!("ffmpeg finished with error code: {:?}", status.code()))
    }
}