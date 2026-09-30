use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
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
    
    println!("Watching folder: {:?}", watch_path);

    for res in rx {
        match res {
            Ok(event) => {
                println!("Event: {:?}", event);
            }
            Err(error) => {
                println!("Error: {:?}", error)
            }
        }
    }

    Ok(())
}
