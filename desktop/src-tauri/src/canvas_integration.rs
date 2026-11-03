use crate::{database, snapshot, AppState, Request};
use jev_tasks_core::{
    canvas::{normalize_base, CanvasClient, CanvasConfig, CanvasReport},
    Plan, Snapshot,
};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSettings {
    base_url: String,
    configured: bool,
    auto_sync: bool,
    report: Option<CanvasReport>,
    error: Option<String>,
}
#[tauri::command]
pub fn get_canvas_settings(state: State<'_, AppState>) -> Result<CanvasSettings, String> {
    let config = CanvasConfig::load(&state.data_dir);
    Ok(CanvasSettings {
        base_url: config.base_url,
        configured: !config.token.trim().is_empty(),
        auto_sync: config.auto_sync,
        report: database(&state.data_dir, false)?.canvas_report()?,
        error: state
            .canvas_error
            .lock()
            .map_err(|_| "Could not read Canvas status.")?
            .clone(),
    })
}
#[tauri::command]
pub fn save_canvas_settings(
    state: State<'_, AppState>,
    base_url: String,
    token: Option<String>,
    auto_sync: bool,
) -> Result<CanvasSettings, String> {
    let _guard = state.canvas_gate.try_lock().map_err(|_| {
        "Wait for the current Canvas sync to finish before changing the connection."
    })?;
    let mut config = CanvasConfig::load(&state.data_dir);
    let base = normalize_base(&base_url)?
        .as_str()
        .trim_end_matches('/')
        .to_string();
    let new_token = token.filter(|s| !s.trim().is_empty());
    if base != config.base_url.trim_end_matches('/')
        && new_token.is_none()
        && !config.token.is_empty()
    {
        return Err("Enter a token for the new Canvas address before saving.".into());
    }
    config.base_url = base;
    if let Some(token) = new_token {
        config.token = token.trim().into();
    }
    config.auto_sync = auto_sync;
    config.save(&state.data_dir)?;
    *state
        .canvas_error
        .lock()
        .map_err(|_| "Could not update Canvas status.")? = None;
    // Avoid reusing the held sync guard via a state-consuming settings call.
    Ok(CanvasSettings {
        base_url: config.base_url,
        configured: true,
        auto_sync: config.auto_sync,
        report: database(&state.data_dir, false)?.canvas_report()?,
        error: None,
    })
}
pub fn sync(app: &AppHandle) -> Result<CanvasReport, String> {
    let state = app.state::<AppState>();
    let _sync_guard = state
        .canvas_gate
        .try_lock()
        .map_err(|_| "Canvas is already syncing.")?;
    *state
        .canvas_last_attempt
        .lock()
        .map_err(|_| "Could not start Canvas sync.")? = Some(std::time::Instant::now());
    let _ = app.emit("canvas-sync-status", serde_json::json!({"syncing":true}));
    let result = (|| {
        let import = CanvasClient {
            config: CanvasConfig::load(&state.data_dir),
        }
        .assignments()?;
        // Network fetching does not hold the task gate. Only the import transaction does.
        let _task_guard = state
            .gate
            .lock()
            .map_err(|_| "Could not save Canvas assignments.")?;
        database(&state.data_dir, false)?.import_canvas(import)
    })();
    *state
        .canvas_error
        .lock()
        .map_err(|_| "Could not update Canvas status.")? = result.as_ref().err().cloned();
    match &result {
        Ok(report) => {
            let _ = app.emit(
                "canvas-sync-status",
                serde_json::json!({"syncing":false,"report":report}),
            );
            let _ = app.emit("tasks-changed", serde_json::json!({"demo":false}));
        }
        Err(error) => {
            let _ = app.emit(
                "canvas-sync-status",
                serde_json::json!({"syncing":false,"error":error}),
            );
        }
    }
    result
}
#[tauri::command]
pub async fn sync_canvas(app: AppHandle) -> Result<CanvasReport, String> {
    tauri::async_runtime::spawn_blocking(move || sync(&app))
        .await
        .map_err(|_| "Canvas sync was interrupted.".to_string())?
}
pub async fn sync_chat(
    app: AppHandle,
    request: Request,
    message: String,
) -> Result<Snapshot, String> {
    if request.demo {
        return Err(
            "Canvas sync imports into your real workspace. Leave the demo to sync assignments."
                .into(),
        );
    }
    tauri::async_runtime::spawn_blocking(move || {
        let report = sync(&app)?;
        let state = app.state::<AppState>();
        let _guard = state.gate.lock().map_err(|_| "Could not save the conversation.")?;
        let view = jev_tasks_core::View { status: "all".into(), label: "canvas".into(), ..jev_tasks_core::View::default() };
        let reply = format!("Synced {} Canvas assignments from {} courses: {} added, {} updated. All Canvas assignments carry the canvas label.", report.assignments, report.courses, report.created, report.updated);
        database(&state.data_dir, false)?.apply(Plan { reply, actions: vec![], view: None, focus_ids: vec![], ..Default::default() }, &message, None, true)?;
        let mut response = snapshot(&state.data_dir, Request { view, ..request }, vec![])?;
        response.show_list = true;
        Ok(response)
    }).await.map_err(|_| "Canvas sync was interrupted.".to_string())?
}
#[tauri::command]
pub fn open_canvas_assignment(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let source = database(&state.data_dir, false)?
        .tasks()?
        .into_iter()
        .find(|t| t.id == id)
        .and_then(|t| t.canvas)
        .ok_or("This task does not have a Canvas assignment.")?;
    // Use the canonical same-origin assignment path, never an arbitrary model-supplied URL.
    let url = normalize_base(&source.base_url)?
        .join(&format!(
            "courses/{}/assignments/{}",
            source.course_id, source.assignment_id
        ))
        .map_err(|_| "Invalid Canvas assignment link.")?;
    #[cfg(target_os = "macos")]
    unsafe {
        let value = objc2_foundation::NSString::from_str(url.as_str());
        let native_url = objc2_foundation::NSURL::URLWithString(&value)
            .ok_or("Invalid Canvas assignment link.")?;
        if !objc2_app_kit::NSWorkspace::sharedWorkspace().openURL(&native_url) {
            return Err("Could not open Canvas in your browser.".into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    return Err("Opening Canvas links currently requires macOS.".into());
    Ok(())
}
pub fn start_background(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        let state = app.state::<AppState>();
        if state.canvas_stop.load(Ordering::Relaxed) {
            break;
        }
        let config = CanvasConfig::load(&state.data_dir);
        let due = state
            .canvas_last_attempt
            .lock()
            .is_ok_and(|last| last.is_none_or(|time| time.elapsed().as_secs() >= 900));
        if config.auto_sync && !config.token.trim().is_empty() && due {
            let _ = sync(&app);
        }
        for _ in 0..60 {
            if state.canvas_stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}
