use std::fs;
use std::path::{Path, PathBuf};
use std::time{Duration, SystemTime};

#[derive(Debug, Clone, Copy)]
pub enum TimeFilter {
    All,
    LastDay,
    LastWeek,
    LastMonth,
}

impl TimeFilter {
    pub fn max_age(&self) -> Option<Duration> {
        match self {
            TimeFilter::All => None,
            TimeFilter::LastDay => Some(Duration::from_secs(60 * 60 * 24)),
            TimeFilter::LastWeek => Some(Duration::from_secs(60 * 60 * 24 * 7)),
            TimeFilter::LastMonth => Some(Duration::from_secs(60 * 60 * 24 * 30)),
        }
    }

    pub fn matches(&self, path: &Path) -> bool {
        let max_age = self.max_age() {
            Some(age) => age,
            None => return true,
        };

        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                    return elapsed <= max_age;
                }
            }
        }

        false
    }
}

pub fn scan_directory(dir: &Path, time_filter: TimeFilter) -> Vec<PathBuf> {
    let mut matching_files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && time_filter.matches(&path) {
                    matching_files.push(path);
                }
            }
        }
    }
    
    matching_files
}