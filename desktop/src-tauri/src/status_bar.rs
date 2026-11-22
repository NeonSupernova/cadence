//! The always-available status item: the macOS menu bar extra, the Windows notification
//! area icon, and the Linux app indicator. Also drives the Dock / taskbar / launcher badge
//! and progress bar. Task state still goes through the Rust store.
use crate::{database, notifications, AppState};
use jev_tasks_core::{alerts, Action, Plan, Task};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    window::{ProgressBarState, ProgressBarStatus},
    AppHandle, Emitter, Manager, Wry,
};

pub struct StatusState {
    tray: TrayIcon,
    rendered: Mutex<Rendered>,
    pub stop: Arc<AtomicBool>,
}

/// What is currently on screen, so native objects are only touched when something changes.
#[derive(Default, PartialEq)]
struct Rendered {
    menu: String,
    title: Option<String>,
    tooltip: String,
    badge: i64,
    progress: Option<(u64, bool)>,
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn ui_action(app: &AppHandle, payload: serde_json::Value) {
    show_main(app);
    let _ = app.emit_to("main", "native-action", payload);
}

pub(crate) fn change(app: &AppHandle, kind: &str, id: i64, demo: bool) -> Result<(), String> {
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
    let _ = app.emit("tasks-changed", serde_json::json!({ "demo": demo, "id": id }));
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

/// Handles status item menu ids. Returns false for ids that belong to other menus.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    match id {
        "open-main" => show_main(app),
        "quit-app" => app.exit(0),
        "pause-running" => report(app, running_change(app, "pause")),
        "complete-running" => report(app, running_change(app, "complete")),
        "due-view" => ui_action(app, serde_json::json!({ "action": "due-view" })),
        "alerts-pause" => report(app, notifications::pause(app, Some(60)).map(|_| ())),
        "alerts-resume" => report(app, notifications::pause(app, None).map(|_| ())),
        _ => match id.strip_prefix("tray-task:").and_then(|n| n.parse::<i64>().ok()) {
            Some(task_id) => ui_action(app, serde_json::json!({ "action": "inspect", "id": task_id, "demo": false })),
            None => return false,
        },
    }
    true
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.into()
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}

fn clock(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// Everything the status item shows, derived from tasks, reminders, and alert settings.
struct View {
    running: Option<(Task, u64)>,
    due: Vec<Task>,
    badge: i64,
    next_reminder: Option<String>,
    paused_until: Option<String>,
    settings: alerts::AlertSettings,
}

fn view(tasks: &[Task], data_dir: &std::path::Path) -> View {
    let now = chrono::Local::now();
    let settings = alerts::load_settings(data_dir);
    let reminders = if settings.reminders { alerts::load_reminders(data_dir) } else { vec![] };
    let running = tasks
        .iter()
        .find(|t| !t.deleted && t.timer_started_at.is_some())
        .map(|t| (t.clone(), t.elapsed_ms_at(chrono::Utc::now()) / 1000));
    let due: Vec<Task> = alerts::due_now(tasks, now.date_naive()).into_iter().cloned().collect();
    let next_reminder = alerts::next_reminder(&reminders).and_then(|r| {
        let at = r.at_utc()?.with_timezone(&chrono::Local);
        let when = if at.date_naive() == now.date_naive() { at.format("%-I:%M %p") } else { at.format("%a %-I:%M %p") };
        Some(format!("Next reminder · {when} · {}", clip(&r.title, 32)))
    });
    let paused_until = settings
        .paused_until
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .filter(|until| *until > now)
        .map(|until| until.with_timezone(&chrono::Local).format("%-I:%M %p").to_string());
    View {
        badge: alerts::badge_count(tasks, settings.badge, now.date_naive()),
        running,
        due,
        next_reminder,
        paused_until,
        settings,
    }
}

const MENU_TASKS: usize = 6;

/// A stable description of the menu; it is rebuilt only when this changes.
fn menu_signature(v: &View) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut parts = vec![
        v.running.as_ref().map(|(t, _)| format!("run:{}:{}", t.id, t.title)).unwrap_or_default(),
        format!("due:{}", v.due.len()),
        v.next_reminder.clone().unwrap_or_default(),
        v.paused_until.clone().unwrap_or_default(),
        today,
    ];
    parts.extend(v.due.iter().take(MENU_TASKS).map(|t| format!("{}:{}:{:?}", t.id, t.title, t.due_date)));
    parts.join("|")
}

