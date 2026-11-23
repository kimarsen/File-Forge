use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out.push(component.as_os_str()),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

pub fn make_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        normalize(&cwd.join(path))
    }
}

pub fn is_within(path: &Path, dir: &Path) -> bool {
    let path = make_absolute(path);
    let dir = make_absolute(dir);
    path != dir && path.starts_with(dir)
}

pub fn unique_path(desired: &Path) -> PathBuf {
    if !desired.exists() {
        return desired.to_path_buf();
    }
    let parent = desired.parent().unwrap_or(Path::new(""));
    let stem = desired
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let ext = desired.extension().and_then(|value| value.to_str());
    for index in 2..10_000 {
        let name = match ext {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    desired.to_path_buf()
}

pub fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| format!("не удалось создать папку: {err}"))?;
    }
    Ok(())
}

pub fn relocate(from: &Path, to: &Path) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    ensure_parent(to)?;
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|err| format!("не удалось скопировать файл: {err}"))?;
            fs::remove_file(from)
                .map_err(|err| format!("не удалось удалить исходный файл: {err}"))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("file-forge-paths-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn normalizes_parent_components() {
        let path = normalize(Path::new("/tmp/a/../b/./c"));
        assert_eq!(path, PathBuf::from("/tmp/b/c"));
    }

    #[test]
    fn within_uses_components() {
        let root = temp_dir();
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("a.txt");
        fs::write(&file, b"a").unwrap();
        assert!(is_within(&file, &root));
        assert!(!is_within(&file, &root.join("nest")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unique_path_skips_existing() {
        let root = temp_dir();
        let desired = root.join("a.txt");
        fs::write(&desired, b"a").unwrap();
        let unique = unique_path(&desired);
        assert_eq!(unique, root.join("a (2).txt"));
        let _ = fs::remove_dir_all(root);
    }
}
