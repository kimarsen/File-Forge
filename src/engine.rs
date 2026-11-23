use crate::config::{AppConfig, QualitySettings};
use crate::converter::{self, output_path};
use crate::log::{self, OperationRecord, timestamp_now};
use crate::paths::{self, relocate, unique_path};
use crate::renamer::{self, NameContext, RenameSpec};
use crate::rules::{self, Action, PresetKind, Rule, find_rule_index};
use crate::scanner::{self, TimeFilter};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const OUTPUT_GUARD: Duration = Duration::from_secs(15);

pub struct Engine {
    config_path: PathBuf,
    log_path: PathBuf,
    config: AppConfig,
    produced: Vec<(PathBuf, Instant)>,
}

impl Engine {
    pub fn open(config_path: PathBuf) -> Result<Self, String> {
        let mut config = AppConfig::load_or_create(&config_path)?;
        let changed = config.ensure_ids();
        let log_path = log::log_path_for(&config_path);
        let engine = Self {
            config_path,
            log_path,
            config,
            produced: Vec::new(),
        };
        if changed {
            engine.save()?;
        }
        Ok(engine)
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn save(&self) -> Result<(), String> {
        self.config.save(&self.config_path)
    }

    pub fn replace_config(&mut self, mut config: AppConfig) -> Result<(), String> {
        config.ensure_ids();
        config.settings.quality = config.settings.quality.clone().normalized();
        self.config = config;
        self.save()
    }

    pub fn update<F>(&mut self, change: F) -> Result<(), String>
    where
        F: FnOnce(&mut AppConfig),
    {
        change(&mut self.config);
        self.config.ensure_ids();
        self.config.settings.quality = self.config.settings.quality.clone().normalized();
        self.save()
    }

    pub fn add_preset(
        &mut self,
        kind: PresetKind,
        watch_dir: Option<String>,
        output_dir: Option<String>,
    ) -> Result<Vec<Rule>, String> {
        let rules = rules::preset_rules(kind, watch_dir, output_dir);
        self.config.rules.extend(rules.iter().cloned());
        self.save()?;
        Ok(rules)
    }

    pub fn watch_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        push_dir(&mut dirs, &self.config.watch_folder);
        for dir in &self.config.watch_folders {
            push_dir(&mut dirs, dir);
        }
        for rule in &self.config.rules {
            if !rule.enabled {
                continue;
            }
            if let Some(dir) = &rule.watch_dir {
                push_dir(&mut dirs, dir);
            }
        }
        dirs
    }

    pub fn prepare_watch_dirs(&self) -> Result<Vec<PathBuf>, String> {
        let dirs = self.watch_dirs();
        if dirs.is_empty() {
            return Err("в конфиге нет папок для наблюдения".to_string());
        }
        for dir in &dirs {
            fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        }
        Ok(dirs)
    }

    pub fn recent_log(&self, limit: usize) -> Result<Vec<OperationRecord>, String> {
        log::read_recent(&self.log_path, limit)
    }

    pub fn process_existing(&mut self, filter: TimeFilter) -> Vec<OperationRecord> {
        let files = self
            .watch_dirs()
            .into_iter()
            .flat_map(|dir| scanner::scan_directory(&dir, filter))
            .collect::<Vec<_>>();
        let mut records = Vec::new();
        for file in files {
            records.extend(self.process_path(&file));
        }
        records
    }

    pub fn process_path(&mut self, path: &Path) -> Vec<OperationRecord> {
        if !path.is_file() || self.is_recent_output(path) {
            return Vec::new();
        }
        if self.config.ensure_ids() {
            let _ = self.save();
        }
        let mut records = Vec::new();
        let mut current = path.to_path_buf();
        let mut used = HashSet::new();
        for _ in 0..20 {
            if !current.is_file() {
                break;
            }
            let Some(rule) = self
                .config
                .rules
                .iter()
                .find(|rule| rule.enabled && !used.contains(&rule.id) && rule.matches(&current))
                .cloned()
            else {
                break;
            };
            used.insert(rule.id.clone());
            let source = current.clone();
            let action = action_name(&rule.action).to_string();
            let quality = self.config.settings.quality.clone();
            let delete_default = self.config.settings.delete_original;
            let mut counter = self.config.settings.counter;
            let result = apply_rule(&rule, &source, &quality, delete_default, &mut counter);
            let counter_changed = counter != self.config.settings.counter;
            self.config.settings.counter = counter;
            match result {
                Ok(Step::Skipped) => {}
                Ok(Step::Done {
                    destination,
                    message,
                }) => {
                    if let Some(destination) = &destination {
                        self.remember(destination);
                        current = destination.clone();
                    }
                    records.push(self.record(
                        true,
                        &source,
                        destination.as_deref(),
                        &action,
                        &message,
                    ));
                }
                Err(err) => {
                    records.push(self.record(false, &source, None, &action, &err));
                }
            }
            if counter_changed {
                let _ = self.save();
            }
        }
        records
    }

