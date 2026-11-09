use crate::{
    Active, AppState, backups, compression, filesystem, media,
    model::{Job, now},
    scheduling,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoSettings {
    pub tools_directory: Option<String>,
    pub backup_directory: Option<String>,
    pub retention_days: u32,
}
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            tools_directory: None,
            backup_directory: None,
            retention_days: 7,
        }
    }
}

#[tauri::command]
pub fn video_settings(state: State<'_, AppState>) -> Result<VideoSettings, String> {
    state.0.db.setting("video")
}

fn tools(settings: &VideoSettings) -> Result<media::Tools, String> {
    let root = PathBuf::from(
        settings
            .tools_directory
            .as_ref()
            .ok_or("Set up video tools in Settings first.")?,
    );
    let ffmpeg = root.join("ffmpeg.exe");
    let ffprobe = root.join("ffprobe.exe");
    if !root.is_absolute() || !ffmpeg.is_file() || !ffprobe.is_file() {
        return Err("Choose the FFmpeg bin folder containing ffmpeg.exe and ffprobe.exe.".into());
    }
    Ok(media::Tools { ffmpeg, ffprobe })
}

#[tauri::command]
pub async fn save_video_settings(
    mut value: VideoSettings,
    state: State<'_, AppState>,
) -> Result<VideoSettings, String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if value.retention_days < 7 {
            return Err("Keep originals for at least seven days.".into());
        }
        if let Some(path) = &value.tools_directory {
            value.tools_directory = Some(filesystem::display(&filesystem::root(path)?));
            tools(&value)?;
        }
        if let Some(path) = &value.backup_directory {
            let root = filesystem::root(path)?;
            for game in engine.db.games()? {
                if root.starts_with(
                    filesystem::root(&game.path).unwrap_or_else(|_| PathBuf::from(&game.path)),
                ) {
                    return Err("Choose a backup location outside your game folders.".into());
                }
            }
            // The picker chooses a dedicated existing folder. Never move old
            // copies when this setting changes; retain their vault in the registry.
            backups::Vault::open(&root)?;
            value.backup_directory = Some(filesystem::display(&root));
        }
        engine.db.save_video_settings(&value)?;
        Ok(value)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    path: String,
    bytes: u64,
    preview_available: bool,
}

fn candidate_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("Invalid video path.".into());
    }
    let path = fs::canonicalize(root.join(relative)).map_err(|e| e.to_string())?;
    if !path.starts_with(root) {
        return Err("Video moved outside the game folder.".into());
    }
    Ok(path)
}

