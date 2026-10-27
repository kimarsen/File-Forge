use crate::rules::Rule;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub watch_folder: String,
    pub rules: Vec<Rule>,
}

impl AppConfig { //
    pub fn default_config() -> Self {
        Self {
            watch_folder: r"C:\Users\THE_BLACK_SWORDSMAN\Downloads".to_string(),
            rules: vec![
                Rule::new("ogg", "mp3"),
                Rule::new("wav", "mp3"),
                Rule::new_rename("txt", "DONE_").with_name_filter("report"),
            ],
        }
    }

    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read config file: {}", e))?;
            let config: AppConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse JSON config: {}", e))?;
            Ok(config)
        } else {
            let config = Self::default_config();
            let json = serde_json::to_string_pretty(&config)
                .map_err(|e| format!("Failed to serialize default config: {}", e))?;
            fs::write(path, json)
                .map_err(|e| format!("Failed to write default config file: {}", e))?;
            println!("Default config file created at {:?}", path);
            Ok(config)
        }
    }
}