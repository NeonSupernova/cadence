//! Remembers which version's release notes the person has seen, so What's New opens once per update.
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

/// Set at launch: true when no task database existed yet, i.e. a brand-new install.
pub struct WhatsNewState {
    pub fresh_install: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Seen {
    last_seen: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNewStatus {
    current: String,
    last_seen: Option<String>,
    fresh_install: bool,
}

fn path(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("whats-new.json")
}

#[tauri::command]
pub fn whats_new_status(app: AppHandle, state: State<'_, AppState>) -> WhatsNewStatus {
    let seen: Seen = std::fs::read_to_string(path(&state))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    WhatsNewStatus {
        current: app.package_info().version.to_string(),
        last_seen: seen.last_seen,
        fresh_install: app.state::<WhatsNewState>().fresh_install,
    }
}

#[tauri::command]
pub fn mark_whats_new_seen(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let seen = Seen { last_seen: Some(app.package_info().version.to_string()) };
    std::fs::write(path(&state), serde_json::to_vec(&seen).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
