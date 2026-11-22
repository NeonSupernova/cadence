//! Signed updates from GitHub Releases, both automatic and on demand.
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Default)]
pub struct UpdaterState {
    checking: AtomicBool,
    last_checked_at: Mutex<Option<String>>,
    message: Mutex<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current_version: String,
    checking: bool,
    last_checked_at: Option<String>,
    message: String,
}

fn status(app: &AppHandle) -> UpdateStatus {
    let state = app.state::<UpdaterState>();
    UpdateStatus {
        current_version: app.package_info().version.to_string(),
        checking: state.checking.load(Ordering::SeqCst),
        last_checked_at: state.last_checked_at.lock().ok().and_then(|v| v.clone()),
        message: state.message.lock().map(|v| v.clone()).unwrap_or_default(),
    }
}

fn publish(app: &AppHandle, message: &str) {
    if let Ok(mut current) = app.state::<UpdaterState>().message.lock() {
        *current = message.to_string();
    }
    let _ = app.emit("update-status", status(app));
}

struct CheckGuard<'a>(&'a AtomicBool);
impl Drop for CheckGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn check_and_install(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<UpdaterState>();
    state
        .checking
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "An update check is already in progress.".to_string())?;
    let _guard = CheckGuard(&state.checking);
    publish(app, "Checking for updates…");
    let result = async {
        let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
        if let Ok(mut checked) = state.last_checked_at.lock() {
            *checked = Some(chrono::Utc::now().to_rfc3339());
        }
        let Some(update) = update else {
            publish(app, "Cadence is up to date.");
            return Ok(false);
        };
        publish(app, &format!("Downloading Cadence {}…", update.version));
        update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
        publish(app, "Update installed. Restarting…");
        Ok(true)
    }.await;
    if let Err(error) = &result {
        publish(app, &format!("Update check failed: {error}"));
    }
    result
}

pub fn start_background(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        match tauri::async_runtime::block_on(check_and_install(&app)) {
            Ok(true) => app.restart(),
            Ok(false) => {}
            Err(error) => eprintln!("Update check failed: {error}"),
        }
        std::thread::sleep(CHECK_INTERVAL);
    });
}

#[tauri::command]
pub fn get_update_status(app: AppHandle) -> UpdateStatus {
    status(&app)
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateStatus, String> {
    if cfg!(debug_assertions) {
        return Err("Update checks are available in installed builds.".into());
    }
    if check_and_install(&app).await? {
        app.restart();
    }
    Ok(status(&app))
}
