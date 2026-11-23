use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn default_true() -> bool {
    true
}

pub fn new_id() -> String {
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("rule-{nanos}-{sequence}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
    Audio,
    Video,
    Images,
}

impl PresetKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "audio" | "mp3" | "аудио" => Some(Self::Audio),
            "video" | "mp4" | "видео" => Some(Self::Video),
            "images" | "image" | "jpg" | "картинки" | "изображения" => {
                Some(Self::Images)
            }
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Audio => "Аудио → MP3",
            Self::Video => "Видео → MP4",
            Self::Images => "Картинки → JPG",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum Action {
    Convert {
        target_format: String,
        #[serde(default)]
        delete_original: Option<bool>,
    },
    Rename {
        #[serde(default)]
        pattern: Option<String>,
        #[serde(default)]
        prefix: Option<String>,
        #[serde(default)]
        suffix: Option<String>,
        #[serde(default)]
        find: Option<String>,
        #[serde(default)]
        replace: Option<String>,
    },
    Move {
        dest: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Rule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub watch_dir: Option<String>,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default)]
    pub name_contains: Option<String>,
    #[serde(default)]
    pub name_pattern: Option<String>,
    pub action: Action,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Rule {
    pub fn new(extension: &str, target_format: &str) -> Self {
        Self {
            id: new_id(),
            watch_dir: None,
            extension: Some(extension.to_string()),
            name_contains: None,
            name_pattern: None,
            action: Action::Convert {
                target_format: target_format.to_string(),
                delete_original: None,
            },
            output_dir: None,
            enabled: true,
        }
    }

    pub fn new_rename(extension: &str, prefix: &str) -> Self {
        Self {
            id: new_id(),
            watch_dir: None,
            extension: Some(extension.to_string()),
            name_contains: None,
            name_pattern: None,
            action: Action::Rename {
                pattern: None,
                prefix: Some(prefix.to_string()),
                suffix: None,
                find: None,
                replace: None,
            },
            output_dir: None,
            enabled: true,
        }
    }

    pub fn with_name_filter(mut self, substr: &str) -> Self {
        self.name_contains = Some(substr.to_string());
        self
    }

    pub fn matches(&self, path: &Path) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(dir) = self
            .watch_dir
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            && !paths::is_within(path, Path::new(dir))
        {
            return false;
        }
        if let Some(rule_ext) = self.extension.as_ref() {
            let expected = rule_ext.trim().trim_start_matches('.').to_ascii_lowercase();
            if !expected.is_empty() {
                let file_ext = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if file_ext != expected {
                    return false;
                }
            }
        }
        if let Some(substr) = self
            .name_contains
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let file_stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if !file_stem.to_lowercase().contains(&substr.to_lowercase()) {
                return false;
            }
        }
        if let Some(pattern) = self
            .name_pattern
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            let file_stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if !glob_match(pattern, file_name) && !glob_match(pattern, file_stem) {
                return false;
            }
        }
        true
    }
}

pub fn find_matching_rules<'a>(rules: &'a [Rule], path: &Path) -> Vec<&'a Rule> {
    rules.iter().filter(|rule| rule.matches(path)).collect()
}

struct PresetSpec {
    extensions: &'static [&'static str],
    target: &'static str,
}

fn preset_spec(kind: PresetKind) -> PresetSpec {
    match kind {
        PresetKind::Audio => PresetSpec {
            extensions: &["ogg", "wav", "flac"],
            target: "mp3",
        },
        PresetKind::Video => PresetSpec {
            extensions: &["mkv", "webm"],
            target: "mp4",
        },
        PresetKind::Images => PresetSpec {
            extensions: &["webp", "png"],
            target: "jpg",
        },
    }
}

pub fn preset_rules(
    kind: PresetKind,
    watch_dir: Option<String>,
    output_dir: Option<String>,
) -> Vec<Rule> {
    let spec = preset_spec(kind);
    spec.extensions
        .iter()
        .map(|extension| Rule {
            id: new_id(),
            watch_dir: watch_dir.clone(),
            extension: Some((*extension).to_string()),
            name_contains: None,
            name_pattern: None,
            action: Action::Convert {
                target_format: spec.target.to_string(),
                delete_original: None,
            },
            output_dir: output_dir.clone(),
            enabled: true,
        })
        .collect()
}

