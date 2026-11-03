//! Self-update from GitHub Releases. Updates are signed in CI and verified
//! against the public key in tauri.conf.json before they are installed.
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// How often a long-running session looks for a new release.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

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
async fn install_update(app: &AppHandle) -> tauri_plugin_updater::Result<bool> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(false);
    };
    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(true)
}
