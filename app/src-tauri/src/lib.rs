pub mod backups;
pub mod compression;
mod compression_job;
mod database;
mod discovery;
mod filesystem;
pub mod media;
mod model;
mod scanner;
mod scheduling;

use database::Database;
use model::{Game, Job, now};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tauri::{
    Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_opener::OpenerExt;

struct Active {
    id: String,
    game_id: String,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<scheduling::Pause>>,
}
struct Engine {
    db: Database,
    performance: scheduling::Policy,
    active: Mutex<Option<Active>>,
    quitting: AtomicBool,
    prepared: Mutex<Option<Prepared>>,
    preparation_generation: AtomicU64,
}
struct Prepared {
    token: String,
    game_id: String,
    plan: compression::Plan,
}
struct AppState(Arc<Engine>);

#[tauri::command]
fn list_games(state: State<'_, AppState>) -> Result<Vec<Game>, String> {
    state.0.db.games()
}

#[tauri::command]
fn list_jobs(state: State<'_, AppState>) -> Result<Vec<Job>, String> {
    let active = state.0.active.lock().map_err(|e| e.to_string())?;
    let mut jobs = state.0.db.jobs()?;
    if let Some(active) = active
        .as_ref()
        .filter(|active| active.cancel.load(Ordering::Relaxed))
        && let Some(job) = jobs
            .iter_mut()
            .find(|job| job.id == active.id && job.state == "running")
    {
        job.state = "stopping".into();
        job.message = "Stopping after the current file".into();
    }
    if let Some(active) = active.as_ref()
        && !active.cancel.load(Ordering::Relaxed)
        && let Some(pause) = &active.pause
        && pause.requested.load(Ordering::Relaxed)
        && let Some(job) = jobs
            .iter_mut()
            .find(|j| j.id == active.id && j.state == "running")
    {
        let waiting = pause.waiting.load(Ordering::Relaxed);
        job.state = if waiting { "paused" } else { "pausing" }.into();
        job.message = if waiting {
            "Paused. Resume when you are ready."
        } else {
            "Pausing after the current file"
        }
        .into();
    }
    if let Some(active) = active.as_ref()
        && !active.cancel.load(Ordering::Relaxed)
        && active
            .pause
            .as_ref()
            .is_some_and(|p| p.waiting_idle.load(Ordering::Relaxed))
        && let Some(job) = jobs
            .iter_mut()
            .find(|j| j.id == active.id && j.state == "running")
    {
        job.state = "waiting".into();
        job.message = "Waiting for two minutes without keyboard or mouse input. Change Idle-only in Settings to continue now.".into();
    }
    Ok(jobs)
}

#[tauri::command]
fn add_game(
    name: String,
    path: String,
    launcher: String,
    state: State<'_, AppState>,
) -> Result<Game, String> {
    if !["Manual", "Steam", "Epic"].contains(&launcher.as_str()) {
        return Err("Unknown launcher.".into());
    }
    state.0.db.add(&name, &path, &launcher)
}

#[tauri::command]
fn remove_game(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let active = state.0.active.lock().map_err(|e| e.to_string())?;
    if active.as_ref().is_some_and(|a| a.game_id == id) {
        return Err("Stop the active scan before removing this game.".into());
    }
    state.0.db.remove(&id)
}

#[tauri::command]
async fn discover_games() -> Result<model::Discovery, String> {
    tauri::async_runtime::spawn_blocking(discovery::discover)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_game_folder(
    id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let game = state.0.db.game(&id)?;
    let path = filesystem::root(&game.path)?;
    app.opener()
        .open_path(filesystem::display(&path), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({"version":env!("CARGO_PKG_VERSION"), "databasePath":filesystem::display(&state.0.db.path), "performance":state.0.performance.get()})
}

#[tauri::command]
fn set_performance(
    value: scheduling::Preferences,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Serialize settings changes with each other and job startup; save first so
    // a failed disk write never makes the UI promise a persisted preference.
    let _active = state.0.active.lock().map_err(|e| e.to_string())?;
    state.0.db.save_performance(value)?;
    state.0.performance.set(value);
    Ok(())
}

#[tauri::command]
async fn prepare_compression(
    id: String,
    operation: compression::Operation,
    strength: Option<compression::Strength>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let engine = state.0.clone();
    let generation = engine.preparation_generation.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        if engine.active.lock().map_err(|e| e.to_string())?.is_some() { return Err("Wait for the current job to finish.".into()); }
        let _background = scheduling::Background::enter();
        let game = engine.db.game(&id)?;
        let root = filesystem::root(&game.path)?;
        let mut plan = compression::prepare(&root, operation, &AtomicBool::new(false))?;
        plan.strength = strength.unwrap_or_default();
        let token = uuid::Uuid::new_v4().to_string();
        let preview = serde_json::json!({"token":token, "operation":operation, "files":plan.files.len(), "skipped":plan.skipped, "allocatedBytes":plan.allocated, "freeBytes":plan.free, "requiredFreeBytes":plan.required_free});
        let mut prepared = engine.prepared.lock().map_err(|e| e.to_string())?;
        if engine.preparation_generation.load(Ordering::SeqCst) != generation { return Err("Preparation was replaced by a newer request.".into()); }
        *prepared = Some(Prepared { token, game_id: id, plan });
        Ok(preview)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn start_compression(
    token: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Job, String> {
    let engine = state.0.clone();
    let mut active = engine.active.lock().map_err(|e| e.to_string())?;
    if active.is_some() || engine.quitting.load(Ordering::Relaxed) {
        return Err("Wait for the current job to finish.".into());
    }
    let mut prepared = engine.prepared.lock().map_err(|e| e.to_string())?;
    let ready = prepared
        .as_ref()
        .filter(|p| p.token == token)
        .ok_or("Preparation expired. Check the files again.")?;
    let game = engine.db.game(&ready.game_id)?;
    if filesystem::root(&game.path)? != ready.plan.root {
        return Err("The installation folder changed. Check the files again.".into());
    }
    compression::ensure_game_stopped(&ready.plan.root)?;
    if ready.plan.files.is_empty() {
        return Err("No eligible files need this operation.".into());
    }
    if compression::free_space(&ready.plan.root)? < ready.plan.required_free {
        return Err("Not enough free space. Free some space and check the files again.".into());
    }
    let job = Job {
        id: uuid::Uuid::new_v4().to_string(),
        game_id: game.id.clone(),
        game_name: game.name,
        state: "running".into(),
        operation: match ready.plan.operation {
            compression::Operation::Compress => "compress",
            compression::Operation::Decompress => "decompress",
        }
        .into(),
        total_files: Some(ready.plan.files.len() as u64),
        started_at: now(),
        message: "Preparing to process files".into(),
        ..Default::default()
    };
    engine.db.begin_compression(&job, &ready.plan)?;
    let ready = prepared.take().unwrap();
    let pause = Arc::new(scheduling::Pause::default());
    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(Active {
        id: job.id.clone(),
        game_id: game.id,
        pause: Some(pause.clone()),
        cancel: cancel.clone(),
    });
    drop(active);
    drop(prepared);
    let work = job.clone();
    std::thread::spawn(move || {
        let _background = scheduling::Background::enter();
        let finished = compression_job::run_scheduled(
            &engine.db,
            &ready.plan,
            work,
            &cancel,
            &pause,
            &engine.performance,
            |job| {
                let _ = app.emit("scan-progress", job);
            },
        );
        if let Ok(mut active) = engine.active.lock() {
            *active = None;
        }
        let _ = app.emit("scan-progress", &finished);
        if engine.quitting.load(Ordering::Relaxed) {
            app.exit(0);
        }
    });
    Ok(job)
}

#[tauri::command]
fn start_scan(
    id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Job, String> {
    let engine = state.0.clone();
    let mut active = engine.active.lock().map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("A job is already running. Wait for it to finish or cancel it.".into());
    }
    if engine.quitting.load(Ordering::Relaxed) {
        return Err("GameZip is closing.".into());
    }
    let game = engine.db.game(&id)?;
    let root = filesystem::root(&game.path)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let job = Job {
        id: uuid::Uuid::new_v4().to_string(),
        game_id: game.id,
        game_name: game.name,
        state: "running".into(),
        files: 0,
        logical_bytes: 0,
        message: "Reading game files".into(),
        started_at: now(),
        finished_at: None,
        operation: "scan".into(),
        ..Default::default()
    };
    engine.db.save_job(&job)?;
    *active = Some(Active {
        id: job.id.clone(),
        game_id: id.clone(),
        pause: None,
        cancel: cancel.clone(),
    });
    drop(active);
    let mut work = job.clone();
    std::thread::spawn(move || {
        let _background = scheduling::Background::enter();
        let result = scanner::scan(&root, &cancel, |files, bytes| {
            work.files = files;
            work.logical_bytes = bytes;
            if cancel.load(Ordering::Relaxed) {
                work.state = "stopping".into();
            }
            let _ = engine.db.save_job(&work);
            let _ = app.emit("scan-progress", &work);
        });
        match result {
            Ok(report) => match engine.db.save_scan(&id, &report) {
                Ok(()) => {
                    work.state = "completed".into();
                    work.message = if report.skipped > 0 || report.allocation_unknown > 0 {
                        "Scan complete with unavailable items. Review details.".into()
                    } else {
                        "Scan complete".into()
                    };
                }
                Err(e) => {
                    work.state = "failed".into();
                    work.message = e;
                }
            },
            Err(error) => {
                work.state = if error == "cancelled" {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                work.message = if error == "cancelled" {
                    "Scan cancelled. Previous results were kept.".into()
                } else {
                    error
                };
            }
        }
        work.finished_at = Some(now());
        work = engine.db.finish_job(work);
        if let Ok(mut active) = engine.active.lock() {
            *active = None;
        }
        let _ = app.emit("scan-progress", &work);
        if engine.quitting.load(Ordering::Relaxed) {
            app.exit(0);
        }
    });
    Ok(job)
}

#[tauri::command]
fn cancel_scan(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let active = state.0.active.lock().map_err(|e| e.to_string())?;
    let job = active
        .as_ref()
        .filter(|a| a.id == id)
        .ok_or("This job is no longer running.")?;
    job.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn pause_job(id: String, paused: bool, state: State<'_, AppState>) -> Result<(), String> {
    let active = state.0.active.lock().map_err(|e| e.to_string())?;
    let job = active
        .as_ref()
        .filter(|a| a.id == id)
        .ok_or("This job is no longer running.")?;
    if job.cancel.load(Ordering::Relaxed) {
        return Err("This job is stopping.".into());
    }
    let pause = job.pause.as_ref().ok_or("Scanning cannot be paused.")?;
    pause.requested.store(paused, Ordering::Relaxed);
    Ok(())
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            // Isolate native integration tests from the user's library. Release
            // binaries always use the standard application data directory.
            #[cfg(debug_assertions)]
            let directory = std::env::var_os("GAMEZIP_TEST_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(directory);
            std::fs::create_dir_all(&directory)?;
            let db =
                Database::open(directory.join("gamezip.sqlite")).map_err(std::io::Error::other)?;
            app.manage(AppState(Arc::new(Engine {
                performance: scheduling::Policy::new(
                    db.performance().map_err(std::io::Error::other)?,
                ),
                db,
                active: Mutex::new(None),
                quitting: AtomicBool::new(false),
                prepared: Mutex::new(None),
                preparation_generation: AtomicU64::new(0),
            })));
            let open = MenuItem::with_id(app, "open", "Open GameZip", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit GameZip", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("GameZip")
                .show_menu_on_left_click(false);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "open" => show_window(app),
                "quit" => {
                    let engine = &app.state::<AppState>().0;
                    engine.quitting.store(true, Ordering::Relaxed);
                    let active = engine.active.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(job) = active.as_ref() {
                        job.cancel.store(true, Ordering::Relaxed);
                    } else {
                        app.exit(0);
                    }
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                    show_window(tray.app_handle());
                }
            })
            .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.hide().is_ok()
            {
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            list_jobs,
            add_game,
            remove_game,
            discover_games,
            start_scan,
            cancel_scan,
            pause_job,
            open_game_folder,
            app_info,
            set_performance,
            prepare_compression,
            start_compression
        ])
        .run(tauri::generate_context!())
        .expect("GameZip could not start");
}
