//! AppKit integrations live here; task state still goes through the Rust store.
use crate::status_bar::{change, show_main};
use crate::{database, get_appearance, AppState};
use jev_tasks_core::{Action, Plan};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSString};
use std::{cell::RefCell, ptr::NonNull};
use tauri::{
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem as Native, Submenu},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

fn item(
    app: &AppHandle,
    id: &str,
    title: &str,
    shortcut: Option<&str>,
) -> tauri::Result<MenuItem<tauri::Wry>> {
    MenuItem::with_id(app, id, title, true, shortcut)
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
fn report(app: &AppHandle, result: Result<(), String>) {
    if let Err(error) = result {
        show_main(app);
        let _ = app.emit("native-error", error);
    }
}
pub fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "quick-capture" => report(app, quick_capture(app)),
        "settings" | "new-task" | "find-task" | "focus-view" | "next-view" | "journal-view"
        | "toggle-timer" | "complete-selected" => ui_action(app, id),
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
