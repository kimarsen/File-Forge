use crate::paths::{relocate, unique_path};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct RenameSpec {
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub find: Option<String>,
    #[serde(default)]
    pub replace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameContext {
    pub date: String,
    pub time: String,
    pub counter: u64,
}

impl NameContext {
    pub fn now(counter: u64) -> Self {
        let now = chrono::Local::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            time: now.format("%H-%M-%S").to_string(),
            counter,
        }
    }
}

pub fn rename_with_prefix(input_path: &Path, prefix: &str) -> Result<(), String> {
    let spec = RenameSpec {
        prefix: Some(prefix.to_string()),
        ..RenameSpec::default()
    };
    let mut context = NameContext::now(0);
    rename_file(input_path, &spec, None, &mut context)?;
    Ok(())
}

pub fn rename_file(
    source: &Path,
    spec: &RenameSpec,
    output_dir: Option<&Path>,
    context: &mut NameContext,
) -> Result<Option<PathBuf>, String> {
    let parent = source.parent().unwrap_or(Path::new(""));
    let original_stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "имя файла не в UTF-8".to_string())?;
    let ext = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let dir = output_dir.unwrap_or(parent);
    let start_counter = context.counter;
    let (mut file_name, used_counter) = compose_name(original_stem, ext, spec, context);
    if file_name.trim().is_empty() || file_name == "." || file_name == ".." {
        return Err("шаблон дал пустое имя".to_string());
    }
    let mut destination = dir.join(&file_name);
    if used_counter {
        while destination.exists() && destination != source {
            context.counter += 1;
            if context.counter > start_counter + 10_000 {
                break;
            }
            let (next_name, _) = compose_name(original_stem, ext, spec, context);
            file_name = next_name;
            destination = dir.join(&file_name);
        }
        context.counter += 1;
    } else if destination.exists() && destination != source {
        destination = unique_path(&destination);
    }
    if destination == source {
        return Ok(None);
    }
    relocate(source, &destination)?;
    Ok(Some(destination))
}

fn compose_name(
    original_stem: &str,
    ext: &str,
    spec: &RenameSpec,
    context: &NameContext,
) -> (String, bool) {
    let mut used_counter = false;
    let mut stem = original_stem.to_string();
    if let Some(find) = spec.find.as_ref().filter(|value| !value.is_empty()) {
        let replacement = spec.replace.as_deref().unwrap_or("");
        stem = stem.replace(find, replacement);
    }
    if let Some(pattern) = spec.pattern.as_ref().filter(|value| !value.is_empty()) {
        stem = render(pattern, &stem, ext, context, &mut used_counter);
    }
    if let Some(suffix) = spec.suffix.as_ref().filter(|value| !value.is_empty())
        && !stem.ends_with(suffix.as_str())
    {
        stem.push_str(suffix);
    }
    let mut file_name = if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{ext}")
    };
    if let Some(prefix) = spec.prefix.as_ref().filter(|value| !value.is_empty())
        && !file_name.starts_with(prefix.as_str())
    {
        file_name = format!("{prefix}{file_name}");
    }
    (file_name, used_counter)
}

fn render(
    pattern: &str,
    name: &str,
    ext: &str,
    context: &NameContext,
    used_counter: &mut bool,
) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '{'
            && let Some(end) = chars[index + 1..].iter().position(|ch| *ch == '}')
        {
            let key: String = chars[index + 1..index + 1 + end].iter().collect();
            let key = key.trim();
            let value = match key {
                "name" => name.to_string(),
                "date" => context.date.clone(),
                "time" => context.time.clone(),
                "ext" => ext.to_string(),
                "counter" => {
                    *used_counter = true;
                    context.counter.to_string()
                }
                _ => format!("{{{key}}}"),
            };
            out.push_str(&value);
            index += end + 2;
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("file-forge-rename-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn context() -> NameContext {
        NameContext {
            date: "2026-11-23".to_string(),
            time: "20-15-00".to_string(),
            counter: 4,
        }
    }

    #[test]
    fn adds_prefix_once() {
        let root = temp_dir();
        let source = root.join("report.txt");
        fs::write(&source, b"hi").unwrap();
        let spec = RenameSpec {
            prefix: Some("DONE_".to_string()),
            ..RenameSpec::default()
        };
        let mut ctx = context();
        let dest = rename_file(&source, &spec, None, &mut ctx)
            .unwrap()
            .unwrap();
        assert_eq!(dest, root.join("DONE_report.txt"));
        assert!(rename_file(&dest, &spec, None, &mut ctx).unwrap().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replaces_suffix_and_template() {
        let root = temp_dir();
        let source = root.join("voice.ogg");
        fs::write(&source, b"hi").unwrap();
        let spec = RenameSpec {
            find: Some("voice".to_string()),
            replace: Some("song".to_string()),
            suffix: Some("_final".to_string()),
            pattern: Some("{date}_{counter}_{name}".to_string()),
            ..RenameSpec::default()
        };
        let mut ctx = context();
        let dest = rename_file(&source, &spec, None, &mut ctx)
            .unwrap()
            .unwrap();
        assert_eq!(dest, root.join("2026-11-23_4_song_final.ogg"));
        assert_eq!(ctx.counter, 5);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn counter_skips_existing_name() {
        let root = temp_dir();
        fs::write(root.join("1_a.txt"), b"old").unwrap();
        let source = root.join("a.txt");
        fs::write(&source, b"new").unwrap();
        let spec = RenameSpec {
            pattern: Some("{counter}_{name}".to_string()),
            ..RenameSpec::default()
        };
        let mut ctx = NameContext {
            date: "2026-11-23".to_string(),
            time: "00-00-00".to_string(),
            counter: 1,
        };
        let dest = rename_file(&source, &spec, None, &mut ctx)
            .unwrap()
            .unwrap();
        assert_eq!(dest, root.join("2_a.txt"));
        assert_eq!(ctx.counter, 3);
        let _ = fs::remove_dir_all(root);
    }
}
