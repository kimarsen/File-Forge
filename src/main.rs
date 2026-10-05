mod converter;
mod rules;

use notify::event::EventKind;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use rules::{Action, Rule};
use std::path::Path;
use std::sync::mpsc::channel;

fn main() -> notify::Result<()> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )?;

    let watch_path = Path::new("./test_watch");
    if !watch_path.exists() {
        std::fs::create_dir(watch_path)?;
    }

    watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

    let rules = vec![
        Rule::new("ogg", "mp3"),
        Rule::new("wav", "mp3"),
    ];
    
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

                            for rule in &rules {
                                if rule.matches(path) {
                                    match &rule.action {
                                        Action::Convert { target_format } => {
                                            if let Err(err) = converter::convert(path, target_format) {
                                                eprintln!("Conversion error: {}", err);
                                            }
                                        }
                                    }
                                }
                            }
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
