use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeFilter {
    All,
    #[default]
    LastDay,
    LastWeek,
    LastMonth,
}

impl TimeFilter {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" | "any" | "все" => Some(Self::All),
            "day" | "last-day" | "last_day" | "lastday" | "сутки" => Some(Self::LastDay),
            "week" | "last-week" | "last_week" | "неделя" => Some(Self::LastWeek),
            "month" | "last-month" | "last_month" | "месяц" => Some(Self::LastMonth),
            _ => None,
        }
    }

    pub fn max_age(&self) -> Option<Duration> {
        match self {
            Self::All => None,
            Self::LastDay => Some(Duration::from_secs(60 * 60 * 24)),
            Self::LastWeek => Some(Duration::from_secs(60 * 60 * 24 * 7)),
            Self::LastMonth => Some(Duration::from_secs(60 * 60 * 24 * 30)),
        }
    }

    pub fn matches(&self, path: &Path) -> bool {
        let Some(max_age) = self.max_age() else {
            return true;
        };
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        let Ok(modified) = metadata.modified() else {
            return false;
        };
        match SystemTime::now().duration_since(modified) {
            Ok(elapsed) => elapsed <= max_age,
            Err(_) => true,
        }
    }
}

pub fn scan_directory(dir: &Path, time_filter: TimeFilter) -> Vec<PathBuf> {
    let mut matching_files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return matching_files;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.is_file() && time_filter.matches(&path) {
            matching_files.push(path);
        }
    }
    matching_files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("file-forge-scan-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn filters_by_age() {
        let root = temp_dir();
        let fresh = root.join("fresh.txt");
        let old = root.join("old.txt");
        fs::write(&fresh, b"new").unwrap();
        fs::write(&old, b"old").unwrap();
        let status = Command::new("touch")
            .args(["-d", "3 days ago"])
            .arg(&old)
            .status()
            .unwrap();
        assert!(status.success());
        let day = scan_directory(&root, TimeFilter::LastDay);
        assert!(day.iter().any(|path| path == &fresh));
        assert!(!day.iter().any(|path| path == &old));
        let week = scan_directory(&root, TimeFilter::LastWeek);
        assert!(week.iter().any(|path| path == &old));
        let _ = fs::remove_dir_all(root);
    }
}