    pub fn convert_file(
        &mut self,
        input: &Path,
        output: Option<&Path>,
        format: &str,
        delete: Option<bool>,
    ) -> OperationRecord {
        let format = converter::normalize_format(format);
        let destination = output
            .map(Path::to_path_buf)
            .unwrap_or_else(|| output_path(input, &format, None));
        let quality = self.config.settings.quality.clone();
        let delete = delete.unwrap_or(self.config.settings.delete_original);
        match converter::convert(input, &destination, &format, &quality) {
            Ok(()) => {
                let mut message = String::new();
                if delete && input != destination {
                    match fs::remove_file(input) {
                        Ok(()) => message = "оригинал удалён".to_string(),
                        Err(err) => message = format!("не удалось удалить оригинал: {err}"),
                    }
                }
                self.remember(&destination);
                self.record(true, input, Some(&destination), "convert", &message)
            }
            Err(err) => self.record(false, input, Some(&destination), "convert", &err),
        }
    }

    fn record(
        &self,
        success: bool,
        source: &Path,
        destination: Option<&Path>,
        action: &str,
        message: &str,
    ) -> OperationRecord {
        let record = OperationRecord {
            timestamp: timestamp_now(),
            source: source.display().to_string(),
            destination: destination.map(|path| path.display().to_string()),
            action: action.to_string(),
            success,
            message: message.to_string(),
        };
        if let Err(err) = log::append_record(&self.log_path, &record) {
            tracing::error!("log: {err}");
        }
        record
    }

    fn remember(&mut self, path: &Path) {
        self.produced
            .push((paths::make_absolute(path), Instant::now()));
    }

    fn is_recent_output(&mut self, path: &Path) -> bool {
        let path = paths::make_absolute(path);
        self.produced
            .retain(|(_, seen)| seen.elapsed() < OUTPUT_GUARD);
        self.produced.iter().any(|(produced, _)| produced == &path)
    }
}

enum Step {
    Skipped,
    Done {
        destination: Option<PathBuf>,
        message: String,
    },
}

fn apply_rule(
    rule: &Rule,
    path: &Path,
    quality: &QualitySettings,
    delete_default: bool,
    counter: &mut u64,
) -> Result<Step, String> {
    match &rule.action {
        Action::Convert {
            target_format,
            delete_original,
        } => {
            let format = converter::normalize_format(target_format);
            if format.is_empty() {
                return Err("не указан формат".to_string());
            }
            let directory = rule_output_dir(rule);
            let destination = output_path(path, &format, directory.as_deref());
            converter::convert(path, &destination, &format, quality)?;
            let mut message = String::new();
            if delete_original.unwrap_or(delete_default) && path != destination {
                match fs::remove_file(path) {
                    Ok(()) => message = "оригинал удалён".to_string(),
                    Err(err) => message = format!("не удалось удалить оригинал: {err}"),
                }
            }
            Ok(Step::Done {
                destination: Some(destination),
                message,
            })
        }
        Action::Rename {
            pattern,
            prefix,
            suffix,
            find,
            replace,
        } => {
            let spec = RenameSpec {
                pattern: pattern.clone(),
                prefix: prefix.clone(),
                suffix: suffix.clone(),
                find: find.clone(),
                replace: replace.clone(),
            };
            let directory = rule_output_dir(rule);
            let mut context = NameContext::now(*counter);
            match renamer::rename_file(path, &spec, directory.as_deref(), &mut context)? {
                Some(destination) => {
                    *counter = context.counter;
                    Ok(Step::Done {
                        destination: Some(destination),
                        message: String::new(),
                    })
                }
                None => Ok(Step::Skipped),
            }
        }
        Action::Move { dest } => {
            let dest = dest.trim();
            if dest.is_empty() {
                return Err("не указана папка назначения".to_string());
            }
            let target = move_target(Path::new(dest), path);
            let target = if target.exists() && target != path {
                unique_path(&target)
            } else {
                target
            };
            if target == path {
                return Ok(Step::Skipped);
            }
            relocate(path, &target)?;
            Ok(Step::Done {
                destination: Some(target),
                message: String::new(),
            })
        }
    }
}

fn rule_output_dir(rule: &Rule) -> Option<PathBuf> {
    rule.output_dir
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn move_target(dest: &Path, source: &Path) -> PathBuf {
    let text = dest.to_string_lossy();
    let as_directory =
        dest.is_dir() || dest.extension().is_none() || text.ends_with('/') || text.ends_with('\\');
    if as_directory {
        dest.join(source.file_name().unwrap_or_default())
    } else {
        dest.to_path_buf()
    }
}

fn push_dir(dirs: &mut Vec<PathBuf>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    let path = PathBuf::from(trimmed);
    let key = paths::make_absolute(&path);
    if dirs
        .iter()
        .any(|existing| paths::make_absolute(existing) == key)
    {
        return;
    }
    dirs.push(path);
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Convert { .. } => "convert",
        Action::Rename { .. } => "rename",
        Action::Move { .. } => "move",
    }
}

