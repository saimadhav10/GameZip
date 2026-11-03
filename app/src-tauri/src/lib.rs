mod database;
mod discovery;
mod filesystem;
mod model;
mod scanner;

use database::Database;
use model::{Game, Job, now};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
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
}
struct Engine {
    db: Database,
    active: Mutex<Option<Active>>,
    quitting: AtomicBool,
}
struct AppState(Arc<Engine>);

#[tauri::command]
fn list_games(state: State<'_, AppState>) -> Result<Vec<Game>, String> {
    state.0.db.games()
}

#[tauri::command]
fn list_jobs(state: State<'_, AppState>) -> Result<Vec<Job>, String> {
    state.0.db.jobs()
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
    serde_json::json!({"version":env!("CARGO_PKG_VERSION"), "databasePath":filesystem::display(&state.0.db.path)})
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
        return Err("A scan is already running. Wait for it to finish or cancel it.".into());
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
    };
    engine.db.save_job(&job)?;
    *active = Some(Active {
        id: job.id.clone(),
        game_id: id.clone(),
        cancel: cancel.clone(),
    });
    drop(active);
    let mut work = job.clone();
    std::thread::spawn(move || {
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
        .ok_or("This scan is no longer running.")?;
    job.cancel.store(true, Ordering::Relaxed);
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
                db,
                active: Mutex::new(None),
                quitting: AtomicBool::new(false),
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
            open_game_folder,
            app_info
        ])
        .run(tauri::generate_context!())
        .expect("GameZip could not start");
}
