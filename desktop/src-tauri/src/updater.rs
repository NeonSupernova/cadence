//! Self-update from GitHub Releases. Updates are signed in CI and verified
//! against the public key in tauri.conf.json before they are installed.
//!
//! A background check runs every few hours and installs silently. The
//! "Check for Updates…" menu item and Settings button run the same check on
//! demand and let the person choose when to install.
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// How often a long-running session looks for a new release.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Only one download/install may run at a time, manual or background.
static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn start_background(app: &AppHandle) {
    // Dev builds share the version of the last release; never replace them.
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        match tauri::async_runtime::block_on(install_update(&app)) {
            // Task data lives in SQLite, so restarting loses nothing.
            Ok(true) => app.restart(),
            Ok(false) => {}
            Err(error) => eprintln!("Update check failed: {error}"),
        }
        std::thread::sleep(CHECK_INTERVAL);
    });
}

/// Returns true when a newer version was downloaded and installed.
async fn install_update(app: &AppHandle) -> Result<bool, String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    let result = async {
        let Some(update) = app
            .updater()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(false);
        };
        let handle = app.clone();
        let mut downloaded: u64 = 0;
        update
            .download_and_install(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    let _ = handle.emit(
                        "update-progress",
                        serde_json::json!({ "downloaded": downloaded, "total": total }),
                    );
                },
                || {},
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
    .await;
    INSTALLING.store(false, Ordering::SeqCst);
    result
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current_version: String,
    /// The newer version, when one is published.
    available: Option<String>,
    notes: Option<String>,
    date: Option<String>,
    /// False in development builds, which must never be replaced.
    can_install: bool,
}

fn friendly(error: impl std::fmt::Display) -> String {
    format!("Couldn't check for updates. Check your internet connection and try again. ({error})")
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateCheck, String> {
    let update = app
        .updater()
        .map_err(friendly)?
        .check()
        .await
        .map_err(friendly)?;
    Ok(UpdateCheck {
        current_version: app.package_info().version.to_string(),
        available: update.as_ref().map(|u| u.version.clone()),
        notes: update.as_ref().and_then(|u| u.body.clone()),
        date: update.as_ref().and_then(|u| u.date).and_then(|d| {
            chrono::DateTime::from_timestamp(d.unix_timestamp(), 0).map(|t| t.to_rfc3339())
        }),
        can_install: !cfg!(debug_assertions),
    })
}

#[tauri::command]
pub async fn install_available_update(app: AppHandle) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Development builds don't install updates. Use a release build.".into());
    }
    if INSTALLING.load(Ordering::SeqCst) {
        return Err("An update is already downloading.".into());
    }
    if install_update(&app)
        .await
        .map_err(|e| format!("The update couldn't be installed: {e}"))?
    {
        app.restart();
    }
    Err("You're already on the latest version.".into())
}
