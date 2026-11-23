use crate::paths::ensure_parent;
use crate::rules::{Rule, new_id};
use crate::scanner::TimeFilter;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

fn default_mp3_qscale() -> u8 {
    2
}

fn default_audio_bitrate() -> String {
    "192k".to_string()
}

fn default_video_crf() -> u8 {
    23
}

fn default_image_quality() -> u8 {
    2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualitySettings {
    #[serde(default = "default_mp3_qscale")]
    pub mp3_qscale: u8,
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,
    #[serde(default)]
    pub audio_codec: String,
    #[serde(default)]
    pub video_codec: String,
    #[serde(default = "default_video_crf")]
    pub video_crf: u8,
    #[serde(default = "default_image_quality")]
    pub image_quality: u8,
}

impl Default for QualitySettings {
    fn default() -> Self {
        Self {
            mp3_qscale: default_mp3_qscale(),
            audio_bitrate: default_audio_bitrate(),
            audio_codec: String::new(),
            video_codec: String::new(),
            video_crf: default_video_crf(),
            image_quality: default_image_quality(),
        }
    }
}

impl QualitySettings {
    pub fn normalized(mut self) -> Self {
        self.mp3_qscale = self.mp3_qscale.min(9);
        self.video_crf = self.video_crf.min(51);
        if self.image_quality == 0 {
            self.image_quality = default_image_quality();
        }
        self.image_quality = self.image_quality.clamp(1, 31);
        if self.audio_bitrate.trim().is_empty() {
            self.audio_bitrate = default_audio_bitrate();
        } else {
            self.audio_bitrate = self.audio_bitrate.trim().to_string();
        }
        self.audio_codec = self.audio_codec.trim().to_string();
        self.video_codec = self.video_codec.trim().to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub delete_original: bool,
    #[serde(default)]
    pub scan_on_start: TimeFilter,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub watch_on_start: bool,
    #[serde(default)]
    pub quality: QualitySettings,
    #[serde(default)]
    pub counter: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            delete_original: true,
            scan_on_start: TimeFilter::default(),
            theme: Theme::default(),
            autostart: false,
            watch_on_start: true,
            quality: QualitySettings::default(),
            counter: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub watch_folder: String,
    #[serde(default)]
    pub watch_folders: Vec<String>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub settings: Settings,
}

impl AppConfig {
    pub fn default_config() -> Self {
        Self {
            watch_folder: default_watch_folder(),
            watch_folders: Vec::new(),
            rules: vec![
                Rule::new("ogg", "mp3"),
                Rule::new("wav", "mp3"),
                Rule::new_rename("txt", "DONE_").with_name_filter("report"),
            ],
            settings: Settings::default(),
        }
    }

    pub fn ensure_ids(&mut self) -> bool {
        let mut changed = false;
        let mut seen = HashSet::new();
        for rule in &mut self.rules {
            let missing = rule.id.trim().is_empty();
            let duplicate = !missing && !seen.insert(rule.id.clone());
            if missing || duplicate {
                rule.id = new_id();
                seen.insert(rule.id.clone());
                changed = true;
            }
        }
        let quality = self.settings.quality.clone().normalized();
        if quality != self.settings.quality {
            self.settings.quality = quality;
            changed = true;
        }
        if self.settings.counter == 0 {
            self.settings.counter = 1;
            changed = true;
        }
        changed
    }

    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|err| format!("не удалось прочитать конфиг: {err}"))?;
            serde_json::from_str(&content)
                .map_err(|err| format!("не удалось разобрать конфиг: {err}"))
        } else {
            ensure_parent(path)?;
            let config = Self::default_config();
            config.save(path)?;
            Ok(config)
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        ensure_parent(path)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| format!("не удалось сериализовать конфиг: {err}"))?;
        fs::write(path, json).map_err(|err| format!("не удалось записать конфиг: {err}"))
    }
}

pub fn default_config_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.trim().is_empty()
    {
        return PathBuf::from(appdata)
            .join("file-forge")
            .join("config.json");
    }
    if cfg!(windows)
        && let Ok(profile) = std::env::var("USERPROFILE")
    {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join("file-forge")
            .join("config.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("file-forge")
            .join("config.json");
    }
    PathBuf::from("config.json")
}

pub fn resolve_config_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Ok(path) = std::env::var("FILE_FORGE_CONFIG")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }
    let local = PathBuf::from("config.json");
    if local.exists() {
        return local;
    }
    default_config_path()
}

fn default_watch_folder() -> String {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("Downloads")
            .to_string_lossy()
            .to_string();
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Downloads")
            .to_string_lossy()
            .to_string();
    }
    "Downloads".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_checked_in_config() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.json");
        let config = AppConfig::load_or_create(&path).unwrap();
        assert!(config.watch_folder.contains("Downloads"));
        assert_eq!(config.rules.len(), 3);
        assert!(config.settings.delete_original);
        assert_eq!(config.settings.scan_on_start, TimeFilter::LastDay);
        assert!(config.rules[2].matches(Path::new("report.txt")));
        assert!(!config.rules[2].matches(Path::new("notes.txt")));
    }

    #[test]
    fn roundtrip_preserves_rules() {
        let path = std::env::temp_dir().join(format!(
            "file-forge-config-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut config = AppConfig::default_config();
        config.watch_folders.push("/tmp/incoming".to_string());
        config.save(&path).unwrap();
        let loaded = AppConfig::load_or_create(&path).unwrap();
        assert_eq!(loaded.watch_folders, config.watch_folders);
        assert_eq!(loaded.rules.len(), config.rules.len());
        let _ = fs::remove_file(path);
    }
}
