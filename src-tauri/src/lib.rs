use file_forge::Engine;
use file_forge::config::{AppConfig, resolve_config_path};
use file_forge::log::OperationRecord;
use file_forge::rules::PresetKind;
use file_forge::watcher::run_watcher;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_notification::NotificationExt;

struct WatchThread {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

struct AppState {
    engine: Mutex<Engine>,
    watch: Mutex<Option<WatchThread>>,
}

pub fn run() {
    let engine = match Engine::open(resolve_config_path(None)) {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState {
            engine: Mutex::new(engine),
            watch: Mutex::new(None),
        })
        .setup(|app| {
            if let Err(err) = install_tray(app.handle()) {
                tracing::warn!("tray: {err}");
            }
            let enabled = {
                let state = app.state::<AppState>();
                lock_engine(&state).config().settings.autostart
            };
            if enabled && let Err(err) = apply_autostart(app.handle(), true) {
                tracing::warn!("autostart: {err}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            watch_status,
            start_watch,
            stop_watch,
            process_existing,
            process_files,
            recent_log,
            pick_folder,
            add_preset,
            config_path,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("не удалось открыть окно File Forge");
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> AppConfig {
    lock_engine(&state).config().clone()
}

#[tauri::command]
fn save_config(app: AppHandle, config: AppConfig) -> Result<AppConfig, String> {
    let running = watch_running(&app);
    let autostart_before = {
        let state = app.state::<AppState>();
        lock_engine(&state).config().settings.autostart
    };
    {
        let state = app.state::<AppState>();
        lock_engine(&state).replace_config(config)?;
    }
    let autostart_after = {
        let state = app.state::<AppState>();
        lock_engine(&state).config().settings.autostart
    };
    if autostart_before != autostart_after
        && let Err(err) = apply_autostart(&app, autostart_after)
    {
        tracing::warn!("autostart: {err}");
    }
    if running {
        let records = restart_watch(&app)?;
        emit_records(&app, &records);
    }
    let state = app.state::<AppState>();
    Ok(lock_engine(&state).config().clone())
}

#[tauri::command]
fn watch_status(app: AppHandle) -> bool {
    watch_running(&app)
}

#[tauri::command]
async fn start_watch(app: AppHandle) -> Result<Vec<OperationRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || restart_watch(&app))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
fn stop_watch(app: AppHandle) {
    halt_watch(&app);
}

#[tauri::command]
async fn process_existing(app: AppHandle, since: String) -> Result<Vec<OperationRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let filter = file_forge::scanner::TimeFilter::parse(&since)
            .ok_or_else(|| format!("неизвестный период: {since}"))?;
        let state = app.state::<AppState>();
        let mut engine = lock_engine(&state);
        let _ = engine.prepare_watch_dirs()?;
        Ok(engine.process_existing(filter))
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn process_files(app: AppHandle, paths: Vec<String>) -> Result<Vec<OperationRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut engine = lock_engine(&state);
        let mut records = Vec::new();
        for path in paths {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            records.extend(engine.process_path(Path::new(path)));
        }
        Ok(records)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
fn recent_log(state: State<'_, AppState>, limit: usize) -> Result<Vec<OperationRecord>, String> {
    lock_engine(&state).recent_log(limit)
}

#[tauri::command]
fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Выберите папку")
        .blocking_pick_folder()
        .map(file_path_to_string)
}

#[tauri::command]
fn add_preset(
    app: AppHandle,
    kind: String,
    watch: Option<String>,
    output: Option<String>,
) -> Result<AppConfig, String> {
    let kind = PresetKind::parse(&kind)
        .ok_or_else(|| "неизвестный пресет. Используйте audio, video или images".to_string())?;
    {
        let state = app.state::<AppState>();
        let mut engine = lock_engine(&state);
        engine.add_preset(kind, empty_to_none(watch), empty_to_none(output))?;
    }
    if watch_running(&app) {
        let records = restart_watch(&app)?;
        emit_records(&app, &records);
    }
    let state = app.state::<AppState>();
    Ok(lock_engine(&state).config().clone())
}

#[tauri::command]
fn config_path(state: State<'_, AppState>) -> String {
    lock_engine(&state).config_path().display().to_string()
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn restart_watch(app: &AppHandle) -> Result<Vec<OperationRecord>, String> {
    halt_watch(app);
    let (dirs, filter) = {
        let state = app.state::<AppState>();
        let engine = lock_engine(&state);
        (
            engine.prepare_watch_dirs()?,
            engine.config().settings.scan_on_start,
        )
    };
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let watcher_stop = Arc::clone(&stop);
    let thread_stop = Arc::clone(&stop);
    let app_for_thread = app.clone();
    let thread = thread::spawn(move || {
        let watcher = thread::spawn(move || {
            if let Err(err) = run_watcher(dirs, tx, watcher_stop, Some(ready_tx)) {
                tracing::error!("{err}");
            }
        });
        while !thread_stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(path) => {
                    let records = {
                        let state = app_for_thread.state::<AppState>();
                        let mut engine = lock_engine(&state);
                        engine.process_path(&path)
                    };
                    publish(&app_for_thread, &records);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        thread_stop.store(true, Ordering::Relaxed);
        let _ = watcher.join();
    });
    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => {
            let state = app.state::<AppState>();
            *lock_watch(&state) = Some(WatchThread { stop, thread });
        }
        Ok(Err(err)) => {
            stop.store(true, Ordering::Relaxed);
            let _ = thread.join();
            return Err(err);
        }
        Err(_) => {
            stop.store(true, Ordering::Relaxed);
            let _ = thread.join();
            return Err("наблюдение не запустилось".to_string());
        }
    }
    let state = app.state::<AppState>();
    let mut engine = lock_engine(&state);
    Ok(engine.process_existing(filter))
}

fn halt_watch(app: &AppHandle) {
    let running = {
        let state = app.state::<AppState>();
        lock_watch(&state).take()
    };
    if let Some(running) = running {
        running.stop.store(true, Ordering::Relaxed);
        let _ = running.thread.join();
    }
}

fn watch_running(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let watch = lock_watch(&state);
    watch
        .as_ref()
        .is_some_and(|running| !running.thread.is_finished())
}

fn publish(app: &AppHandle, records: &[OperationRecord]) {
    emit_records(app, records);
    if window_visible(app) {
        return;
    }
    for record in records {
        let body = if record.success {
            match &record.destination {
                Some(destination) => format!("{} → {destination}", record.source),
                None => record.source.clone(),
            }
        } else if record.message.is_empty() {
            "операция не выполнена".to_string()
        } else {
            record.message.clone()
        };
        let _ = app
            .notification()
            .builder()
            .title("File Forge")
            .body(body)
            .show();
    }
}

fn emit_records(app: &AppHandle, records: &[OperationRecord]) {
    for record in records {
        let _ = app.emit("operation", record);
    }
}

fn window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(true)
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    }
    .map_err(|err| err.to_string())
}

fn install_tray(app: &AppHandle) -> Result<(), String> {
    let show = MenuItem::with_id(app, "show", "Открыть", true, None::<&str>)
        .map_err(|err| err.to_string())?;
    let toggle = MenuItem::with_id(app, "toggle", "Пауза или продолжить", true, None::<&str>)
        .map_err(|err| err.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)
        .map_err(|err| err.to_string())?;
    let menu = Menu::with_items(app, &[&show, &toggle, &quit]).map_err(|err| err.to_string())?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("File Forge")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => reveal(app),
            "quit" => app.exit(0),
            "toggle" => {
                if watch_running(app) {
                    halt_watch(app);
                } else {
                    let _ = restart_watch(app);
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map_err(|err| err.to_string())?;
    Ok(())
}

fn reveal(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn file_path_to_string(path: FilePath) -> String {
    match path {
        FilePath::Path(path) => path.display().to_string(),
        FilePath::Url(url) => url.to_string(),
    }
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn lock_engine(state: &AppState) -> MutexGuard<'_, Engine> {
    state.engine.lock().unwrap_or_else(|err| err.into_inner())
}

fn lock_watch(state: &AppState) -> MutexGuard<'_, Option<WatchThread>> {
    state.watch.lock().unwrap_or_else(|err| err.into_inner())
}
