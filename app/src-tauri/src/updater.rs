//! Signed, user-initiated updates. All installation paths share the native job
//! gate; the frontend never receives plugin install permissions or artifact URLs.
use crate::{AppState, Engine};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tauri::{Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub version: Option<String>,
    pub message: String,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            state: "unchecked".into(),
            version: None,
            message: "Updates have not been checked yet.".into(),
        }
    }
}
#[derive(Default)]
pub struct Updates {
    status: Mutex<Status>,
    pending: Mutex<Option<Update>>,
    checking: AtomicBool,
}
impl Updates {
    fn set(&self, state: &str, version: Option<String>, message: &str) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = Status {
            state: state.into(),
            version,
            message: message.into(),
        };
    }
    fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
struct Checking<'a>(&'a AtomicBool);
impl Drop for Checking<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn approved_download(url: &str) -> bool {
    url.starts_with("https://github.com/saimadhav10/GameZip/releases/download/")
        && !url.contains(['?', '#'])
}

async fn check(app: &tauri::AppHandle, engine: &Arc<Engine>) -> Result<Status, String> {
    let updates = &engine.updates;
    if engine.updating.load(Ordering::SeqCst) || updates.checking.swap(true, Ordering::SeqCst) {
        return Ok(updates.status());
    }
    let _checking = Checking(&updates.checking);
    updates.set("checking", None, "Checking for updates…");
    let outcome = async {
        app.updater_builder()
            .timeout(Duration::from_secs(20))
            .build()?
            .check()
            .await
    }
    .await;
    // Install claims the checking flag before taking a pending update, so a
    // late network check cannot overwrite the install state or selected version.
    let mut pending = updates.pending.lock().map_err(|e| e.to_string())?;
    *pending = None;
    match outcome {
        Ok(Some(update)) if approved_download(update.download_url.as_str()) => {
            updates.set(
                "available",
                Some(update.version.clone()),
                "An update is ready to download.",
            );
            *pending = Some(update);
        }
        Ok(Some(_)) => updates.set(
            "unavailable",
            None,
            "The update download address was not recognised. Nothing was installed.",
        ),
        Ok(None) => updates.set("current", None, "You have the latest published version."),
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => updates.set(
            "unavailable",
            None,
            "The update feed is unavailable or has not been published yet.",
        ),
        Err(_) => updates.set(
            "unavailable",
            None,
            "Could not check for updates. Check your connection and try again.",
        ),
    }
    Ok(updates.status())
}

#[tauri::command]
pub fn update_status(state: State<'_, AppState>) -> Status {
    state.0.updates.status()
}
#[tauri::command]
pub async fn check_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Status, String> {
    check(&app, &state.0).await
}

struct InstallReservation(Arc<Engine>);
impl InstallReservation {
    fn acquire(engine: Arc<Engine>) -> Result<Self, String> {
        {
            let active = engine.active.lock().map_err(|e| e.to_string())?;
            if active.is_some() || engine.quitting.load(Ordering::SeqCst) {
                return Err("Finish or cancel the current job before installing an update.".into());
            }
            if engine.updating.swap(true, Ordering::SeqCst) {
                return Err("An update is already being installed.".into());
            }
        }
        Ok(Self(engine))
    }
}
impl Drop for InstallReservation {
    fn drop(&mut self) {
        self.0.updating.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn install_update(version: String, state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.0.clone();
    if engine.updates.checking.swap(true, Ordering::SeqCst) {
        return Err("Wait for the update check to finish.".into());
    }
    let _checking = Checking(&engine.updates.checking);
    let reservation = InstallReservation::acquire(engine.clone())?;
    let update = engine
        .updates
        .pending
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .filter(|u| u.version == version)
        .cloned()
        .ok_or("Check for updates again before installing.")?;
    engine.updates.set(
        "installing",
        Some(version),
        "Downloading and verifying the update…",
    );
    let result = async {
        // The plugin verifies the embedded-key signature before returning bytes.
        let mut update = update;
        update.timeout = Some(Duration::from_secs(600));
        let bytes = update
            .download(|_, _| {}, || {})
            .await
            .map_err(|e| format!("Update download or verification failed: {e}"))?;
        let engine = engine.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _reservation = reservation;
            // Let an already-started original export/cleanup reach completion.
            // New backup I/O refuses to start while updating is set.
            let _backup_io = engine.backup_io.lock().map_err(|e| e.to_string())?;
            engine.updates.set(
                "installing",
                Some(update.version.clone()),
                "Installing the verified update. GameZip will restart.",
            );
            update
                .install(bytes)
                .map_err(|e| format!("Update installation failed: {e}"))
        })
        .await
        .map_err(|e| e.to_string())?
    }
    .await;
    if let Err(error) = &result {
        engine.updates.set("failed", None, error);
    }
    result
}