fn build_menu(app: &AppHandle, v: &View) -> tauri::Result<Menu<Wry>> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let menu = Menu::new(app)?;
    let label = |text: String| MenuItem::with_id(app, format!("label:{text}"), text, false, None::<&str>);
    let action = |id: &str, text: &str, enabled: bool| MenuItem::with_id(app, id, text, enabled, None::<&str>);
    let sep = || PredefinedMenuItem::separator(app);

    menu.append(&label(match &v.running {
        Some((t, _)) => format!("◉ TASK-{:03} · {}", t.id, clip(&t.title, 40)),
        None => "No task in focus".into(),
    })?)?;
    menu.append(&action("pause-running", "Pause Focus", v.running.is_some())?)?;
    menu.append(&action("complete-running", "Complete Focused Task", v.running.is_some())?)?;
    menu.append(&sep()?)?;

    if v.due.is_empty() {
        menu.append(&label("Nothing due today".into())?)?;
    } else {
        menu.append(&label(format!("Due today & overdue · {}", v.due.len()))?)?;
        for task in v.due.iter().take(MENU_TASKS) {
            let when = if task.due_date.as_deref() == Some(today.as_str()) { "today" } else { "overdue" };
            let marker = if when == "overdue" { "⚠︎ " } else { "" };
            menu.append(&action(
                &format!("tray-task:{}", task.id),
                &format!("{marker}TASK-{:03} · {} — {when}", task.id, clip(&task.title, 36)),
                true,
            )?)?;
        }
        if v.due.len() > MENU_TASKS {
            menu.append(&action("due-view", &format!("Show all {} due…", v.due.len()), true)?)?;
        }
    }
    if let Some(next) = &v.next_reminder {
        menu.append(&label(next.clone())?)?;
    }
    menu.append(&sep()?)?;
    match &v.paused_until {
        Some(until) => menu.append(&action("alerts-resume", &format!("Resume Alerts (paused until {until})"), true)?)?,
        None => menu.append(&action("alerts-pause", "Pause Alerts for 1 Hour", v.settings.enabled)?)?,
    }
    #[cfg(target_os = "macos")]
    menu.append(&action("quick-capture", "Quick Capture…", true)?)?;
    menu.append(&action("open-main", "Open Cadence", true)?)?;
    menu.append(&sep()?)?;
    menu.append(&action("quit-app", "Quit Cadence", true)?)?;
    Ok(menu)
}

fn title(v: &View) -> Option<String> {
    let timer = v.running.as_ref().map(|(_, s)| clock(*s));
    let due = (!v.due.is_empty()).then(|| format!("{} due", v.due.len()));
    match v.settings.tray_title {
        alerts::TrayTitle::None => None,
        alerts::TrayTitle::Timer => timer,
        alerts::TrayTitle::Due => due,
        alerts::TrayTitle::Both => match (timer, due) {
            (Some(t), Some(d)) => Some(format!("{t} · {d}")),
            (t, d) => t.or(d),
        },
    }
}

fn tooltip(v: &View) -> String {
    let mut parts = vec!["Cadence".to_string()];
    match &v.running {
        Some((t, s)) => parts.push(format!("{} · {}", clip(&t.title, 40), clock(*s))),
        None => parts.push("no timer running".into()),
    }
    if !v.due.is_empty() {
        parts.push(format!("{} due", v.due.len()));
    }
    if v.paused_until.is_some() {
        parts.push("alerts paused".into());
    }
    parts.join(" — ")
}

/// Running-timer progress against its estimate, as a percentage and whether it ran over.
fn progress(v: &View) -> Option<(u64, bool)> {
    if !v.settings.dock_progress {
        return None;
    }
    let (task, seconds) = v.running.as_ref()?;
    let minutes = task.estimate.as_ref()?.minutes.max(1) as u64;
    let percent = seconds * 100 / (minutes * 60);
    Some((percent.min(100), percent >= 100))
}

fn render(app: &AppHandle, v: View) {
    let state = app.state::<StatusState>();
    let Ok(mut current) = state.rendered.lock() else { return };
    let next = Rendered {
        menu: menu_signature(&v),
        title: title(&v),
        tooltip: tooltip(&v),
        badge: v.badge,
        progress: progress(&v),
    };
    if next.menu != current.menu {
        if let Ok(menu) = build_menu(app, &v) {
            let _ = state.tray.set_menu(Some(menu));
        }
    }
    if next.title != current.title {
        // Shown beside the icon on macOS and as the indicator label on Linux; Windows has no title.
        let _ = state.tray.set_title(next.title.as_deref());
    }
    if next.tooltip != current.tooltip {
        let _ = state.tray.set_tooltip(Some(&next.tooltip));
    }
    if let Some(window) = app.get_webview_window("main") {
        if next.badge != current.badge {
            set_badge(&window, next.badge);
        }
        if next.progress != current.progress {
            let _ = window.set_progress_bar(match next.progress {
                Some((percent, over)) => ProgressBarState {
                    status: Some(if over { ProgressBarStatus::Error } else { ProgressBarStatus::Normal }),
                    progress: Some(percent),
                },
                None => ProgressBarState { status: Some(ProgressBarStatus::None), progress: None },
            });
        }
    }
    *current = next;
}

