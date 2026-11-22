//! Deliberately narrow PostHog capture. No task, chat, or Canvas content is sent as usage data.
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Write, path::Path, process::Stdio};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

const US_HOST: &str = "https://us.i.posthog.com";
const EU_HOST: &str = "https://eu.i.posthog.com";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryConfig {
    enabled: bool,
    project_token: String,
    host: String,
    distinct_id: String,
}

impl TelemetryConfig {
    fn load(data_dir: &Path) -> Self {
        let saved = std::fs::read_to_string(data_dir.join("telemetry.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok());
        let token = std::env::var("POSTHOG_PROJECT_TOKEN")
            .ok()
            .or_else(|| option_env!("POSTHOG_PROJECT_TOKEN").map(str::to_string))
            .unwrap_or_default();
        let host = std::env::var("POSTHOG_HOST")
            .ok()
            .or_else(|| option_env!("POSTHOG_HOST").map(str::to_string))
            .unwrap_or_else(|| US_HOST.into());
        let fresh = saved.is_none();
        let mut config = saved.unwrap_or(Self {
            enabled: true,
            project_token: token.clone(),
            host: if host == EU_HOST { host } else { US_HOST.into() },
            distinct_id: Uuid::new_v4().to_string(),
        });
        let changed = config.project_token.is_empty() && !token.is_empty();
        if changed {
            config.project_token = token;
        }
        if fresh || changed {
            let _ = config.save(data_dir);
        }
        config
    }

    fn save(&self, data_dir: &Path) -> Result<(), String> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(data_dir.join("telemetry.json")).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        }
        file.write_all(serde_json::to_string_pretty(self).map_err(|e| e.to_string())?.as_bytes())
            .map_err(|e| e.to_string())
    }

    fn configured(&self) -> bool {
        !self.project_token.trim().is_empty() && [US_HOST, EU_HOST].contains(&self.host.as_str())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    configured: bool,
    enabled: bool,
    host: String,
}

impl From<&TelemetryConfig> for TelemetrySettings {
    fn from(config: &TelemetryConfig) -> Self {
        Self { configured: config.configured(), enabled: config.enabled, host: config.host.clone() }
    }
}

#[tauri::command]
pub fn get_telemetry_settings(state: State<'_, AppState>) -> TelemetrySettings {
    TelemetrySettings::from(&TelemetryConfig::load(&state.data_dir))
}

#[tauri::command]
pub fn save_telemetry_settings(
    state: State<'_, AppState>,
    enabled: bool,
    project_token: Option<String>,
    host: String,
) -> Result<TelemetrySettings, String> {
    if ![US_HOST, EU_HOST].contains(&host.as_str()) {
        return Err("Choose the US or EU PostHog region.".into());
    }
    let mut config = TelemetryConfig::load(&state.data_dir);
    if let Some(token) = project_token.filter(|s| !s.trim().is_empty()) {
        if token.len() > 200 || token.contains(['\r', '\n']) {
            return Err("Enter a valid PostHog project token.".into());
        }
        config.project_token = token.trim().into();
    }
    config.enabled = enabled;
    config.host = host;
    config.save(&state.data_dir)?;
    Ok(TelemetrySettings::from(&config))
}

fn send(config: TelemetryConfig, event: &str, properties: Value, distinct_id: String) -> Result<(), String> {
    if !config.configured() {
        return Err("Connect PostHog in Settings → Analytics before sending feedback.".into());
    }
    let body = json!({
        "token": config.project_token,
        "event": event,
        "distinct_id": distinct_id,
        "properties": properties,
    });
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("curl.exe");
        cmd.creation_flags(0x0800_0000);
        cmd
    };
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("curl");
    let mut child = command
        .args(["--fail", "--silent", "--show-error", "--max-time", "10", "-H", "Content-Type: application/json", "--data-binary", "@-", &format!("{}/i/v0/e/", config.host)])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("Could not start feedback delivery: {e}"))?;
    child.stdin.take().ok_or("Could not send feedback.")?
        .write_all(body.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("PostHog did not accept the event: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

const USAGE_EVENTS: &[&str] = &[
    "app_opened", "view_opened", "canvas_assignment_opened", "canvas_sync_requested",
    "task_started", "task_completed", "time_logged", "chat_opened", "update_check_requested",
];
const AREAS: &[&str] = &["today", "canvas", "tasks", "capacity", "journal"];

#[tauri::command]
pub fn capture_usage_event(app: AppHandle, event: String, area: Option<String>) -> Result<(), String> {
    if !USAGE_EVENTS.contains(&event.as_str()) || area.as_ref().is_some_and(|a| !AREAS.contains(&a.as_str())) {
        return Err("Unsupported usage event.".into());
    }
    let config = TelemetryConfig::load(&app.state::<AppState>().data_dir);
    if !config.enabled || !config.configured() { return Ok(()); }
    let properties = json!({"app_version": app.package_info().version.to_string(), "platform": std::env::consts::OS, "area": area});
    std::thread::spawn(move || {
        if let Err(error) = send(config.clone(), &event, properties, config.distinct_id.clone()) {
            eprintln!("Usage event delivery failed: {error}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn submit_feedback(app: AppHandle, kind: String, message: String) -> Result<(), String> {
    if !["bug", "idea", "other"].contains(&kind.as_str()) {
        return Err("Choose bug, idea, or other feedback.".into());
    }
    let message = message.trim();
    if message.len() < 5 || message.len() > 3000 {
        return Err("Write 5–3000 characters of feedback.".into());
    }
    let config = TelemetryConfig::load(&app.state::<AppState>().data_dir);
    let distinct_id = if config.enabled { config.distinct_id.clone() } else { format!("feedback-{}", Uuid::new_v4()) };
    let properties = json!({"kind":kind,"message":message,"app_version":app.package_info().version.to_string(),"platform":std::env::consts::OS});
    tauri::async_runtime::spawn_blocking(move || send(config, "feedback_submitted", properties, distinct_id))
        .await.map_err(|e| e.to_string())?
}
