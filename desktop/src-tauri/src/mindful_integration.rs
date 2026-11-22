//! Mindful break overlay: a small frameless, always-on-top window at the top of
//! the screen, built natively by Tauri on macOS, Windows and Linux. The schedule
//! lives in `jev_tasks_core::mindful`; this module shows and hides the overlay.
use crate::{database, AppState};
use jev_tasks_core::mindful::{MindfulSettings, Reminder, Response, Scheduler};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
};

const LABEL: &str = "mindful";
const WIDTH: f64 = 420.0;
const HEIGHT: f64 = 112.0;

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mindful-settings.json")
}
fn load_settings(data_dir: &Path) -> MindfulSettings {
    std::fs::read_to_string(settings_path(data_dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Showing {
    kind: Reminder,
    seconds: u32,
    /// Chooses among a few phrasings so reminders don't feel repetitive.
    variant: u32,
    preview: bool,
}

pub struct MindfulState {
    scheduler: Mutex<Scheduler>,
    showing: Mutex<Option<Showing>>,
    stop: AtomicBool,
    shown_count: std::sync::atomic::AtomicU32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MindfulStatus {
    settings: MindfulSettings,
    quiet_until: Option<String>,
}

fn any_timer_running(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    database(&state.data_dir, false)
        .and_then(|s| s.tasks())
        .map(|tasks| {
            tasks
                .iter()
                .any(|t| !t.deleted && t.timer_started_at.is_some())
        })
        .unwrap_or(false)
}

/// Puts the overlay at the top center of the screen the main window is on.
fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let monitor = app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let origin = monitor.position();
        // Clear the macOS menu bar and notch; elsewhere sit just below the edge.
        let top = if cfg!(target_os = "macos") { 44.0 } else { 14.0 };
        let x = origin.x + ((size.width as f64 - WIDTH * scale) / 2.0).round() as i32;
        let y = origin.y + (top * scale).round() as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Raises the overlay above full-screen apps and other floating windows on macOS.
#[cfg(target_os = "macos")]
fn float_everywhere(window: &tauri::WebviewWindow) {
    let target = window.clone();
    let _ = window.run_on_main_thread(move || {
        use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
        if let Ok(ptr) = target.ns_window() {
            // SAFETY: Tauri returns this window's live NSWindow; we're on the main thread.
            let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
            unsafe {
                ns.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::FullScreenAuxiliary
                        | NSWindowCollectionBehavior::Stationary
                        | NSWindowCollectionBehavior::IgnoresCycle,
                );
                // NSStatusWindowLevel: above normal and floating windows.
                ns.setLevel(25);
            }
        }
    });
}

fn show(app: &AppHandle, showing: Showing) -> Result<(), String> {
    let state = app.state::<MindfulState>();
    *state.showing.lock().map_err(|_| "Reminder state unavailable.")? = Some(showing.clone());
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => {
            let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("mindful".into()))
                .title("Cadence break")
                .inner_size(WIDTH, HEIGHT)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?;
            #[cfg(target_os = "macos")]
            float_everywhere(&window);
            window
        }
    };
    place(app, &window);
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_always_on_top(true);
    // A newly built page fetches the reminder itself; an existing one gets the event.
    let _ = window.emit("mindful-show", showing);
    Ok(())
}

fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

pub fn start_background(app: &AppHandle) {
    app.manage(MindfulState {
        scheduler: Mutex::new(Scheduler::default()),
        showing: Mutex::new(None),
        stop: AtomicBool::new(false),
        shown_count: Default::default(),
    });
    let app = app.clone();
    std::thread::spawn(move || {
        let data_dir = app.state::<AppState>().data_dir.clone();
        loop {
            let state = app.state::<MindfulState>();
            if state.stop.load(Ordering::Relaxed) {
                break;
            }
            let settings = load_settings(&data_dir);
            let running = any_timer_running(&app);
            let due = state
                .scheduler
                .lock()
                .ok()
                .and_then(|mut s| s.tick(chrono::Utc::now(), running, &settings));
            if let Some(kind) = due {
                let variant = state.shown_count.fetch_add(1, Ordering::Relaxed);
                if show(
                    &app,
                    Showing {
                        kind,
                        seconds: settings.display_seconds,
                        variant,
                        preview: false,
                    },
                )
                .is_err()
                {
                    // Couldn't show it; don't leave the scheduler waiting forever.
                    if let Ok(mut s) = state.scheduler.lock() {
                        s.respond(kind, Response::Timeout, chrono::Utc::now());
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });
}

pub fn stop(app: &AppHandle) {
    if let Some(state) = app.try_state::<MindfulState>() {
        state.stop.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn mindful_current(state: State<'_, MindfulState>) -> Option<Showing> {
    state.showing.lock().ok().and_then(|s| s.clone())
}

#[tauri::command]
pub fn mindful_respond(
    app: AppHandle,
    state: State<'_, MindfulState>,
    kind: Reminder,
    response: Response,
) -> Result<(), String> {
    let current = state
        .showing
        .lock()
        .map_err(|_| "Reminder state unavailable.")?
        .take();
    hide(&app);
    if current.is_some_and(|c| !c.preview) {
        state
            .scheduler
            .lock()
            .map_err(|_| "Reminder state unavailable.")?
            .respond(kind, response, chrono::Utc::now());
    }
    let _ = app.emit_to("main", "mindful-response", serde_json::json!({ "kind": kind, "response": response }));
    Ok(())
}

/// Pauses reminders for some minutes, or resumes them with 0.
#[tauri::command]
pub fn mindful_quiet(
    app: AppHandle,
    state: State<'_, MindfulState>,
    minutes: u32,
) -> Result<MindfulStatus, String> {
    if minutes > 24 * 60 {
        return Err("Pause reminders for at most a day.".into());
    }
    let until = (minutes > 0)
        .then(|| chrono::Utc::now() + chrono::Duration::minutes(i64::from(minutes)));
    let showing = state
        .showing
        .lock()
        .map_err(|_| "Reminder state unavailable.")?
        .take();
    {
        let mut scheduler = state.scheduler.lock().map_err(|_| "Reminder state unavailable.")?;
        if let Some(current) = showing.filter(|c| !c.preview) {
            scheduler.respond(current.kind, Response::Skip, chrono::Utc::now());
        }
        scheduler.quiet(until);
    }
    hide(&app);
    Ok(get_mindful_settings(app.state::<AppState>(), state))
}

#[tauri::command]
pub fn get_mindful_settings(
    app_state: State<'_, AppState>,
    state: State<'_, MindfulState>,
) -> MindfulStatus {
    MindfulStatus {
        settings: load_settings(&app_state.data_dir),
        quiet_until: state
            .scheduler
            .lock()
            .ok()
            .and_then(|s| s.quiet_until())
            .filter(|q| *q > chrono::Utc::now())
            .map(|q| q.to_rfc3339()),
    }
}

#[tauri::command]
pub fn save_mindful_settings(
    app_state: State<'_, AppState>,
    state: State<'_, MindfulState>,
    settings: MindfulSettings,
) -> Result<MindfulStatus, String> {
    settings.validate()?;
    std::fs::write(
        settings_path(&app_state.data_dir),
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Could not save break reminders: {e}"))?;
    Ok(get_mindful_settings(app_state, state))
}

/// Shows a reminder right away so the person can see what it looks like.
#[tauri::command]
pub fn mindful_preview(app: AppHandle, kind: Reminder) -> Result<(), String> {
    let state = app.state::<MindfulState>();
    if state
        .showing
        .lock()
        .map_err(|_| "Reminder state unavailable.")?
        .as_ref()
        .is_some_and(|s| !s.preview)
    {
        return Err("A reminder is already showing.".into());
    }
    let seconds = load_settings(&app.state::<AppState>().data_dir).display_seconds;
    let variant = state.shown_count.fetch_add(1, Ordering::Relaxed);
    show(
        &app,
        Showing {
            kind,
            seconds,
            variant,
            preview: true,
        },
    )
}
