use std::path::Path;

#[derive(Debug, Clone)]
pub enum Action {
    Convert { target_format: String },
    Rename { prefix: String,},
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub extension: Option<String>,
    pub name_contains: Option<String>,
    pub action: Action,
}

impl Rule {
    pub fn new(extension: &str, target_format: &str) -> Self {
        Self {
            extension: Some(extension.to_string()),
            name_contains: None,
            action: Action::Convert {
                target_format: target_format.to_string(),
            },
        }
    }

    pub fn new_rename(extension: &str, prefix: &str) -> Self {
        Self {
            extension: Some(extension.to_string()),
            name_contains: None,
            action: Action::Rename {
                prefix: prefix.to_string(),
            },
        }
    }

    pub fn with_name_filter(mut self, substr: &str) -> Self {
        self.name_contains = Some(substr.to_string());
        self
    }

    pub fn matches(&self, path: &Path) -> bool {
        if let Some(rule_ext) = &self.extension {
            let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !file_ext.eq_ignore_ascii_case(rule_ext) {
                return false;
            }
        }
        if let Some(substr) = &self.name_contains {
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !file_stem.to_lowercase().contains(&substr.to_lowercase()) {
                return false;
            }
        }

        true
    }
}