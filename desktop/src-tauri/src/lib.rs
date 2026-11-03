use jev_tasks_core::ai::PlanningContext;
use jev_tasks_core::{Action, Config, Plan, Router, Snapshot, Store, View};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};
mod canvas_integration;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(desktop)]
mod updater;
mod workload_integration;

#[tauri::command]
fn quick_capture(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::quick_capture(&app);
    #[cfg(not(target_os = "macos"))]
    Err("Quick Capture requires macOS.".into())
}
#[tauri::command]
fn task_context_menu(app: tauri::AppHandle, id: i64, demo: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::task_menu(&app, id, demo);
    #[cfg(not(target_os = "macos"))]
    Err("Native task menus require macOS.".into())
}
#[tauri::command]
fn show_main(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    macos::show_main(&app);
}
#[tauri::command]
fn dismiss_capture(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("capture") {
        let _ = window.hide();
    }
}

struct AppState {
    data_dir: PathBuf,
    gate: Arc<Mutex<()>>,
    appearance: Mutex<Option<Appearance>>,
    canvas_gate: Arc<Mutex<()>>,
    canvas_last_attempt: Mutex<Option<std::time::Instant>>,
    canvas_error: Mutex<Option<String>>,
    canvas_stop: Arc<std::sync::atomic::AtomicBool>,
}
#[derive(Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Appearance {
    reduce_transparency: bool,
    reduce_motion: bool,
    increase_contrast: bool,
    native_glass: bool,
}
#[tauri::command]
fn get_appearance(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Appearance, String> {
    #[allow(unused_mut)]
    let mut prefs = Appearance::default();
    #[cfg(target_os = "macos")]
    {
        // NSWorkspace's read-only accessibility display preferences do not require
        // accessibility permission. NSVisualEffectView supplies the system material.
        let workspace = unsafe { objc2_app_kit::NSWorkspace::sharedWorkspace() };
        prefs.reduce_transparency =
            unsafe { workspace.accessibilityDisplayShouldReduceTransparency() };
        prefs.reduce_motion = unsafe { workspace.accessibilityDisplayShouldReduceMotion() };
        prefs.increase_contrast = unsafe { workspace.accessibilityDisplayShouldIncreaseContrast() };
        prefs.native_glass = !prefs.reduce_transparency && !prefs.increase_contrast;
    }
    let mut previous = state
        .appearance
        .lock()
        .map_err(|_| "Could not update appearance.")?;
    if previous.as_ref() != Some(&prefs) {
        if let Some(window) = app.get_webview_window("main") {
            #[cfg(target_os = "macos")]
            {
                use tauri::window::{Effect, EffectState, EffectsBuilder};
                if prefs.native_glass {
                    if window
                        .set_effects(
                            EffectsBuilder::new()
                                .effect(Effect::Sidebar)
                                .state(EffectState::FollowsWindowActiveState)
                                .build(),
                        )
                        .is_err()
                    {
                        prefs.native_glass = false;
                    }
                } else {
                    let _ = window.set_effects(None::<tauri::utils::config::WindowEffectsConfig>);
                }
            }
            let _ = window.set_theme(None);
        }
        *previous = Some(prefs.clone());
    }
    Ok(prefs)
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    view: View,
    selected_id: Option<i64>,
    demo: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    configured: bool,
    model: String,
    jev_model: String,
    reasoning_effort: String,
    label_threshold: f64,
}
fn database(data_dir: &std::path::Path, demo: bool) -> Result<Store, String> {
    let mut store = Store::open(&data_dir.join(if demo { "demo.sqlite" } else { "tasks.sqlite" }))?;
    if demo {
        store.seed_demo()?;
    }
    Ok(store)
}
fn snapshot(
    data_dir: &std::path::Path,
    request: Request,
    focus_ids: Vec<i64>,
) -> Result<Snapshot, String> {
    let config = Config::load(data_dir);
    database(data_dir, request.demo)?.snapshot(
        request.view,
        focus_ids,
        !config.api_key.is_empty(),
        config.model,
        request.demo,
    )
}
#[tauri::command]
fn get_snapshot(state: State<'_, AppState>, request: Request) -> Result<Snapshot, String> {
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    let c = Config::load(&state.data_dir);
    Settings {
        configured: !c.api_key.is_empty(),
        model: c.model,
        jev_model: c.jev_model,
        reasoning_effort: c.reasoning_effort,
        label_threshold: c.label_threshold,
    }
}
#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    api_key: Option<String>,
    model: String,
    jev_model: String,
    reasoning_effort: String,
) -> Result<Settings, String> {
    let mut config = Config::load(&state.data_dir);
    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            config.api_key = key.trim().into();
        }
    }
    config.model = model.trim().into();
    config.jev_model = jev_model.trim().into();
    config.reasoning_effort = reasoning_effort;
    config.save(&state.data_dir)?;
    Ok(get_settings(state))
}
#[tauri::command]
fn change_tasks(
    state: State<'_, AppState>,
    request: Request,
    actions: Vec<Action>,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Please wait for the current task operation to finish.")?;
    let mut store = database(&state.data_dir, request.demo)?;
    let focus = store.apply(
        Plan {
            reply: String::new(),
            actions,
            view: None,
            focus_ids: vec![],
            ..Default::default()
        },
        "",
        None,
        false,
    )?;
    snapshot(&state.data_dir, request, focus)
}
#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: Request,
    message: String,
) -> Result<Snapshot, String> {
    if [
        "sync canvas",
        "refresh canvas",
        "import canvas assignments",
        "sync my canvas assignments",
        "sync canvas assignments",
    ]
    .contains(
        &message
            .trim()
            .trim_end_matches(['.', '!'])
            .to_lowercase()
            .as_str(),
    ) {
        return canvas_integration::sync_chat(app, request, message).await;
    }
    let data_dir = state.data_dir.clone();
    let gate = state.gate.clone();
    process_chat(app, data_dir, gate, request, message, false).await
}
#[tauri::command]
async fn capture_task(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<Snapshot, String> {
    if message.trim().is_empty() || message.chars().count() > 19000 {
        return Err("Write a task between 1 and 19,000 characters.".into());
    }
    let data_dir = state.data_dir.clone();
    let gate = state.gate.clone();
    process_chat(
        app,
        data_dir,
        gate,
        Request {
            view: View::default(),
            selected_id: None,
            demo: false,
        },
        format!("Create one new task from this text: {message}"),
        true,
    )
    .await
}
async fn process_chat(
    app: tauri::AppHandle,
    data_dir: PathBuf,
    gate: Arc<Mutex<()>>,
    request: Request,
    message: String,
    capture_only: bool,
) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.try_lock().map_err(|_| "Please wait for the current task operation to finish.")?;
        let message = message.trim();
        if message.is_empty() || message.chars().count() > 20000 { return Err("Write a message between 1 and 20,000 characters.".into()); }
        request.view.validate()?;
        let store = database(&data_dir, request.demo)?;
        let tasks = store.tasks()?;
        if serde_json::to_vec(&tasks).map_err(|e| e.to_string())?.len() > 350000 { return Err("This prototype's task context is full. Shorten large descriptions before using chat.".into()); }
        let history = store.messages()?;
        let workload = store.workload(&tasks)?;
        let task_version = serde_json::to_vec(&tasks).map_err(|e|e.to_string())?;
        drop(store);
        drop(_guard);
        let router = Router { config: Config::load(&data_dir) };
        let _ = app.emit("chat-progress", "Organizing your request…");
        let (classification, intent) = router.classify(message, &tasks)?;
        let _ = app.emit("chat-progress", "Working out the details…");
        let mut plan = router.plan(PlanningContext { input: message, tasks: &tasks, messages: &history, view: &request.view, selected: request.selected_id, classification: &classification, intent: &intent, workload: &workload })?;
        if capture_only && (plan.actions.len() != 1 || plan.actions[0].kind != "create" || plan.capacity.is_some() || plan.checkin.is_some() || plan.feedback.is_some()) {
            return Err("Quick Capture only adds a new task. Open the main window to change existing tasks.".into());
        }
        if plan.actions.iter().filter(|a| a.kind == "create").count() > 1 {
            for action in plan.actions.iter_mut().filter(|a| a.kind == "create") {
                let text = format!("{}\n{}", action.title.as_deref().unwrap_or(""), action.description.as_deref().unwrap_or(""));
                let (c, _) = router.classify(&text, &tasks)?;
                if action.task_type.is_none(){action.task_type = Some(c.kind);}
                if action.labels.is_none(){action.labels = Some(c.labels);}
                if action.priority.is_none(){action.priority = Some(c.priority);}
                if action.estimate.is_none(){action.estimate=c.estimate;}
            }
        }
        let show_list = plan.view.is_some();
        let view = plan.view.clone().unwrap_or_else(|| request.view.clone());
        let _ = app.emit("chat-progress", "Saving your changes…");
        let _commit_guard = gate.lock().map_err(|_| "Could not save the conversation.")?;
        let mut store = database(&data_dir, request.demo)?;
        let current = store.tasks()?;
        jev_tasks_core::store::validate_chat_version(&task_version, &current, &plan)?;
        let focus = store.apply(plan, message, Some(&classification), true)?;
        let _ = app.emit("tasks-changed", serde_json::json!({ "demo": request.demo }));
        let mut result = snapshot(&data_dir, Request { view, ..request }, focus)?;
        result.show_list = show_list;
        Ok(result)
    }).await.map_err(|_| "Task processing was interrupted. Please try again.".to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(AppState {
                data_dir,
                gate: Arc::new(Mutex::new(())),
                appearance: Mutex::new(None),
                canvas_gate: Arc::new(Mutex::new(())),
                canvas_last_attempt: Mutex::new(None),
                canvas_error: Mutex::new(None),
                canvas_stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_skip_taskbar(false);
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }
            #[cfg(target_os = "macos")]
            macos::setup(app.handle())?;
            canvas_integration::start_background(app.handle());
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                updater::start_background(app.handle());
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            #[cfg(target_os = "macos")]
            macos::handle_menu(app, event.id().as_ref());
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // A normal macOS app remains in the Dock; clicking its icon reopens it.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_settings,
            save_settings,
            change_tasks,
            chat,
            get_appearance,
            quick_capture,
            task_context_menu,
            show_main,
            dismiss_capture,
            capture_task,
            canvas_integration::get_canvas_settings,
            canvas_integration::save_canvas_settings,
            canvas_integration::sync_canvas,
            canvas_integration::open_canvas_assignment,
            workload_integration::save_task_estimate,
            workload_integration::open_task_reference,
            workload_integration::save_capacity_profile,
            workload_integration::save_day_checkin,
            workload_integration::rate_task_effort,
            workload_integration::review_capacity_day,
            workload_integration::assess_task_effort
        ])
        .build(tauri::generate_context!())
        .expect("Could not start Cadence")
        .run(|app, event| {
            if matches!(&event, tauri::RunEvent::Exit) {
                app.state::<AppState>()
                    .canvas_stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(target_os = "macos")]
            if matches!(&event, tauri::RunEvent::Exit) {
                app.state::<macos::NativeState>()
                    .stop
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        });
}
