//! Folder sync service: iCloud Drive, OneDrive, Dropbox, Google Drive or any synced folder.
//! The merge rules live in `jev_tasks_core::sync`; this module finds folders, keeps settings
//! and runs a background pass every few seconds.
use crate::{database, AppState};
use jev_tasks_core::sync::{read_device_files, write_device_file, DeviceFile, MergeReport, FORMAT};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    folder: Option<String>,
    device_id: String,
    device_name: String,
}
impl SyncConfig {
    fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("sync-settings.json")
    }
    fn load(data_dir: &Path) -> Self {
        if let Some(config) = std::fs::read_to_string(Self::path(data_dir))
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
        {
            return config;
        }
        let config = Self {
            enabled: false,
            folder: None,
            device_id: jev_tasks_core::new_uid(),
            device_name: computer_name(),
        };
        let _ = config.save(data_dir);
        config
    }
    fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::write(
            Self::path(data_dir),
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Could not save sync settings: {e}"))
    }
}

fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("/usr/sbin/scutil")
        .args(["--get", "ComputerName"])
        .output()
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "This computer".into())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    name: String,
    path: String,
}

/// Synced folders that exist on this computer.
fn providers() -> Vec<Provider> {
    let mut found = vec![];
    let mut add = |name: String, path: PathBuf| {
        if path.is_dir() && !found.iter().any(|p: &Provider| Path::new(&p.path) == path) {
            found.push(Provider {
                name,
                path: path.to_string_lossy().into(),
            });
        }
    };
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if let Some(home) = &home {
        add(
            "iCloud Drive".into(),
            home.join("Library/Mobile Documents/com~apple~CloudDocs"),
        );
        add("iCloud Drive".into(), home.join("iCloudDrive"));
        if let Ok(entries) = std::fs::read_dir(home.join("Library/CloudStorage")) {
            for entry in entries.flatten() {
                let raw = entry.file_name().to_string_lossy().to_string();
                let name = if let Some(account) = raw.strip_prefix("OneDrive-") {
                    format!("OneDrive ({account})")
                } else if raw == "OneDrive" {
                    "OneDrive".into()
                } else if raw.starts_with("Dropbox") {
                    "Dropbox".into()
                } else if let Some(account) = raw.strip_prefix("GoogleDrive-") {
                    // Google Drive keeps files under "My Drive".
                    add(format!("Google Drive ({account})"), entry.path().join("My Drive"));
                    continue;
                } else {
                    raw.clone()
                };
                add(name, entry.path());
            }
        }
        add("Dropbox".into(), home.join("Dropbox"));
        add("OneDrive".into(), home.join("OneDrive"));
    }
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(path) = std::env::var_os(var) {
            add(
                if var == "OneDriveCommercial" { "OneDrive (work)" } else { "OneDrive" }.into(),
                PathBuf::from(path),
            );
        }
    }
    found
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerDevice {
    name: String,
    exported_at: String,
    tasks: usize,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    last_synced_at: Option<String>,
    devices: Vec<PeerDevice>,
    report: Option<MergeReport>,
    warnings: Vec<String>,
    error: Option<String>,
}
pub struct SyncState {
    gate: Mutex<()>,
    progress: Mutex<Progress>,
    seen: Mutex<String>,
    pub stop: AtomicBool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    enabled: bool,
    folder: Option<String>,
    device_name: String,
    providers: Vec<Provider>,
    #[serde(flatten)]
    progress: Progress,
}
fn status(app: &AppHandle) -> SyncStatus {
    let config = SyncConfig::load(&app.state::<AppState>().data_dir);
    SyncStatus {
        enabled: config.enabled,
        folder: config.folder,
        device_name: config.device_name,
        providers: providers(),
        progress: app
            .state::<SyncState>()
            .progress
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default(),
    }
}