pub fn start_checks(app: tauri::AppHandle) {
    // Isolated native tests must not contact a live release feed.
    #[cfg(debug_assertions)]
    if std::env::var_os("GAMEZIP_TEST_DATA_DIR").is_some() {
        return;
    }
    std::thread::spawn(move || {
        loop {
            let engine = app.state::<AppState>().0.clone();
            if engine.quitting.load(Ordering::Relaxed) {
                break;
            }
            let _ = tauri::async_runtime::block_on(check(&app, &engine));
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GAMEZIP_TEST_INSTALLER pointing to a locally signed review installer"]
    fn built_installer_signature_matches_embedded_key_and_rejects_tampering() {
        use base64::Engine as _;
        let installer = std::env::var("GAMEZIP_TEST_INSTALLER").expect("signed installer path");
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let decode = |value: &str| {
            String::from_utf8(
                base64::engine::general_purpose::STANDARD
                    .decode(value.trim())
                    .unwrap(),
            )
            .unwrap()
        };
        let public = minisign_verify::PublicKey::decode(&decode(
            config["plugins"]["updater"]["pubkey"].as_str().unwrap(),
        ))
        .unwrap();
        let signature = minisign_verify::Signature::decode(&decode(
            &std::fs::read_to_string(format!("{installer}.sig")).unwrap(),
        ))
        .unwrap();
        let mut bytes = std::fs::read(&installer).unwrap();
        public.verify(&bytes, &signature, true).unwrap();
        bytes[0] ^= 1;
        assert!(public.verify(&bytes, &signature, true).is_err());
    }
    #[test]
    fn updates_refuse_running_paused_and_waiting_jobs_and_release_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let engine = Arc::new(Engine {
            db: crate::database::Database::open(directory.path().join("fixture.sqlite")).unwrap(),
            active: Mutex::new(None),
            quitting: AtomicBool::new(false),
            updating: AtomicBool::new(false),
            updates: Updates::default(),
            backup_io: Mutex::new(()),
            backup_warnings: Mutex::new(vec![]),
            performance: crate::scheduling::Policy::new(Default::default()),
            prepared: Mutex::new(None),
            preparation_generation: std::sync::atomic::AtomicU64::new(0),
        });
        let pause = Arc::new(crate::scheduling::Pause::default());
        *engine.active.lock().unwrap() = Some(crate::Active {
            id: "fixture".into(),
            game_id: "game".into(),
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Some(pause.clone()),
        });
        assert!(InstallReservation::acquire(engine.clone()).is_err());
        pause.requested.store(true, Ordering::Relaxed);
        assert!(InstallReservation::acquire(engine.clone()).is_err());
        pause.waiting_idle.store(true, Ordering::Relaxed);
        assert!(InstallReservation::acquire(engine.clone()).is_err());
        *engine.active.lock().unwrap() = None;
        let reserved = InstallReservation::acquire(engine.clone()).unwrap();
        assert!(engine.updating.load(Ordering::SeqCst));
        assert!(InstallReservation::acquire(engine.clone()).is_err());
        drop(reserved);
        assert!(!engine.updating.load(Ordering::SeqCst));
        engine.quitting.store(true, Ordering::SeqCst);
        assert!(InstallReservation::acquire(engine).is_err());
    }
    #[test]
    fn only_project_release_artifacts_are_accepted() {
        assert!(approved_download(
            "https://github.com/saimadhav10/GameZip/releases/download/v1.0.0/GameZip.exe"
        ));
        assert!(!approved_download(
            "http://github.com/saimadhav10/GameZip/releases/download/file.exe"
        ));
        assert!(!approved_download(
            "https://github.com/other/GameZip/releases/download/file.exe"
        ));
        assert!(!approved_download(
            "https://github.com.evil.test/saimadhav10/GameZip/releases/download/file.exe"
        ));
    }
}
