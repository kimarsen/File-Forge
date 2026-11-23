use notify::event::{AccessKind, AccessMode, EventKind, ModifyKind};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const STABLE_FOR: Duration = Duration::from_millis(300);
const MAX_WAIT: Duration = Duration::from_secs(600);

struct Pending {
    last_size: Option<u64>,
    since: Instant,
    started: Instant,
}

pub struct WatchSession {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    rx: Receiver<PathBuf>,
}

impl WatchSession {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<PathBuf, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }
}

impl Drop for WatchSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn spawn_watcher(dirs: Vec<PathBuf>) -> Result<WatchSession, String> {
    if dirs.is_empty() {
        return Err("нет папок для наблюдения".to_string());
    }
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let join = thread::spawn(move || {
        if let Err(err) = run_watcher(dirs, tx, stop_flag, Some(ready_tx)) {
            tracing::error!("{err}");
        }
    });
    let session = WatchSession {
        stop,
        join: Some(join),
        rx,
    };
    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => Ok(session),
        Ok(Err(err)) => {
            drop(session);
            Err(err)
        }
        Err(_) => {
            drop(session);
            Err("наблюдение не запустилось".to_string())
        }
    }
}

pub fn run_watcher(
    dirs: Vec<PathBuf>,
    tx: Sender<PathBuf>,
    stop: Arc<AtomicBool>,
    ready: Option<Sender<Result<(), String>>>,
) -> Result<(), String> {
    let (event_tx, event_rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = event_tx.send(result);
        },
        Config::default(),
    )
    .map_err(|err| format!("не удалось запустить наблюдение: {err}"))?;

    let mut watched = 0usize;
    let mut errors = Vec::new();
    for dir in &dirs {
        match watcher.watch(dir, RecursiveMode::NonRecursive) {
            Ok(()) => watched += 1,
            Err(err) => errors.push(format!("{}: {err}", dir.display())),
        }
    }
    if watched == 0 {
        let err = if errors.is_empty() {
            "нет папок для наблюдения".to_string()
        } else {
            errors.join("; ")
        };
        if let Some(ready) = ready {
            let _ = ready.send(Err(err.clone()));
        }
        return Err(err);
    }
    if let Some(ready) = ready {
        let _ = ready.send(Ok(()));
    }
    tracing::info!("watching {watched} folders");

    let mut pending: HashMap<PathBuf, Pending> = HashMap::new();
    let mut recent: HashMap<PathBuf, Instant> = HashMap::new();
    while !stop.load(Ordering::Relaxed) {
        match event_rx.recv_timeout(Duration::from_millis(150)) {
            Ok(Ok(event)) => {
                if considers(&event.kind) {
                    for path in event.paths {
                        note_path(&mut pending, &path);
                    }
                }
            }
            Ok(Err(err)) => tracing::warn!("watch event: {err}"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if !poll_pending(&mut pending, &mut recent, &tx) {
            break;
        }
    }
    Ok(())
}

fn considers(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Access(AccessKind::Close(AccessMode::Write | AccessMode::Any))
    )
}

fn note_path(pending: &mut HashMap<PathBuf, Pending>, path: &Path) {
    if path.is_dir() || is_partial(path) {
        return;
    }
    let now = Instant::now();
    let size = fs::metadata(path).ok().map(|metadata| metadata.len());
    pending
        .entry(path.to_path_buf())
        .and_modify(|state| {
            if state.last_size != size {
                state.last_size = size;
                state.since = now;
            }
        })
        .or_insert(Pending {
            last_size: size,
            since: now,
            started: now,
        });
}

fn poll_pending(
    pending: &mut HashMap<PathBuf, Pending>,
    recent: &mut HashMap<PathBuf, Instant>,
    tx: &Sender<PathBuf>,
) -> bool {
    let now = Instant::now();
    recent.retain(|_, seen| now.duration_since(*seen) < Duration::from_secs(5));
    let mut ready = Vec::new();
    pending.retain(|path, state| {
        if state.started.elapsed() > MAX_WAIT {
            return false;
        }
        if !path.exists() {
            return false;
        }
        if !path.is_file() {
            return true;
        }
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        let size = metadata.len();
        if state.last_size != Some(size) {
            state.last_size = Some(size);
            state.since = now;
            return true;
        }
        if state.since.elapsed() < STABLE_FOR {
            return true;
        }
        ready.push(path.clone());
        false
    });
    for path in ready {
        if recent.contains_key(&path) {
            continue;
        }
        recent.insert(path.clone(), now);
        if tx.send(path).is_err() {
            return false;
        }
    }
    true
}

fn is_partial(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    const SUFFIXES: &[&str] = &[
        ".crdownload",
        ".part",
        ".partial",
        ".download",
        ".filepart",
        ".tmp",
    ];
    lower.starts_with("~$") || SUFFIXES.iter().any(|suffix| lower.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_stable_file_and_ignores_partials() {
        let root = std::env::temp_dir().join(format!(
            "file-forge-watch-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let session = spawn_watcher(vec![root.clone()]).unwrap();
        let partial = root.join("song.ogg.crdownload");
        let ready = root.join("song.ogg");
        fs::write(&partial, b"partial").unwrap();
        thread::sleep(Duration::from_millis(500));
        fs::write(&ready, b"audio").unwrap();
        let deadline = Instant::now() + Duration::from_secs(6);
        let mut saw_ready = false;
        let mut saw_partial = false;
        while Instant::now() < deadline {
            match session.recv_timeout(Duration::from_millis(200)) {
                Ok(path) if path == ready => saw_ready = true,
                Ok(path) if path == partial => saw_partial = true,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    if saw_ready {
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if saw_ready {
                break;
            }
        }
        drop(session);
        assert!(saw_ready);
        assert!(!saw_partial);
        let _ = fs::remove_dir_all(root);
    }
}