pub fn find_rule_index(rules: &[Rule], id: &str) -> Option<usize> {
    if let Ok(number) = id.parse::<usize>()
        && (1..=rules.len()).contains(&number)
    {
        return Some(number - 1);
    }
    rules.iter().position(|rule| rule.id == id)
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().flat_map(|ch| ch.to_lowercase()).collect();
    let text: Vec<char> = text.chars().flat_map(|ch| ch.to_lowercase()).collect();
    let mut states = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    states[0][0] = true;
    for pattern_index in 1..=pattern.len() {
        if pattern[pattern_index - 1] == '*' {
            states[pattern_index][0] = states[pattern_index - 1][0];
        }
    }
    for pattern_index in 1..=pattern.len() {
        for text_index in 1..=text.len() {
            if pattern[pattern_index - 1] == '*' {
                states[pattern_index][text_index] =
                    states[pattern_index - 1][text_index] || states[pattern_index][text_index - 1];
            } else if pattern[pattern_index - 1] == '?'
                || pattern[pattern_index - 1] == text[text_index - 1]
            {
                states[pattern_index][text_index] = states[pattern_index - 1][text_index - 1];
            }
        }
    }
    states[pattern.len()][text.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_legacy_rule() {
        let json = r#"{
            "extension": "ogg",
            "name_contains": null,
            "action": { "Convert": { "target_format": "mp3" } }
        }"#;
        let rule: Rule = serde_json::from_str(json).unwrap();
        assert!(rule.enabled);
        assert!(rule.matches(Path::new("/tmp/voice.ogg")));
        assert!(!rule.matches(Path::new("/tmp/voice.wav")));
    }

    #[test]
    fn name_filter_and_pattern() {
        let rule = Rule::new_rename("txt", "DONE_").with_name_filter("report");
        assert!(rule.matches(Path::new("notes/report-final.TXT")));
        assert!(!rule.matches(Path::new("notes/other.txt")));

        let mut patterned = Rule::new("mp3", "mp3");
        patterned.name_pattern = Some("take-*".to_string());
        assert!(patterned.matches(Path::new("take-01.mp3")));
        assert!(!patterned.matches(Path::new("demo.mp3")));
    }

    #[test]
    fn watch_dir_limits_matches() {
        let root = std::env::temp_dir().join(format!(
            "file-forge-rules-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let left = root.join("left");
        let right = root.join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        let mut rule = Rule::new("ogg", "mp3");
        rule.watch_dir = Some(left.to_string_lossy().to_string());
        assert!(rule.matches(&left.join("a.ogg")));
        assert!(!rule.matches(&right.join("a.ogg")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn find_matching_rules_keeps_order() {
        let rules = vec![Rule::new("ogg", "mp3"), Rule::new("wav", "mp3")];
        let matched = find_matching_rules(&rules, Path::new("a.ogg"));
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].extension.as_deref(), Some("ogg"));
    }

    #[test]
    fn presets_follow_the_plan() {
        let audio = preset_rules(PresetKind::Audio, None, None);
        let video = preset_rules(PresetKind::Video, None, None);
        let images = preset_rules(PresetKind::Images, None, None);
        assert_eq!(audio.len(), 3);
        assert_eq!(video.len(), 2);
        assert_eq!(images.len(), 2);
        assert!(
            audio
                .iter()
                .any(|rule| rule.extension.as_deref() == Some("flac"))
        );
        assert!(
            video
                .iter()
                .any(|rule| rule.extension.as_deref() == Some("mkv"))
        );
        assert!(
            images
                .iter()
                .any(|rule| rule.extension.as_deref() == Some("webp"))
        );
    }

    #[test]
    fn glob_supports_stars_and_questions() {
        assert!(glob_match("*.ogg", "Voice.OGG"));
        assert!(glob_match("song?", "song1"));
        assert!(!glob_match("song?", "song10"));
        assert!(glob_match("*report*", "final-report-v2"));
    }

    #[test]
    fn rule_index_accepts_position_and_id() {
        let rules = vec![Rule::new("ogg", "mp3")];
        let id = rules[0].id.clone();
        assert_eq!(find_rule_index(&rules, "1"), Some(0));
        assert_eq!(find_rule_index(&rules, &id), Some(0));
        assert_eq!(find_rule_index(&rules, "missing"), None);
    }
}
