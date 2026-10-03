use std::path::Path;

#[derive(Debug, Clone)]
pub enum Action {
    Convert { target_format: String },
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub extension: String,
    pub action: Action,
}

impl Rule {
    pub fn new(extension: &str, target_format: &str) -> Self {
        Self {
            extension: extension.to_string(),
            action: Action::Convert {
                target_format: target_format.to_string(),
            },
        }

    }

    pub fn matches(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return ext.eq_ignore_ascii_case(&self.extension);
        }
        false
    }
}