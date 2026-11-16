//! Folder sync through iCloud Drive, OneDrive, Dropbox or any synced folder.
//!
//! Each device writes only its own file, `<root>/Cadence Sync/devices/<device-id>.json`,
//! containing every task it knows. Devices read the other files and merge them task by task:
//! the most recently updated version of a task wins. Because no two devices ever write the
//! same file, the cloud provider never has to resolve a write conflict, and stray conflict
//! copies are merged like any other file. The SQLite database itself never enters the folder.

use crate::model::{new_uid, Task};
use crate::store::{err, validate_task, Store};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;
pub const FOLDER: &str = "Cadence Sync";
const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

/// Links between tasks, expressed as uids so they survive different local numbering.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkUids {
    pub parent: Option<String>,
    pub milestone: Option<String>,
    pub cycle: Option<String>,
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTask {
    pub uid: String,
    pub task: Task,
    #[serde(default)]
    pub links: LinkUids,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFile {
    pub format: u32,
    pub device_id: String,
    pub device_name: String,
    pub exported_at: String,
    pub tasks: Vec<SyncTask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MergeReport {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

/// Canvas assignments get the same uid on every device, so two devices that both sync
/// Canvas converge on one task instead of creating duplicates.
pub fn canvas_uid(source: &crate::canvas::CanvasSource) -> String {
    format!("canvas:{}", source.key())
}

fn newer(a: &str, b: &str) -> bool {
    match (
        chrono::DateTime::parse_from_rfc3339(a),
        chrono::DateTime::parse_from_rfc3339(b),
    ) {
        (Ok(a), Ok(b)) => a > b,
        _ => a > b,
    }
}

impl Store {
    /// Gives every task a uid. Older tasks were created before sync existed.
    pub fn ensure_uids(&mut self) -> Result<usize, String> {
        let missing: Vec<Task> = self
            .tasks()?
            .into_iter()
            .filter(|t| t.uid.is_none())
            .collect();
        if missing.is_empty() {
            return Ok(0);
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        for mut task in missing.iter().cloned() {
            task.uid = Some(task.canvas.as_ref().map(canvas_uid).unwrap_or_else(new_uid));
            // Assigning identity is not an edit: updated_at stays as it was.
            tx.execute(
                "UPDATE tasks SET payload = ? WHERE id = ?",
                params![serde_json::to_string(&task).map_err(err)?, task.id],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(missing.len())
    }

    /// Every task (including Trash) in the portable sync format.
    pub fn export_sync(&self) -> Result<Vec<SyncTask>, String> {
        let tasks = self.tasks()?;
        let uid_of: BTreeMap<i64, String> = tasks
            .iter()
            .filter_map(|t| t.uid.clone().map(|u| (t.id, u)))
            .collect();
        let link = |id: Option<i64>| id.and_then(|id| uid_of.get(&id).cloned());
        Ok(tasks
            .iter()
            .filter_map(|t| {
                let uid = t.uid.clone()?;
                let o = &t.organization;
                Some(SyncTask {
                    uid,
                    links: LinkUids {
                        parent: link(o.parent_id),
                        milestone: link(o.milestone_id),
                        cycle: link(o.cycle_id),
                        related: o
                            .related_ids
                            .iter()
                            .filter_map(|id| uid_of.get(id).cloned())
                            .collect(),
                    },
                    task: t.clone(),
                })
            })
            .collect())
    }

    /// Merges tasks from other devices. A remote task replaces the local one only when it was
    /// updated more recently. New tasks keep their task number when it is free here.
    pub fn merge_sync(&mut self, records: Vec<SyncTask>) -> Result<MergeReport, String> {
        self.ensure_uids()?;
        let mut report = MergeReport::default();
        // Several files may carry the same task; keep the newest copy of each.
        let mut incoming: BTreeMap<String, SyncTask> = BTreeMap::new();
        for record in records {
            if record.uid.trim().is_empty() || record.uid.len() > 300 {
                report.skipped += 1;
                continue;
            }
            match incoming.get(&record.uid) {
                Some(existing) if !newer(&record.task.updated_at, &existing.task.updated_at) => {}
                _ => {
                    incoming.insert(record.uid.clone(), record);
                }
            }
        }
        let local = self.tasks()?;
        let mut id_of: BTreeMap<String, i64> = local
            .iter()
            .filter_map(|t| t.uid.clone().map(|u| (u, t.id)))
            .collect();
        let local_by_id: BTreeMap<i64, &Task> = local.iter().map(|t| (t.id, t)).collect();
        let mut used: BTreeSet<i64> = local.iter().map(|t| t.id).collect();
        // Decide what changes, and number new tasks: keep the remote number when it is free.
        let mut changes: Vec<(SyncTask, bool)> = vec![];
        let mut unnumbered = vec![];
        for (uid, record) in incoming {
            match id_of.get(&uid) {
                Some(id) => {
                    if newer(&record.task.updated_at, &local_by_id[id].updated_at) {
                        changes.push((record, false));
                    }
                }
                None => {
                    let wanted = record.task.id;
                    if wanted > 0 && used.insert(wanted) {
                        id_of.insert(uid, wanted);
                    } else {
                        unnumbered.push(uid);
                    }
                    changes.push((record, true));
                }
            }
        }
        let first_free = used.iter().max().copied().unwrap_or(0) + 1;
        for (id, uid) in (first_free..).zip(unnumbered) {
            id_of.insert(uid, id);
        }
        if changes.is_empty() {
            return Ok(report);
        }
        let build = |record: &SyncTask, keep_links: bool| -> Task {
            let mut task = record.task.clone();
            task.id = id_of[&record.uid];
            task.uid = Some(record.uid.clone());
            let o = &mut task.organization;
            let local_id = |uid: &Option<String>| uid.as_ref().and_then(|u| id_of.get(u).copied());
            if keep_links {
                o.parent_id = local_id(&record.links.parent);
                o.milestone_id = local_id(&record.links.milestone);
                o.cycle_id = local_id(&record.links.cycle);
                o.related_ids = record
                    .links
                    .related
                    .iter()
                    .filter_map(|u| id_of.get(u).copied())
                    .collect();
            } else {
                o.parent_id = None;
                o.milestone_id = None;
                o.cycle_id = None;
                o.related_ids.clear();
            }
            task
        };
        // Bad remote data must never leave the workspace in a state that blocks local edits,
        // so links are dropped if they don't validate, and the whole merge rolls back otherwise.
        for keep_links in [true, false] {
            let tx = self
                .connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            let mut attempt = MergeReport {
                skipped: report.skipped,
                ..Default::default()
            };
            for (record, created) in &changes {
                let task = build(record, keep_links);
                if validate_task(&task).is_err() {
                    attempt.skipped += 1;
                    continue;
                }
                tx.execute(
                    "INSERT INTO tasks(id, payload) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
                    params![task.id, serde_json::to_string(&task).map_err(err)?],
                )
                .map_err(err)?;
                if let Some(source) = &task.canvas {
                    // Lets this device's own Canvas sync recognize the assignment.
                    tx.execute(
                        "INSERT OR IGNORE INTO canvas_sources(source_key, task_id) VALUES (?, ?)",
                        params![source.key(), task.id],
                    )
                    .map_err(err)?;
                }
                if *created {
                    attempt.created += 1;
                } else {
                    attempt.updated += 1;
                }
            }
            let all: Vec<Task> = {
                let mut stmt = tx.prepare("SELECT id, payload FROM tasks").map_err(err)?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                    .map_err(err)?;
                let mut out = vec![];
                for row in rows {
                    let (id, payload) = row.map_err(err)?;
                    let mut t: Task = serde_json::from_str(&payload).map_err(err)?;
                    t.id = id;
                    out.push(t);
                }
                out
            };
            if crate::organization::validate(&all).is_ok() {
                tx.commit().map_err(err)?;
                return Ok(attempt);
            }
        }
        Err("Synced tasks had links that don't fit this workspace. Nothing was merged.".into())
    }
}

pub fn devices_dir(root: &Path) -> PathBuf {
    root.join(FOLDER).join("devices")
}

/// Writes this device's file atomically. Skips the write when the tasks are unchanged,
/// so idle devices don't keep uploading. Returns whether anything was written.
pub fn write_device_file(root: &Path, file: &DeviceFile) -> Result<bool, String> {
    if !root.is_dir() {
        return Err(format!("The sync folder {} is not available.", root.display()));
    }
    let dir = devices_dir(root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the sync folder: {e}"))?;
    let path = dir.join(format!("{}.json", file.device_id));
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if let Ok(previous) = serde_json::from_str::<DeviceFile>(&existing) {
            if previous.device_name == file.device_name
                && serde_json::to_string(&previous.tasks).ok()
                    == serde_json::to_string(&file.tasks).ok()
            {
                return Ok(false);
            }
        }
    }
    let temp = dir.join(format!(".{}.json.tmp", file.device_id));
    std::fs::write(&temp, serde_json::to_vec(file).map_err(err)?)
        .map_err(|e| format!("Could not write to the sync folder: {e}"))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("Could not write to the sync folder: {e}"))?;
    Ok(true)
}

#[derive(Debug, Default)]
pub struct FolderRead {
    pub files: Vec<DeviceFile>,
    pub warnings: Vec<String>,
    /// iCloud placeholders for files not downloaded to this Mac yet.
    pub not_downloaded: Vec<PathBuf>,
}

/// Reads every other device's file. Conflict copies made by the provider are read too.
pub fn read_device_files(root: &Path, own_device_id: &str) -> Result<FolderRead, String> {
    let mut out = FolderRead::default();
    let dir = devices_dir(root);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    let own = format!("{own_device_id}.json");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name.ends_with(".icloud") {
            out.not_downloaded.push(path);
            continue;
        }
        if name == own || name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        if entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) {
            out.warnings.push(format!("Skipped {name}: too large."));
            continue;
        }
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<DeviceFile>(&s).map_err(|e| e.to_string()))
        {
            Ok(file) if file.format > FORMAT => out.warnings.push(format!(
                "{} uses a newer Cadence. Update this device to sync with it.",
                file.device_name
            )),
            Ok(file) => out.files.push(file),
            // Usually a file still uploading; it will be read on the next pass.
            Err(_) => out.warnings.push(format!("Could not read {name} yet.")),
        }
    }
    out.files.sort_by(|a, b| a.device_id.cmp(&b.device_id));
    Ok(out)
}
