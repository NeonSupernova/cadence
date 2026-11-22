//! Delivers due-date alerts and custom reminders through each platform's own notification
//! stream: Notification Center on macOS, toast notifications (Action Center) on Windows, and
//! the freedesktop D-Bus notification service on Linux. Scheduling lives in `core::alerts`.
use crate::{database, AppState};
use jev_tasks_core::alerts::{self, AlertSettings, Reminder, Repeat};
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

const TICK: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct AlertsState {
    /// Serializes reads and writes of reminders and delivery records.
    lock: Mutex<()>,
    wake: Mutex<Option<mpsc::Sender<()>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertsView {
    settings: AlertSettings,
    reminders: Vec<Reminder>,
    platform: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderInput {
    id: Option<String>,
    task_id: Option<i64>,
    title: String,
    #[serde(default)]
    note: String,
    at: String,
    #[serde(default)]
    repeat: Repeat,
    #[serde(default = "enabled")]
    enabled: bool,
}
fn enabled() -> bool {
    true
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// The name each backend recognizes as "the system's default alert sound".
fn default_sound() -> &'static str {
    if cfg!(target_os = "macos") {
        "NSUserNotificationDefaultSoundName"
    } else if cfg!(windows) {
        "Default"
    } else {
        "message-new-instant"
    }
}

fn deliver(app: &AppHandle, title: &str, body: &str, sound: bool) -> Result<(), String> {
    let mut builder = app.notification().builder().title(title).body(body);
    if sound {
        builder = builder.sound(default_sound());
    }
    builder.show().map_err(|e| e.to_string())
}

fn view(app: &AppHandle) -> AlertsView {
    let dir = &app.state::<AppState>().data_dir;
    let mut reminders = alerts::load_reminders(dir);
    reminders.sort_by(|a, b| b.enabled.cmp(&a.enabled).then_with(|| a.at_utc().cmp(&b.at_utc())));
    AlertsView { settings: alerts::load_settings(dir), reminders, platform: platform() }
}

fn changed(app: &AppHandle) {
    if let Ok(wake) = app.state::<AlertsState>().wake.lock() {
        if let Some(tx) = wake.as_ref() {
            let _ = tx.send(());
        }
    }
    let _ = app.emit("alerts-changed", ());
}

fn tick(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AlertsState>();
    let _guard = state.lock.lock().map_err(|_| "Alert state is unavailable.")?;
    let dir = app.state::<AppState>().data_dir.clone();
    let settings = alerts::load_settings(&dir);
    let reminders = alerts::load_reminders(&dir);
    let mut sent = alerts::load_sent(&dir);
    // Alerts cover the real workspace only; the demo workspace stays silent.
    let tasks = database(&dir, false)?.tasks()?;
    let now = chrono::Local::now();
    let plan = alerts::plan(&tasks, &reminders, &settings, &sent, &now);
    let mut dirty = false;
    if let Some(next) = &plan.reminders {
        alerts::save_reminders(&dir, next)?;
        dirty = true;
    }
    if !plan.sent_keys.is_empty() {
        let stamp = chrono::Utc::now().to_rfc3339();
        for key in &plan.sent_keys {
            sent.insert(key.clone(), stamp.clone());
        }
        alerts::prune_sent(&mut sent, chrono::Utc::now());
        // Record before showing so a crash never produces a duplicate.
        alerts::save_sent(&dir, &sent)?;
        dirty = true;
    }
    for delivery in &plan.deliveries {
        if let Err(error) = deliver(app, &delivery.title, &delivery.body, settings.sound) {
            eprintln!("Notification delivery failed: {error}");
        }
    }
    if dirty {
        let _ = app.emit("alerts-changed", ());
    }
    Ok(())
}

pub fn start(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    if let Ok(mut wake) = app.state::<AlertsState>().wake.lock() {
        *wake = Some(tx);
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        if let Err(error) = tick(&app) {
            eprintln!("Alert check failed: {error}");
        }
        // Settings and reminder edits wake the loop early; otherwise check every 20 seconds.
        match rx.recv_timeout(TICK) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    });
}

/// Holds alerts for `minutes`, or resumes them with `None`. Held alerts are delivered later.
pub fn pause(app: &AppHandle, minutes: Option<u32>) -> Result<AlertsView, String> {
    {
        let dir = &app.state::<AppState>().data_dir;
        let mut settings = alerts::load_settings(dir);
        settings.paused_until = minutes
            .filter(|m| (1..=24 * 60).contains(m))
            .map(|m| (chrono::Utc::now() + chrono::Duration::minutes(m as i64)).to_rfc3339());
        alerts::save_settings(dir, &settings)?;
    }
    changed(app);
    Ok(view(app))
}

#[tauri::command]
pub fn get_alerts(app: AppHandle) -> AlertsView {
    view(&app)
}

#[tauri::command]
pub fn save_alert_settings(app: AppHandle, state: State<'_, AppState>, settings: AlertSettings) -> Result<AlertsView, String> {
    let settings = settings.validate()?;
    alerts::save_settings(&state.data_dir, &settings)?;
    changed(&app);
    Ok(view(&app))
}

#[tauri::command]
pub fn pause_alerts(app: AppHandle, minutes: Option<u32>) -> Result<AlertsView, String> {
    pause(&app, minutes)
}

#[tauri::command]
pub fn save_reminder(app: AppHandle, input: ReminderInput) -> Result<AlertsView, String> {
    {
        let dir = app.state::<AppState>().data_dir.clone();
        if let Some(id) = input.task_id {
            let tasks = database(&dir, false)?.tasks()?;
            if !tasks.iter().any(|t| t.id == id && !t.deleted) {
                return Err("That task no longer exists.".into());
            }
        }
        let reminder = Reminder {
            id: input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            task_id: input.task_id,
            title: input.title,
            note: input.note,
            at: input.at,
            repeat: input.repeat,
            enabled: input.enabled,
            last_fired_at: None,
        }
        .validate()?;
        let state = app.state::<AlertsState>();
        let _guard = state.lock.lock().map_err(|_| "Alert state is unavailable.")?;
        let mut reminders = alerts::load_reminders(&dir);
        match reminders.iter_mut().find(|r| r.id == reminder.id) {
            Some(existing) => *existing = Reminder { last_fired_at: existing.last_fired_at.clone(), ..reminder },
            None => reminders.push(reminder),
        }
        alerts::save_reminders(&dir, &reminders)?;
    }
    changed(&app);
    Ok(view(&app))
}

#[tauri::command]
pub fn delete_reminder(app: AppHandle, id: String) -> Result<AlertsView, String> {
    {
        let dir = app.state::<AppState>().data_dir.clone();
        let state = app.state::<AlertsState>();
        let _guard = state.lock.lock().map_err(|_| "Alert state is unavailable.")?;
        let mut reminders = alerts::load_reminders(&dir);
        reminders.retain(|r| r.id != id);
        alerts::save_reminders(&dir, &reminders)?;
    }
    changed(&app);
    Ok(view(&app))
}

#[tauri::command]
pub fn test_notification(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let settings = alerts::load_settings(&state.data_dir);
    deliver(&app, "Cadence alerts are on", "Due dates and reminders will appear like this.", settings.sound)
}
