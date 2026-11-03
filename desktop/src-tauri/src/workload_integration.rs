use crate::{database, snapshot, AppState, Request};
use jev_tasks_core::{
    workload::{CapacityProfile, Checkin, Estimate},
    Config, Router, Snapshot,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn save_task_estimate(
    state: State<'_, AppState>,
    request: Request,
    id: i64,
    mut estimate: Estimate,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    estimate.source = "user".into();
    database(&state.data_dir, request.demo)?.save_estimate(id, estimate)?;
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
pub fn save_capacity_profile(
    state: State<'_, AppState>,
    request: Request,
    profile: CapacityProfile,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    database(&state.data_dir, request.demo)?.save_capacity(profile)?;
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
pub fn save_day_checkin(
    state: State<'_, AppState>,
    request: Request,
    checkin: Checkin,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    database(&state.data_dir, request.demo)?.save_checkin(checkin)?;
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
pub fn rate_task_effort(
    state: State<'_, AppState>,
    request: Request,
    id: i64,
    rating: u8,
    actual_minutes: Option<f64>,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    database(&state.data_dir, request.demo)?.save_feedback(id, rating, actual_minutes)?;
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
pub fn review_capacity_day(
    state: State<'_, AppState>,
    request: Request,
    date: String,
    rating: String,
) -> Result<Snapshot, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    database(&state.data_dir, request.demo)?.review_day(date, rating)?;
    snapshot(&state.data_dir, request, vec![])
}
#[tauri::command]
pub async fn assess_task_effort(
    app: AppHandle,
    request: Request,
    id: i64,
) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let tasks = database(&state.data_dir, request.demo)?.tasks()?;
        let task = tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or("Task does not exist.")?;
        let router = Router {
            config: Config::load(&state.data_dir),
        };
        let (classification, _) = router.classify(
            &format!(
                "Estimate the work required, not an instruction to modify a task: {}\n{}",
                task.title, task.description
            ),
            &tasks,
        )?;
        let estimate = classification
            .estimate
            .ok_or("Jev returned no workload assessment.")?;
        let _guard = state
            .gate
            .lock()
            .map_err(|_| "Could not save the assessment.")?;
        let current = database(&state.data_dir, request.demo)?
            .tasks()?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or("Task no longer exists.")?;
        if current
            .estimate
            .as_ref()
            .is_some_and(|e| e.source == "user")
        {
            return Err(
                "This task has a personal estimate. Edit it directly to preserve your assessment."
                    .into(),
            );
        }
        database(&state.data_dir, request.demo)?.save_estimate(id, estimate)?;
        snapshot(&state.data_dir, request, vec![])
    })
    .await
    .map_err(|_| "Workload assessment was interrupted.".to_string())?
}

#[tauri::command]
pub fn open_task_reference(state: State<'_, AppState>, request: Request, id: i64, index: usize) -> Result<(), String> {
    let task = database(&state.data_dir, request.demo)?.tasks()?.into_iter().find(|t|t.id==id).ok_or("Task does not exist.")?;
    let link = task.organization.external.get(index).ok_or("Reference does not exist.")?;
    let url = tauri::Url::parse(&link.url).map_err(|_|"Invalid reference URL.")?;
    if !["http","https"].contains(&url.scheme()) || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() { return Err("Invalid reference URL.".into()); }
    #[cfg(target_os = "macos")]
    unsafe {
        let value = objc2_foundation::NSString::from_str(url.as_str());
        let native_url = objc2_foundation::NSURL::URLWithString(&value).ok_or("Invalid reference URL.")?;
        if !objc2_app_kit::NSWorkspace::sharedWorkspace().openURL(&native_url) { return Err("Could not open the reference in your browser.".into()); }
    }
    #[cfg(not(target_os = "macos"))]
    return Err("Opening references currently requires macOS.".into());
    Ok(())
}