/// The Dock badge on macOS, the launcher count on Linux (Unity API: GNOME Dash to Dock,
/// KDE, Plank), and a taskbar overlay icon on Windows, which has no numeric badge API.
fn set_badge(window: &tauri::WebviewWindow, count: i64) {
    #[cfg(windows)]
    {
        let _ = window.set_overlay_icon((count > 0).then(|| badge_icon(count)));
    }
    #[cfg(not(windows))]
    {
        let _ = window.set_badge_count((count > 0).then_some(count));
    }
}

/// 3×5 bitmap digits for the Windows taskbar overlay.
#[cfg_attr(not(windows), allow(dead_code))]
fn badge_icon(count: i64) -> Image<'static> {
    const GLYPHS: [[u8; 5]; 11] = [
        [7, 5, 5, 5, 7], [2, 6, 2, 2, 7], [7, 1, 7, 4, 7], [7, 1, 7, 1, 7], [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7], [7, 4, 7, 5, 7], [7, 1, 1, 1, 1], [7, 5, 7, 5, 7], [7, 5, 7, 1, 7],
        [0, 2, 7, 2, 0], // "+"
    ];
    const SIZE: usize = 16;
    let mut pixels = vec![0_u8; SIZE * SIZE * 4];
    let mut put = |x: usize, y: usize, rgb: [u8; 3]| {
        if x < SIZE && y < SIZE {
            pixels[(y * SIZE + x) * 4..][..4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f64 - 7.5, y as f64 - 7.5);
            if dx * dx + dy * dy <= 64.0 {
                put(x, y, [214, 48, 49]);
            }
        }
    }
    let glyphs: Vec<usize> = if count > 9 { vec![9, 10] } else { vec![count.clamp(0, 9) as usize] };
    let scale = if glyphs.len() == 1 { 2 } else { 1 };
    let width = glyphs.len() * 3 * scale + (glyphs.len() - 1) * scale;
    let (left, top) = ((SIZE - width) / 2, (SIZE - 5 * scale) / 2);
    for (i, g) in glyphs.iter().enumerate() {
        for (row, bits) in GLYPHS[*g].iter().enumerate() {
            for col in 0..3 {
                if bits & (4 >> col) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put(left + (i * 4 + col) * scale + sx, top + row * scale + sy, [255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
    Image::new_owned(pixels, SIZE as u32, SIZE as u32)
}

/// Monochrome template glyph for the macOS menu bar, which tints it for light/dark.
#[cfg(target_os = "macos")]
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
    let builder = TrayIconBuilder::with_id("cadence-status").tooltip("Cadence");
    #[cfg(target_os = "macos")]
    let builder = builder.icon(template_icon()).icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    };
    // Windows convention: left click opens the app, right click opens the menu.
    #[cfg(windows)]
    let builder = builder.show_menu_on_left_click(false).on_tray_icon_event(|tray, event| {
        if let tauri::tray::TrayIconEvent::Click {
            button: tauri::tray::MouseButton::Left,
            button_state: tauri::tray::MouseButtonState::Up,
            ..
        } = event
        {
            show_main(tray.app_handle());
        }
    });
    let placeholder = Menu::with_items(app, &[&MenuItem::with_id(app, "open-main", "Open Cadence", true, None::<&str>)?])?;
    let tray = builder.menu(&placeholder).build(app)?;
    let stop = Arc::new(AtomicBool::new(false));
    app.manage(StatusState { tray, rendered: Mutex::new(Rendered::default()), stop: stop.clone() });

    let handle = app.clone();
    let path = app.state::<AppState>().data_dir.clone();
    std::thread::spawn(move || {
        // A native loop keeps the status item current while every webview is hidden.
        let Ok(store) = database(&path, false) else { return };
        while !stop.load(Ordering::Relaxed) {
            if let Ok(tasks) = store.tasks() {
                let v = view(&tasks, &path);
                let main = handle.clone();
                // Menus, tray, and Dock calls belong on the main thread.
                let _ = handle.run_on_main_thread(move || render(&main, v));
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    Ok(())
}
