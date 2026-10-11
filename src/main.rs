mod config;
mod converter;
mod renamer;
mod rules;
mod scanner;

use config::AppConfig;
use notify::event::EventKind;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use rules::{Action, Rule};
use scanner::TimeFilter;
use std::path::Path;
use std::sync::mpsc::channel;

fn process_file(path: &Path, rules: &[Rule]) {
    for rule in rules {
        if rule.matches(path) {
            match &rule.action {
                Action::Convert { target_format } => {
                    if let Err(err) = converter::convert(path, target_format) {
                        eprintln!("Conversion error: {}", err);
                    }
                }
                Action::Rename { prefix } => {
                    if let Err(err) = renamer::rename_with_prefix(path, prefix) {
                        eprintln!("Renaming error: {}", err);
                    }
                }
            }
        }
    }
}

fn main() -> notify::Result<()> {

    let config_path = Path::new("config.json");
    let config = AppConfig::load_or_create(config_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let watch_path = Path::new(&config.watch_folder);
    if !watch_path.exists() {
        std::fs::create_dir_all(watch_path)?;
    }

    let rules = config.rules;

    println!("Scanning existing files in folder: {:?}", watch_path);
    let existing_files = scanner::scan_directory(watch_path, TimeFilter::LastDay);
    println!("Found {} files matching time filter.", existing_files.len());

    for path in &existing_files {
        process_file(path, &rules);
    }

    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )?;

    watcher.watch(watch_path, RecursiveMode::NonRecursive)?;
    
    println!("Watching folder: {:?}", watch_path);

    for res in rx {
        match res {
            Ok(event) => {
                if let EventKind::Create(_) = event.kind {
                    for path in &event.paths {
                        if path.is_file() {
                            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            println!("New file detected: {}", file_name);
                            println!("File extension: {}", extension);
                            println!("Full path: {:?}", path);

                            process_file(path, &rules);
                        }
                    }
                }
            }
            Err(error) => {
                println!("Error: {:?}", error);
            }
        }
    }

    Ok(())
}