pub fn describe_rule(index: usize, rule: &Rule) -> String {
    let state = if rule.enabled { "вкл" } else { "выкл" };
    let extension = rule
        .extension
        .as_ref()
        .map(|value| value.trim().trim_start_matches('.'))
        .filter(|value| !value.is_empty())
        .unwrap_or("*");
    let mut details = Vec::new();
    if let Some(value) = present(&rule.name_contains) {
        details.push(format!("имя содержит «{value}»"));
    }
    if let Some(value) = present(&rule.name_pattern) {
        details.push(format!("шаблон «{value}»"));
    }
    if let Some(value) = present(&rule.watch_dir) {
        details.push(format!("папка {value}"));
    }
    if let Some(value) = present(&rule.output_dir) {
        details.push(format!("выход {value}"));
    }
    let action = match &rule.action {
        Action::Convert {
            target_format,
            delete_original,
        } => {
            let deletion = match delete_original {
                Some(true) => "удалять оригинал",
                Some(false) => "оставлять оригинал",
                None => "оригинал по общей настройке",
            };
            format!("конвертировать в {target_format}, {deletion}")
        }
        Action::Rename {
            pattern,
            prefix,
            suffix,
            find,
            replace,
        } => {
            let mut parts = Vec::new();
            if let Some(value) = present(prefix) {
                parts.push(format!("префикс {value}"));
            }
            if let Some(value) = present(suffix) {
                parts.push(format!("суффикс {value}"));
            }
            if let Some(value) = present(find) {
                let replacement = replace.as_deref().unwrap_or("");
                parts.push(format!("замена «{value}» → «{replacement}»"));
            }
            if let Some(value) = present(pattern) {
                parts.push(format!("шаблон {value}"));
            }
            if parts.is_empty() {
                "переименовать".to_string()
            } else {
                format!("переименовать: {}", parts.join(", "))
            }
        }
        Action::Move { dest } => format!("переместить в {dest}"),
    };
    let extra = if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    };
    format!(
        "{index}. {} [{state}] *.{extension}: {action}{extra}",
        rule.id
    )
}

fn present(value: &Option<String>) -> Option<&str> {
    value
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
}

pub fn rule_by_token<'a>(rules: &'a [Rule], token: &str) -> Option<(usize, &'a Rule)> {
    find_rule_index(rules, token).map(|index| (index, &rules[index]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("file-forge-{label}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn engine_in(root: &Path) -> Engine {
        Engine::open(root.join("config.json")).unwrap()
    }

    #[test]
    fn renames_reports_and_ignores_its_output() {
        let root = temp_dir("engine-rename");
        let mut engine = engine_in(&root);
        engine
            .update(|config| {
                config.watch_folder = root.to_string_lossy().to_string();
                config.rules = vec![Rule::new_rename("txt", "DONE_").with_name_filter("report")];
                config.settings.counter = 1;
            })
            .unwrap();
        let source = root.join("report.txt");
        fs::write(&source, b"quarter").unwrap();
        fs::write(root.join("notes.txt"), b"skip").unwrap();
        let records = engine.process_path(&source);
        assert_eq!(records.len(), 1);
        assert!(records[0].success);
        let renamed = root.join("DONE_report.txt");
        assert!(renamed.exists());
        assert!(!source.exists());
        assert!(engine.process_path(&renamed).is_empty());
        assert!(root.join("notes.txt").exists());
        let logged = engine.recent_log(10).unwrap();
        assert_eq!(logged.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn converts_wav_and_deletes_original() {
        let root = temp_dir("engine-convert");
        let mut engine = engine_in(&root);
        engine
            .update(|config| {
                config.watch_folder = root.to_string_lossy().to_string();
                config.rules = vec![Rule::new("wav", "mp3")];
                config.settings.delete_original = true;
            })
            .unwrap();
        let wav = root.join("tone (1).wav");
        let synth = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.2",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&wav)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            synth.status.success(),
            "{}",
            String::from_utf8_lossy(&synth.stderr)
        );
        let records = engine.process_path(&wav);
        assert_eq!(records.len(), 1, "{records:?}");
        assert!(records[0].success, "{records:?}");
        assert!(root.join("tone.mp3").metadata().unwrap().len() > 0);
        assert!(!wav.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn moves_into_directory() {
        let root = temp_dir("engine-move");
        let dest = root.join("ready");
        let mut engine = engine_in(&root);
        engine
            .update(|config| {
                config.watch_folder = root.to_string_lossy().to_string();
                config.rules = vec![Rule {
                    id: String::new(),
                    watch_dir: None,
                    extension: Some("pdf".to_string()),
                    name_contains: None,
                    name_pattern: None,
                    action: Action::Move {
                        dest: dest.to_string_lossy().to_string(),
                    },
                    output_dir: None,
                    enabled: true,
                }];
            })
            .unwrap();
        let source = root.join("brief.pdf");
        fs::write(&source, b"%PDF").unwrap();
        let records = engine.process_path(&source);
        assert!(records[0].success);
        assert!(dest.join("brief.pdf").exists());
        assert!(!source.exists());
        let _ = fs::remove_dir_all(root);
    }
}