/// One pass: merge other devices' files if they changed, then publish this device's tasks.
fn sync_once(app: &AppHandle, force: bool) -> Result<MergeReport, String> {
    let state = app.state::<AppState>();
    let sync = app.state::<SyncState>();
    let _pass = sync.gate.try_lock().map_err(|_| "Sync is already running.")?;
    let config = SyncConfig::load(&state.data_dir);
    let root = match (&config.folder, config.enabled) {
        (Some(folder), true) => PathBuf::from(folder),
        _ => return Err("Choose a sync folder and turn sync on.".into()),
    };
    if !root.is_dir() {
        return Err(format!(
            "The sync folder isn't available right now ({}). Is the drive signed in?",
            root.display()
        ));
    }
    let read = read_device_files(&root, &config.device_id)?;
    #[cfg(target_os = "macos")]
    for placeholder in &read.not_downloaded {
        // Ask iCloud to download files it has evicted; they are read on a later pass.
        let _ = std::process::Command::new("/usr/bin/brctl")
            .arg("download")
            .arg(placeholder)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    let fingerprint = read
        .files
        .iter()
        .map(|f| format!("{}@{}", f.device_id, f.exported_at))
        .collect::<Vec<_>>()
        .join("|");
    let changed = force
        || sync
            .seen
            .lock()
            .map(|seen| *seen != fingerprint)
            .unwrap_or(true);
    let mut report = MergeReport::default();
    if changed && !read.files.is_empty() {
        let records = read.files.iter().flat_map(|f| f.tasks.clone()).collect();
        let _tasks = state.gate.lock().map_err(|_| "Could not merge synced tasks.")?;
        report = database(&state.data_dir, false)?.merge_sync(records)?;
    }
    if let Ok(mut seen) = sync.seen.lock() {
        *seen = fingerprint;
    }
    let mut store = database(&state.data_dir, false)?;
    store.ensure_uids()?;
    write_device_file(
        &root,
        &DeviceFile {
            format: FORMAT,
            device_id: config.device_id.clone(),
            device_name: config.device_name.clone(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            tasks: store.export_sync()?,
        },
    )?;
    if let Ok(mut progress) = sync.progress.lock() {
        progress.last_synced_at = Some(chrono::Utc::now().to_rfc3339());
        progress.devices = read
            .files
            .iter()
            .filter(|f| f.device_id != config.device_id)
            .map(|f| PeerDevice {
                name: f.device_name.clone(),
                exported_at: f.exported_at.clone(),
                tasks: f.tasks.len(),
            })
            .collect();
        if report.created + report.updated > 0 || progress.report.is_none() {
            progress.report = Some(report.clone());
        }
        progress.warnings = read.warnings.clone();
        if !read.not_downloaded.is_empty() {
            progress
                .warnings
                .push("Waiting for iCloud to download another device's tasks.".into());
        }
        progress.error = None;
    }
    if report.created + report.updated > 0 {
        let _ = app.emit(
            "tasks-changed",
            serde_json::json!({ "demo": false, "reason": "sync" }),
        );
    }
    Ok(report)
}
fn run(app: &AppHandle, force: bool) -> Result<MergeReport, String> {
    let result = sync_once(app, force);
    if let Err(error) = &result {
        if error != "Sync is already running." {
            if let Ok(mut progress) = app.state::<SyncState>().progress.lock() {
                progress.error = Some(error.clone());
            }
        }
    }
    let _ = app.emit("sync-status", status(app));
    result
}

#[tauri::command]
pub fn get_sync_settings(app: AppHandle) -> SyncStatus {
    status(&app)
}
#[tauri::command]
pub async fn save_sync_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
    folder: Option<String>,
    device_name: String,
) -> Result<SyncStatus, String> {
    let mut config = SyncConfig::load(&state.data_dir);
    let folder = folder
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty());
    if let Some(path) = &folder {
        let path = Path::new(path);
        if !path.is_absolute() || !path.is_dir() {
            return Err("Choose an existing folder (a full path).".into());
        }
    } else if enabled {
        return Err("Choose a folder to sync through.".into());
    }
    let name = device_name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Give this device a name of 1–60 characters.".into());
    }
    if config.folder != folder {
        // A new folder starts fresh: merge everything found there on the first pass.
        if let Ok(mut seen) = app.state::<SyncState>().seen.lock() {
            seen.clear();
        }
    }
    config.enabled = enabled;
    config.folder = folder;
    config.device_name = name.into();
    config.save(&state.data_dir)?;
    if let Ok(mut progress) = app.state::<SyncState>().progress.lock() {
        *progress = Progress::default();
    }
    if enabled {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || run(&handle, true))
            .await
            .map_err(|_| "Sync was interrupted.".to_string())??;
    }
    Ok(status(&app))
}
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<SyncStatus, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || run(&handle, true))
        .await
        .map_err(|_| "Sync was interrupted.".to_string())??;
    Ok(status(&app))
}

pub fn start_background(app: &AppHandle) {
    app.manage(SyncState {
        gate: Mutex::new(()),
        progress: Mutex::new(Progress::default()),
        seen: Mutex::new(String::new()),
        stop: AtomicBool::new(false),
    });
    let app = app.clone();
    std::thread::spawn(move || loop {
        if app.state::<SyncState>().stop.load(Ordering::Relaxed) {
            break;
        }
        if SyncConfig::load(&app.state::<AppState>().data_dir).enabled {
            let _ = run(&app, false);
        }
        // Frequent, cheap passes: unchanged files are neither re-merged nor rewritten.
        std::thread::sleep(std::time::Duration::from_secs(5));
    });
}
