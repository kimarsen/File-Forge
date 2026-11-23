use crate::paths::ensure_parent;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationRecord {
    pub timestamp: String,
    pub source: String,
    pub destination: Option<String>,
    pub action: String,
    pub success: bool,
    pub message: String,
}

pub fn timestamp_now() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn log_path_for(config_path: &Path) -> std::path::PathBuf {
    config_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("operations.jsonl")
}

pub fn append_record(path: &Path, record: &OperationRecord) -> Result<(), String> {
    ensure_parent(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("не удалось открыть журнал: {err}"))?;
    let line = serde_json::to_string(record)
        .map_err(|err| format!("не удалось записать журнал: {err}"))?;
    writeln!(file, "{line}").map_err(|err| format!("не удалось записать журнал: {err}"))
}

pub fn read_records(path: &Path) -> Result<Vec<OperationRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(path).map_err(|err| format!("не удалось прочитать журнал: {err}"))?;
    let mut records = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<OperationRecord>(line) {
            records.push(record);
        }
    }
    Ok(records)
}

pub fn read_recent(path: &Path, limit: usize) -> Result<Vec<OperationRecord>, String> {
    let mut records = read_records(path)?;
    if limit == 0 {
        return Ok(Vec::new());
    }
    if records.len() > limit {
        records = records.split_off(records.len() - limit);
    }
    Ok(records)
}

pub fn format_record(record: &OperationRecord) -> String {
    let status = if record.success { "ok" } else { "fail" };
    let route = match &record.destination {
        Some(destination) => format!("{} -> {destination}", record.source),
        None => record.source.clone(),
    };
    if record.message.is_empty() {
        format!("[{status}] {} {} {route}", record.timestamp, record.action)
    } else {
        format!(
            "[{status}] {} {} {route}: {}",
            record.timestamp, record.action, record.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_reads_recent() {
        let path = std::env::temp_dir().join(format!(
            "file-forge-log-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let record = OperationRecord {
            timestamp: "2026-11-23T20:00:00+05:00".to_string(),
            source: "a.ogg".to_string(),
            destination: Some("a.mp3".to_string()),
            action: "convert".to_string(),
            success: true,
            message: String::new(),
        };
        append_record(&path, &record).unwrap();
        append_record(&path, &record).unwrap();
        let recent = read_recent(&path, 1).unwrap();
        assert_eq!(recent.len(), 1);
        assert!(format_record(&recent[0]).contains("convert"));
        let _ = fs::remove_file(path);
    }
}