#[tauri::command]
pub async fn video_candidates(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Candidate>, String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = filesystem::root(&engine.db.game(&id)?.path)?;
        let mut directories = vec![root.clone()];
        let mut found = Vec::new();
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if filesystem::is_link(&metadata) {
                    continue;
                }
                if metadata.is_dir() {
                    directories.push(path);
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if metadata.is_file()
                    && [
                        "mp4", "webm", "bik", "bk2", "usm", "wmv", "avi", "mov", "mkv",
                    ]
                    .contains(&ext.as_str())
                {
                    found.push(Candidate {
                        path: path
                            .strip_prefix(&root)
                            .map_err(|e| e.to_string())?
                            .to_string_lossy()
                            .into(),
                        bytes: metadata.len(),
                        preview_available: ext == "mp4",
                    });
                    if found.len() >= 200 {
                        return Ok(found);
                    }
                }
            }
        }
        found.sort_by_key(|f| std::cmp::Reverse(f.bytes));
        Ok(found)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn start_video_preview(
    id: String,
    path: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Job, String> {
    let engine = state.0.clone();
    let mut active = engine.active.lock().map_err(|e| e.to_string())?;
    if active.is_some()
        || engine.quitting.load(Ordering::Relaxed)
        || engine.updating.load(Ordering::SeqCst)
    {
        return Err("Wait for the current job to finish.".into());
    }
    let game = engine.db.game(&id)?;
    let root = filesystem::root(&game.path)?;
    compression::ensure_game_stopped(&root)?;
    let source = candidate_path(&root, &path)?;
    let tools = tools(&engine.db.setting("video")?)?;
    let destination = engine
        .db
        .path
        .parent()
        .ok_or("Missing data directory")?
        .join("Previews");
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let required = fs::metadata(&source)
        .map_err(|e| e.to_string())?
        .len()
        .saturating_mul(2)
        .saturating_add(256 * 1024 * 1024);
    if compression::free_space(&destination)? < required {
        return Err("Not enough space for a separate preview. Free some space first.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let job = Job {
        id: uuid::Uuid::new_v4().to_string(),
        game_id: id.clone(),
        game_name: game.name,
        state: "running".into(),
        operation: "video_preview".into(),
        message: "Creating a separate 1080p preview. Original video stays in place.".into(),
        started_at: now(),
        total_files: Some(1),
        ..Default::default()
    };
    engine.db.save_job(&job)?;
    *active = Some(Active {
        id: job.id.clone(),
        game_id: id,
        cancel: cancel.clone(),
        pause: None,
    });
    drop(active);
    let mut work = job.clone();
    std::thread::spawn(move || {
        let _background = scheduling::Background::enter();
        let finished = Arc::new(AtomicBool::new(false));
        let (done, stop) = (finished.clone(), cancel.clone());
        let monitor = std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if compression::ensure_game_stopped(&root).is_err() {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
        match tools.preview_1080p(&source, &destination, &cancel) {
            Ok(result) => {
                work.state = "completed".into();
                work.files = 1;
                work.before_bytes = result.original_bytes;
                work.after_bytes = result.output_bytes;
                work.output_path = Some(filesystem::display(&result.path));
                work.message =
                    "1080p preview ready. The game still uses its original video.".into();
            }
            Err(error) => {
                work.state = if error == "cancelled" {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                work.message = if error == "cancelled" {
                    "Preview stopped. Original video was kept.".into()
                } else {
                    error
                };
            }
        }
        finished.store(true, Ordering::Relaxed);
        let _ = monitor.join();
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
pub fn open_job_output(
    id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let jobs = state.0.db.jobs()?;
    let job = jobs
        .iter()
        .find(|j| j.id == id && j.state == "completed" && j.operation == "video_preview")
        .ok_or("Preview is not ready.")?;
    let path = fs::canonicalize(job.output_path.as_ref().ok_or("Missing preview path")?)
        .map_err(|e| e.to_string())?;
    let root = fs::canonicalize(
        state
            .0
            .db
            .path
            .parent()
            .ok_or("Missing data directory")?
            .join("Previews"),
    )
    .map_err(|e| e.to_string())?;
    if !path.starts_with(root) || !path.is_file() {
        return Err("Preview is no longer available.".into());
    }
    app.opener()
        .open_path(filesystem::display(&path), None::<&str>)
        .map_err(|e| e.to_string())
}

fn vault_key(path: &str) -> String {
    format!("{:x}", Sha256::digest(path.as_bytes()))
}

/// Revisit unavailable drives every hour, including once at startup. Each vault
/// independently verifies retained originals and journals deletion; a failed
/// vault must not prevent other vaults from being maintained.
pub(crate) fn expire_locations(paths: &[String], timestamp: u64) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| {
            backups::Vault::open(Path::new(path))
                .and_then(|vault| vault.expire(timestamp))
                .err()
                .map(|error| format!("Backup cleanup postponed: {path}. {error}"))
        })
        .collect()
}

pub(crate) fn start_maintenance(engine: std::sync::Weak<crate::Engine>) {
    std::thread::spawn(move || {
        loop {
            let Some(engine) = engine.upgrade() else {
                break;
            };
            if engine.quitting.load(Ordering::Relaxed) {
                break;
            }
            {
                let Ok(_io) = engine.backup_io.lock() else {
                    break;
                };
                if !engine.updating.load(Ordering::SeqCst) {
                    let _background = scheduling::Background::enter();
                    let warnings = match engine.db.backup_locations() {
                        Ok(paths) => expire_locations(&paths, now()),
                        Err(error) => vec![format!("Cannot check backup locations: {error}")],
                    };
                    if let Ok(mut current) = engine.backup_warnings.lock() {
                        *current = warnings;
                    }
                }
            }
            drop(engine);
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });
}

#[tauri::command]
pub async fn video_backups(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut backups = Vec::new();
        let mut warnings = engine
            .backup_warnings
            .lock()
            .map_err(|e| e.to_string())?
            .clone();
        for path in engine.db.backup_locations()? {
            match backups::Vault::open(Path::new(&path)).and_then(|v| v.list()) {
                Ok(records) => {
                    for record in records {
                        backups
                            .push(serde_json::json!({"vault":vault_key(&path), "record":record}));
                    }
                }
                Err(error) => {
                    warnings.push(format!("Backup location unavailable: {path}. {error}"))
                }
            }
        }
        Ok(serde_json::json!({"backups":backups,"warnings":warnings}))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_video_backup(
    vault: String,
    id: String,
    destination: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _io = engine.backup_io.lock().map_err(|e| e.to_string())?;
        if engine.updating.load(Ordering::SeqCst) {
            return Err("Wait for the app update to finish.".into());
        }
        let _background = scheduling::Background::enter();
        let root = engine
            .db
            .backup_locations()?
            .into_iter()
            .find(|p| vault_key(p) == vault)
            .ok_or("Unknown backup location")?;
        backups::Vault::open(Path::new(&root))?.export(&id, Path::new(&destination))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_cannot_escape_game_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        fs::write(root.join("sample.mp4"), b"fixture").unwrap();
        assert!(candidate_path(&root, "sample.mp4").is_ok());
        assert!(candidate_path(&root, "../private.mp4").is_err());
        assert!(candidate_path(&root, "C:\\private.mp4").is_err());
    }

    #[test]
    fn cleanup_retries_reconnected_vaults_and_keeps_incomplete_copies() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        fs::create_dir(&root).unwrap();
        let source = temp.path().join("fixture.mp4");
        fs::write(&source, b"original").unwrap();
        let vault = backups::Vault::open(&root).unwrap();
        let complete = vault.preserve(&source, 7).unwrap();
        let incomplete = vault.preserve(&source, 7).unwrap();
        fs::write(&source, b"converted").unwrap();
        vault
            .complete(
                &complete.id,
                &format!("{:x}", Sha256::digest(b"converted")),
                1,
            )
            .unwrap();
        drop(vault);
        let paths = vec![root.to_string_lossy().into_owned()];
        let disconnected = temp.path().join("disconnected");
        fs::rename(&root, &disconnected).unwrap();
        assert_eq!(expire_locations(&paths, 700_000).len(), 1);
        fs::rename(&disconnected, &root).unwrap();
        assert!(expire_locations(&paths, 700_000).is_empty());
        let records = backups::Vault::open(&root).unwrap().list().unwrap();
        assert_eq!(
            records.iter().find(|r| r.id == complete.id).unwrap().state,
            backups::State::Expired
        );
        assert_eq!(
            records
                .iter()
                .find(|r| r.id == incomplete.id)
                .unwrap()
                .state,
            backups::State::Ready
        );
        assert_eq!(fs::read(&source).unwrap(), b"converted");
    }
}
