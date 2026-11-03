//! AppKit integrations live here; task state still goes through the Rust store.
use crate::{database, get_appearance, AppState};
use jev_tasks_core::{Action, Plan};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSString};
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{
    image::Image,
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem as Native, Submenu},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub struct NativeState {
    tray: TrayIcon,
    info: MenuItem<tauri::Wry>,
    pause: MenuItem<tauri::Wry>,
    complete: MenuItem<tauri::Wry>,
    pub stop: Arc<AtomicBool>,
}
fn item(
    app: &AppHandle,
    id: &str,
    title: &str,
    shortcut: Option<&str>,
) -> tauri::Result<MenuItem<tauri::Wry>> {
    MenuItem::with_id(app, id, title, true, shortcut)
}
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn quick_capture(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("capture") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        let _ = window.emit("capture-ready", ());
    } else {
        WebviewWindowBuilder::new(app, "capture", WebviewUrl::App("capture".into()))
            .title("Quick Capture")
            .inner_size(590.0, 290.0)
            .resizable(false)
            .center()
            .always_on_top(true)
            .build()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn ui_action(app: &AppHandle, action: &str) {
    show_main(app);
    let _ = app.emit_to(
        "main",
        "native-action",
        serde_json::json!({ "action": action }),
    );
}
fn change(app: &AppHandle, kind: &str, id: i64, demo: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current task operation to finish.")?;
    database(&state.data_dir, demo)?.apply(
        Plan {
            reply: String::new(),
            actions: vec![Action::status(kind, id)],
            view: None,
            focus_ids: vec![],
            ..Default::default()
        },
        "",
        None,
        false,
    )?;
    let _ = app.emit(
        "tasks-changed",
        serde_json::json!({ "demo": demo, "id": id }),
    );
    Ok(())
}
fn running_change(app: &AppHandle, kind: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let task = database(&state.data_dir, false)?
        .tasks()?
        .into_iter()
        .find(|t| !t.deleted && t.timer_started_at.is_some());
    if let Some(task) = task {
        change(app, kind, task.id, false)?;
    }
    Ok(())
}
fn report(app: &AppHandle, result: Result<(), String>) {
    if let Err(error) = result {
        show_main(app);
        let _ = app.emit("native-error", error);
    }
}
pub fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "quick-capture" => report(app, quick_capture(app)),
        "open-main" => show_main(app),
        "settings" | "new-task" | "find-task" | "focus-view" | "next-view" | "journal-view"
        | "toggle-timer" | "complete-selected" => ui_action(app, id),
        "pause-running" => report(app, running_change(app, "pause")),
        "complete-running" => report(app, running_change(app, "complete")),
        "quit-app" => app.exit(0),
        _ => {
            let parts: Vec<_> = id.split(':').collect();
            if parts.len() == 4 && parts[0] == "task" {
                if let Ok(task_id) = parts[3].parse::<i64>() {
                    let demo = parts[1] == "demo";
                    match parts[2] {
                        "edit" => {
                            let _ = app.emit_to("main", "native-action", serde_json::json!({ "action": "edit", "id": task_id, "demo": demo }));
                        }
                        "copy" => {
                            // NSPasteboard performs a write only; the app never reads the clipboard.
                            unsafe {
                                let pasteboard = objc2_app_kit::NSPasteboard::generalPasteboard();
                                pasteboard.clearContents();
                                pasteboard.setString_forType(
                                    &NSString::from_str(&format!("TASK-{task_id:03}")),
                                    objc2_app_kit::NSPasteboardTypeString,
                                );
                            }
                        }
                        kind => report(app, change(app, kind, task_id, demo)),
                    }
                }
            }
        }
    }
}
pub fn task_menu(app: &AppHandle, id: i64, demo: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let task = database(&state.data_dir, demo)?
        .tasks()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or("Task does not exist.")?;
    let prefix = format!("task:{}:", if demo { "demo" } else { "real" });
    let make = |kind: &str, title: &str| {
        item(app, &format!("{prefix}{kind}:{id}"), title, None).map_err(|e| e.to_string())
    };
    let menu = Menu::new(app).map_err(|e| e.to_string())?;
    let append = |entry: MenuItem<tauri::Wry>| menu.append(&entry).map_err(|e| e.to_string());
    if task.deleted {
        append(make("restore", "Restore Task")?)?;
    } else {
        if task.status != jev_tasks_core::Status::Done {
            append(make(
                if task.timer_started_at.is_some() {
                    "pause"
                } else {
                    "start"
                },
                if task.timer_started_at.is_some() {
                    "Pause Focus"
                } else {
                    "Focus on This Task"
                },
            )?)?;
        }
        append(make(
            if task.status == jev_tasks_core::Status::Done {
                "reopen"
            } else {
                "complete"
            },
            if task.status == jev_tasks_core::Status::Done {
                "Reopen Task"
            } else {
                "Mark Complete"
            },
        )?)?;
        append(make("edit", "Edit in Conversation…")?)?;
        append(make("delete", "Move to Trash")?)?;
    }
    menu.append(&Native::separator(app).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    append(make("copy", "Copy Task Reference")?)?;
    menu.popup({
        let window = app
            .get_webview_window("main")
            .ok_or("Window is unavailable.")?;
        let view: &tauri::Webview = window.as_ref();
        view.window()
    })
    .map_err(|e| e.to_string())
}

fn template_icon() -> Image<'static> {
    let mut pixels = vec![0_u8; 18 * 18 * 4];
    for y in 0..18 {
        for x in 0..18 {
            let dx = x as f64 - 8.5;
            let dy = y as f64 - 8.5;
            let radius = (dx * dx + dy * dy).sqrt();
            if (5.8..7.3).contains(&radius)
                || ((8..10).contains(&x) && (4..10).contains(&y))
                || ((8..13).contains(&x) && (8..10).contains(&y))
            {
                pixels[(y * 18 + x) * 4 + 3] = 255;
            }
        }
    }
    Image::new_owned(pixels, 18, 18)
}
pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_menu = Submenu::with_items(
        app,
        "Cadence",
        true,
        &[
            &Native::about(app, None, None)?,
            &Native::separator(app)?,
            &item(app, "settings", "Settings…", Some("CmdOrCtrl+,"))?,
            &Native::separator(app)?,
            &Native::services(app, None)?,
            &Native::hide(app, None)?,
            &Native::hide_others(app, None)?,
            &Native::show_all(app, None)?,
            &Native::separator(app)?,
            &Native::quit(app, None)?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &item(app, "new-task", "New Task", Some("CmdOrCtrl+N"))?,
            &item(
                app,
                "quick-capture",
                "Quick Capture…",
                Some("CmdOrCtrl+Shift+N"),
            )?,
            &Native::separator(app)?,
            &Native::close_window(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &Native::undo(app, None)?,
            &Native::redo(app, None)?,
            &Native::separator(app)?,
            &Native::cut(app, None)?,
            &Native::copy(app, None)?,
            &Native::paste(app, None)?,
            &Native::select_all(app, None)?,
            &Native::separator(app)?,
            &item(app, "find-task", "Find Task…", Some("CmdOrCtrl+F"))?,
        ],
    )?;
    let task = Submenu::with_items(
        app,
        "Task",
        true,
        &[
            &item(
                app,
                "toggle-timer",
                "Start / Pause Selected Task",
                Some("CmdOrCtrl+Return"),
            )?,
            &item(
                app,
                "complete-selected",
                "Complete Selected Task",
                Some("CmdOrCtrl+Shift+Return"),
            )?,
            &Native::separator(app)?,
            &item(
                app,
                "pause-running",
                "Pause Current Focus",
                Some("CmdOrCtrl+Shift+P"),
            )?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &Native::minimize(app, None)?,
            &Native::maximize(app, None)?,
            &Native::separator(app)?,
            &item(app, "focus-view", "Today", Some("CmdOrCtrl+1"))?,
            &item(app, "next-view", "All Tasks", Some("CmdOrCtrl+2"))?,
            &item(app, "journal-view", "Focus Journal", Some("CmdOrCtrl+3"))?,
        ],
    )?;
    app.set_menu(Menu::with_items(
        app,
        &[&app_menu, &file, &edit, &task, &window],
    )?)?;
    let info = MenuItem::with_id(app, "timer-info", "No task in focus", false, None::<&str>)?;
    let pause = item(app, "pause-running", "Pause Focus", None)?;
    let complete = item(app, "complete-running", "Complete Focused Task", None)?;
    let tray_menu = Menu::with_items(
        app,
        &[
            &info,
            &pause,
            &complete,
            &Native::separator(app)?,
            &item(app, "quick-capture", "Quick Capture…", None)?,
            &item(app, "open-main", "Open Cadence", None)?,
            &Native::separator(app)?,
            &item(app, "quit-app", "Quit Cadence", None)?,
        ],
    )?;
    let tray = TrayIconBuilder::with_id("focus-timer")
        .icon(template_icon())
        .icon_as_template(true)
        .tooltip("Cadence — focus timer")
        .menu(&tray_menu)
        .build(app)?;
    let stop = Arc::new(AtomicBool::new(false));
    app.manage(NativeState {
        tray,
        info,
        pause,
        complete,
        stop: stop.clone(),
    });
    let handle = app.clone();
    let path = app.state::<AppState>().data_dir.clone();
    std::thread::spawn(move || {
        // A native timer keeps the status item useful while every webview is hidden.
        let Ok(store) = database(&path, false) else {
            return;
        };
        while !stop.load(Ordering::Relaxed) {
            if let Ok(tasks) = store.tasks() {
                let running = tasks
                    .iter()
                    .find(|t| !t.deleted && t.timer_started_at.is_some());
                let state = handle.state::<NativeState>();
                if let Some(task) = running {
                    let seconds = task.elapsed_ms_at(chrono::Utc::now()) / 1000;
                    let elapsed = format!("{}:{:02}", seconds / 60, seconds % 60);
                    let _ = state.tray.set_title(Some(&elapsed));
                    let _ = state
                        .tray
                        .set_tooltip(Some(format!("{} · {elapsed}", task.title)));
                    let _ = state
                        .info
                        .set_text(format!("TASK-{:03} · {}", task.id, task.title));
                } else {
                    let _ = state.tray.set_title(None::<&str>);
                    let _ = state.tray.set_tooltip(Some("Cadence — no timer running"));
                    let _ = state.info.set_text("No task in focus");
                }
                let _ = state.pause.set_enabled(running.is_some());
                let _ = state.complete.set_enabled(running.is_some());
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    observe_workspace(app);
    Ok(())
}

struct Observer {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}
impl Drop for Observer {
    fn drop(&mut self) {
        unsafe {
            self.center.removeObserver((*self.token).as_ref());
        }
    }
}
thread_local! { static OBSERVERS: RefCell<Vec<Observer>> = const { RefCell::new(Vec::new()) }; }
fn observe_workspace(app: &AppHandle) {
    let workspace = unsafe { objc2_app_kit::NSWorkspace::sharedWorkspace() };
    let center = unsafe { workspace.notificationCenter() };
    for (notification, event) in [
        (
            unsafe { objc2_app_kit::NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification },
            "appearance",
        ),
        (
            unsafe { objc2_app_kit::NSWorkspaceWillSleepNotification },
            "sleep",
        ),
        (
            unsafe { objc2_app_kit::NSWorkspaceDidWakeNotification },
            "wake",
        ),
    ] {
        let handle = app.clone();
        let callback = block2::RcBlock::new(move |_: NonNull<NSNotification>| {
            if event == "sleep" {
                let state = handle.state::<AppState>();
                // SQLite serializes this short write; do not wait for a network chat's gate.
                for demo in [false, true] {
                    if demo && !state.data_dir.join("demo.sqlite").exists() {
                        continue;
                    }
                    if let Ok(mut store) = database(&state.data_dir, demo) {
                        if let Ok(tasks) = store.tasks() {
                            let actions: Vec<_> = tasks
                                .iter()
                                .filter(|t| t.timer_started_at.is_some() && !t.deleted)
                                .map(|t| Action::status("pause", t.id))
                                .collect();
                            if !actions.is_empty()
                                && store
                                    .apply(
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
                                    )
                                    .is_ok()
                            {
                                let _ = handle.emit(
                                    "tasks-changed",
                                    serde_json::json!({ "demo": demo, "reason": "sleep" }),
                                );
                            }
                        }
                    }
                }
            } else if event == "appearance" {
                let _ = get_appearance(handle.clone(), handle.state());
                let _ = handle.emit("appearance-changed", ());
            } else {
                let _ = handle.emit("tasks-changed", serde_json::json!({ "reason": "wake" }));
            }
        });
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(notification),
                None,
                None,
                &callback,
            )
        };
        OBSERVERS.with(|list| {
            list.borrow_mut().push(Observer {
                center: center.clone(),
                token,
            })
        });
    }
}
