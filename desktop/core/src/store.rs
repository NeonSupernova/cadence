use crate::model::*;
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Store {
    connection: Connection,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(err)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)?;
        if version == 0 {
            connection.execute_batch("PRAGMA journal_mode=WAL;
          BEGIN IMMEDIATE;
          CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS canvas_sources (source_key TEXT PRIMARY KEY, task_id INTEGER NOT NULL UNIQUE);
          CREATE TABLE IF NOT EXISTS integration_state (key TEXT PRIMARY KEY, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS workload_outcomes (task_id INTEGER PRIMARY KEY, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS workload_checkins (date TEXT PRIMARY KEY, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS workload_reviews (date TEXT PRIMARY KEY, payload TEXT NOT NULL); PRAGMA user_version=1; COMMIT;").map_err(err)?;
        }
        Ok(Self { connection })
    }
    pub fn tasks(&self) -> Result<Vec<Task>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT id, payload FROM tasks ORDER BY id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(err)?;
        rows.map(|r| {
            let (id, text) = r.map_err(err)?;
            let mut t: Task = serde_json::from_str(&text).map_err(err)?;
            t.id = id;
            Ok(t)
        })
        .collect()
    }
    pub fn messages(&self) -> Result<Vec<Message>, String> {
        let mut statement = self.connection.prepare("SELECT payload FROM (SELECT id, payload FROM messages ORDER BY id DESC LIMIT 200) ORDER BY id").map_err(err)?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn canvas_report(&self) -> Result<Option<crate::canvas::CanvasReport>, String> {
        self.connection
            .query_row(
                "SELECT payload FROM integration_state WHERE key = 'canvas'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(err)?
            .map(|s| serde_json::from_str(&s).map_err(err))
            .transpose()
    }
    pub fn capacity_profile(&self) -> Result<crate::workload::CapacityProfile, String> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT payload FROM integration_state WHERE key = 'capacity'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        value
            .map(|s| serde_json::from_str(&s).map_err(err))
            .transpose()
            .map(|v| v.unwrap_or_default())
    }
    fn workload_rows<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
        order: &str,
    ) -> Result<Vec<T>, String> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT payload FROM {table} ORDER BY {order}"))
            .map_err(err)?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?;
        rows.map(|row| serde_json::from_str(&row.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn workload(&self, tasks: &[Task]) -> Result<crate::workload::Workload, String> {
        Ok(crate::workload::analyze(
            tasks,
            self.capacity_profile()?,
            &self.workload_rows(
                "workload_outcomes",
                "json_extract(payload,'$.recordedAt'), task_id",
            )?,
            &self.workload_rows("workload_reviews", "date")?,
            &self.workload_rows("workload_checkins", "date")?,
            Utc::now(),
        ))
    }
    pub fn save_capacity(&self, profile: crate::workload::CapacityProfile) -> Result<(), String> {
        profile.validate()?;
        self.connection.execute("INSERT INTO integration_state(key,payload) VALUES ('capacity',?) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(&profile).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn save_checkin(&self, checkin: crate::workload::Checkin) -> Result<(), String> {
        checkin.validate()?;
        self.connection.execute("INSERT INTO workload_checkins(date,payload) VALUES (?,?) ON CONFLICT(date) DO UPDATE SET payload=excluded.payload",params![checkin.date,serde_json::to_string(&checkin).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn save_estimate(
        &self,
        id: i64,
        estimate: crate::workload::Estimate,
    ) -> Result<(), String> {
        estimate.validate()?;
        let payload: String = self
            .connection
            .query_row("SELECT payload FROM tasks WHERE id = ?", [id], |r| r.get(0))
            .map_err(|_| "Task does not exist.")?;
        let mut task: Task = serde_json::from_str(&payload).map_err(err)?;
        task.estimate = Some(estimate);
        task.updated_at = Utc::now().to_rfc3339();
        self.connection
            .execute(
                "UPDATE tasks SET payload = ? WHERE id = ?",
                params![serde_json::to_string(&task).map_err(err)?, id],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn save_feedback(
        &self,
        id: i64,
        rating: u8,
        actual_minutes: Option<f64>,
    ) -> Result<(), String> {
        if !self.capacity_profile()?.learning_enabled {
            return Err("Learning is paused. Enable it before saving calibration feedback.".into());
        }
        let task = self
            .tasks()?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or("Task does not exist.")?;
        let outcome = crate::workload::feedback_outcome(
            &task,
            &crate::workload::Feedback {
                task_id: id,
                rating,
                actual_minutes,
            },
        )?;
        self.connection.execute("INSERT INTO workload_outcomes(task_id,payload) VALUES (?,?) ON CONFLICT(task_id) DO UPDATE SET payload=excluded.payload",params![id,serde_json::to_string(&outcome).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn review_day(&self, date: String, rating: String) -> Result<(), String> {
        if !["light", "sustainable", "overloaded"].contains(&rating.as_str()) {
            return Err("Choose light, sustainable, or overloaded.".into());
        }
        if !self.capacity_profile()?.learning_enabled {
            return Err("Learning is paused.".into());
        }
        let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| "Choose a valid day.")?;
        if day > Local::now().date_naive() {
            return Err("Review today or a past day, not a future day.".into());
        }
        let tasks = self.tasks()?;
        let workload = self.workload(&tasks)?;
        let checkins: Vec<crate::workload::Checkin> =
            self.workload_rows("workload_checkins", "date")?;
        let checkin = checkins.iter().find(|c| c.date == date);
        use chrono::Datelike;
        let minutes = checkin.map(|c| c.minutes).unwrap_or(
            workload.profile.weekday_minutes[day.weekday().num_days_from_monday() as usize],
        );
        let energy = checkin.map(|c| c.energy).unwrap_or(3);
        let (spent_minutes, spent_units) =
            crate::workload::spent_on(&tasks, day, &workload.calibration, Utc::now());
        if spent_minutes < 15.0 {
            return Err(
                "Track at least 15 minutes before using this day to calibrate capacity.".into(),
            );
        }
        let review = crate::workload::DayReview {
            date: date.clone(),
            available_minutes: minutes,
            energy,
            spent_minutes,
            spent_units,
            rating,
        };
        self.connection.execute("INSERT INTO workload_reviews(date,payload) VALUES (?,?) ON CONFLICT(date) DO UPDATE SET payload=excluded.payload",params![date,serde_json::to_string(&review).map_err(err)?]).map_err(err)?;
        Ok(())
    }
    pub fn import_canvas(
        &mut self,
        import: crate::canvas::CanvasImport,
    ) -> Result<crate::canvas::CanvasReport, String> {
        let now = Utc::now();
        let mut report = crate::canvas::CanvasReport {
            courses: import.courses,
            assignments: 0,
            created: 0,
            updated: 0,
            unchanged: 0,
            warnings: import.warnings,
            synced_at: now.to_rfc3339(),
        };
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let mut seen = std::collections::HashSet::new();
        for source in import.assignments {
            let key = source.key();
            if !seen.insert(key.clone()) {
                continue;
            }
            report.assignments += 1;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT task_id FROM canvas_sources WHERE source_key = ?",
                    [&key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            let mut task = if let Some(id) = existing {
                let payload: String = tx
                    .query_row("SELECT payload FROM tasks WHERE id = ?", [id], |r| r.get(0))
                    .map_err(err)?;
                let mut task: Task = serde_json::from_str(&payload).map_err(err)?;
                let before = serde_json::to_string(&task).map_err(err)?;
                if let Some(previous) = &task.canvas {
                    if task.title == previous.title {
                        task.title = source.title.clone();
                    }
                    if task.description == previous.description() {
                        task.description = source.description();
                    }
                    if task.due_date == previous.due_date {
                        task.due_date = source.due_date.clone();
                    }
                    if task.priority == previous.suggested_priority {
                        task.priority = source.suggested_priority;
                    }
                    if !previous.submitted && source.submitted && task.status != Status::Done {
                        task.stop_timer_at(now);
                        task.status = Status::Done;
                        task.completed_at = source
                            .submitted_at
                            .clone()
                            .or_else(|| Some(now.to_rfc3339()));
                    }
                    if task.status == Status::Done
                        && task.completed_at.is_none()
                        && source.submitted
                    {
                        task.completed_at = source.submitted_at.clone();
                    }
                }
                for label in ["canvas".into(), "school".into(), source.course_label()] {
                    if !task.labels.contains(&label) {
                        task.labels.push(label);
                    }
                }
                // Updating only last-sync metadata is not counted as a task edit.
                let mut old_source = task.canvas.clone();
                if let Some(s) = &mut old_source {
                    s.synced_at = source.synced_at.clone();
                }
                let source_changed = serde_json::to_string(&old_source).map_err(err)?
                    != serde_json::to_string(&Some(&source)).map_err(err)?;
                let changed =
                    before != serde_json::to_string(&task).map_err(err)? || source_changed;
                if changed {
                    task.updated_at = now.to_rfc3339();
                    report.updated += 1;
                } else {
                    report.unchanged += 1;
                }
                task
            } else {
                report.created += 1;
                Task {
                    id: 0,
                    title: source.title.clone(),
                    description: source.description(),
                    raw: format!(
                        "Canvas assignment: {} ({})",
                        source.title, source.course_code
                    ),
                    kind: "assignment".into(),
                    labels: vec!["canvas".into(), "school".into(), source.course_label()],
                    priority: source.suggested_priority,
                    status: if source.submitted {
                        Status::Done
                    } else {
                        Status::Open
                    },
                    confidence: 1.0,
                    created_at: now.to_rfc3339(),
                    updated_at: now.to_rfc3339(),
                    completed_at: if source.submitted {
                        source.submitted_at.clone()
                    } else {
                        None
                    },
                    due_date: source.due_date.clone(),
                    deleted: false,
                    tracked_ms: 0,
                    timer_started_at: None,
                    time_entries: vec![],
                    canvas: None,
                    estimate: None,
                    baseline_estimate: None,
                    organization: Default::default(),
                }
            };
            task.canvas = Some(source);
            validate_task(&task)?;
            if task.id == 0 {
                tx.execute(
                    "INSERT INTO tasks(payload) VALUES (?)",
                    [serde_json::to_string(&task).map_err(err)?],
                )
                .map_err(err)?;
                task.id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO canvas_sources(source_key, task_id) VALUES (?, ?)",
                    params![key, task.id],
                )
                .map_err(err)?;
            }
            tx.execute(
                "UPDATE tasks SET payload = ? WHERE id = ?",
                params![serde_json::to_string(&task).map_err(err)?, task.id],
            )
            .map_err(err)?;
        }
        tx.execute("INSERT INTO integration_state(key,payload) VALUES ('canvas',?) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload", [serde_json::to_string(&report).map_err(err)?]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(report)
    }
    pub fn snapshot(
        &self,
        view: View,
        focus_ids: Vec<i64>,
        configured: bool,
        model: String,
        demo: bool,
    ) -> Result<Snapshot, String> {
        view.validate()?;
        let tasks = self.tasks()?;
        let workload = self.workload(&tasks)?;
        let mut shown = visible(&tasks, &view);
        if view.sort == "fit" {
            shown.sort_by(|a, b| {
                (a.status == Status::Done)
                    .cmp(&(b.status == Status::Done))
                    .then(
                        workload.assessments[&b.id]
                            .fit_score
                            .total_cmp(&workload.assessments[&a.id].fit_score),
                    )
                    .then(a.id.cmp(&b.id))
            });
        }
        Ok(Snapshot {
            next_task_id: workload.next_task_id,
            workload,
            show_list: false,
            visible: shown,
            stats: stats(&tasks),
            tasks,
            messages: self.messages()?,
            view,
            focus_ids,
            configured,
            model,
            demo,
        })
    }
    pub fn apply(
        &mut self,
        plan: Plan,
        input: &str,
        classification: Option<&Classification>,
        record_chat: bool,
    ) -> Result<Vec<i64>, String> {
        if plan.actions.len() > 50 {
            return Err("Please change at most 50 tasks at a time.".into());
        }
        if let Some(view) = &plan.view {
            view.validate()?;
        }
        let tasks = self.tasks()?;
        let learning_enabled = plan
            .capacity
            .as_ref()
            .map(|p| p.learning_enabled)
            .unwrap_or(self.capacity_profile()?.learning_enabled);
        for id in &plan.focus_ids {
            if !tasks.iter().any(|t| t.id == *id) {
                return Err(format!("TASK-{id:03} does not exist."));
            }
        }
        if let Some(c) = &plan.checkin {
            c.validate()?;
        }
        if let Some(p) = &plan.capacity {
            p.validate()?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let mut touched = Vec::new();
        for action in &plan.actions {
            if ![
                "create",
                "update",
                "complete",
                "reopen",
                "start",
                "pause",
                "delete",
                "restore",
                "add_time",
                "set_time",
                "remove_time",
            ]
            .contains(&action.kind.as_str())
            {
                return Err("Unknown task operation.".into());
            }
            if action.kind == "create" && !action.task_ids.is_empty() {
                return Err("New tasks cannot reuse existing IDs.".into());
            }
            if action.kind != "create" && action.task_ids.is_empty() {
                return Err("Please specify which task to change.".into());
            }
            if action.task_ids.iter().any(|id| *id <= 0) {
                return Err("Task IDs must be positive numbers.".into());
            }
            let now = Utc::now().to_rfc3339();
            let ids = if action.kind == "create" {
                vec![0]
            } else {
                action.task_ids.clone()
            };
            for id in ids {
                let mut task = if id == 0 {
                    let c = classification.ok_or("Task classification is missing.")?;
                    Task {
                        id: 0,
                        title: String::new(),
                        description: String::new(),
                        raw: input.into(),
                        kind: c.kind.clone(),
                        labels: c.labels.clone(),
                        priority: c.priority,
                        confidence: c.confidence,
                        status: Status::Open,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                        completed_at: None,
                        due_date: None,
                        deleted: false,
                        tracked_ms: 0,
                        timer_started_at: None,
                        time_entries: vec![],
                        canvas: None,
                        estimate: c.estimate.clone(),
                        baseline_estimate: None,
                        organization: Default::default(),
                    }
                } else {
                    let payload: String = tx
                        .query_row("SELECT payload FROM tasks WHERE id = ?", [id], |r| r.get(0))
                        .map_err(|_| format!("TASK-{id:03} does not exist. No changes saved."))?;
                    let mut task: Task = serde_json::from_str(&payload).map_err(err)?;
                    task.id = id;
                    task
                };
                if task.deleted && !["restore", "delete"].contains(&action.kind.as_str()) {
                    return Err(format!(
                        "Restore TASK-{id:03} from Trash before changing it."
                    ));
                }
                let previous_tracked_ms = task.tracked_ms;
                match action.kind.as_str() {
                    "complete" => {
                        task.stop_timer_at(Utc::now());
                        if task.status != Status::Done {
                            task.completed_at = Some(now.clone());
                        }
                        task.status = Status::Done;
                    }
                    "reopen" => {
                        task.stop_timer_at(Utc::now());
                        task.status = Status::Open;
                        task.completed_at = None;
                    }
                    "pause" => task.stop_timer_at(Utc::now()),
                    "start" => {
                        if task.status == Status::Done {
                            return Err(format!("Reopen TASK-{id:03} before starting its timer."));
                        }
                        // One running timer per workspace. Pause the previous task atomically.
                        let tick = Utc::now();
                        let others: Vec<(i64, String)> = {
                            let mut statement = tx
                                .prepare("SELECT id, payload FROM tasks WHERE id != ?")
                                .map_err(err)?;
                            let rows = statement
                                .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
                                .map_err(err)?;
                            rows.collect::<Result<Vec<_>, _>>().map_err(err)?
                        };
                        for (other_id, payload) in others {
                            let mut other: Task = serde_json::from_str(&payload).map_err(err)?;
                            if other.timer_started_at.is_some() {
                                other.stop_timer_at(tick);
                                other.updated_at = tick.to_rfc3339();
                                tx.execute(
                                    "UPDATE tasks SET payload = ? WHERE id = ?",
                                    params![serde_json::to_string(&other).map_err(err)?, other_id],
                                )
                                .map_err(err)?;
                            }
                        }
                        if task.organization.role != "task" {
                            return Err("Start a task inside this planning item instead.".into());
                        }
                        task.status = Status::InProgress;
                        if task.baseline_estimate.is_none() {
                            task.baseline_estimate =
                                Some(crate::workload::Estimate::for_task(&task));
                        }
                        if task.timer_started_at.is_none() {
                            task.timer_started_at = Some(tick.to_rfc3339());
                        }
                    }
                    "delete" => {
                        task.stop_timer_at(Utc::now());
                        task.deleted = true;
                    }
                    "restore" => task.deleted = false,
                    "add_time" => {
                        if task.organization.role != "task" {
                            return Err("Only tasks can record work time.".into());
                        }
                        let minutes = action
                            .minutes
                            .filter(|m| (1..=1440).contains(m))
                            .ok_or("Enter 1–1440 minutes of work time.")?;
                        let date = action.date.as_deref().ok_or("Choose a work date.")?;
                        let day = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                            .map_err(|_| "Choose a valid work date.")?;
                        if day > Local::now().date_naive() {
                            return Err("Work time cannot be logged in the future.".into());
                        }
                        let at = day
                            .and_hms_opt(12, 0, 0)
                            .unwrap()
                            .and_local_timezone(Local)
                            .single()
                            .ok_or("Choose a valid work date.")?
                            .to_rfc3339();
                        let duration_ms = u64::from(minutes) * 60_000;
                        task.tracked_ms = task
                            .tracked_ms
                            .checked_add(duration_ms)
                            .ok_or("Tracked time is too large.")?;
                        task.time_entries.push(TimeEntry {
                            started_at: at.clone(),
                            ended_at: at,
                            duration_ms,
                            manual: true,
                        });
                    }
                    "set_time" | "remove_time" => {
                        let index = action.entry_index.ok_or("Choose a recorded session.")?;
                        let previous = task
                            .time_entries
                            .get(index)
                            .ok_or("That recorded session no longer exists.")?
                            .duration_ms;
                        task.tracked_ms = task.tracked_ms.saturating_sub(previous);
                        if action.kind == "remove_time" {
                            task.time_entries.remove(index);
                        } else {
                            let minutes = action
                                .minutes
                                .filter(|m| (1..=1440).contains(m))
                                .ok_or("Enter 1–1440 minutes of work time.")?;
                            let duration_ms = u64::from(minutes) * 60_000;
                            task.tracked_ms = task
                                .tracked_ms
                                .checked_add(duration_ms)
                                .ok_or("Tracked time is too large.")?;
                            let entry = &mut task.time_entries[index];
                            if !entry.manual {
                                let end = chrono::DateTime::parse_from_rfc3339(&entry.ended_at)
                                    .map_err(|_| "Recorded session has an invalid end time.")?;
                                entry.started_at = (end
                                    - chrono::Duration::minutes(i64::from(minutes)))
                                .to_rfc3339();
                            }
                            entry.duration_ms = duration_ms;
                        }
                    }
                    "create" | "update" => {
                        if let Some(v) = &action.title {
                            task.title = v.trim().into();
                        }
                        if let Some(v) = &action.description {
                            task.description = v.trim().into();
                        }
                        if let Some(v) = &action.labels {
                            task.labels = v
                                .iter()
                                .map(|l| l.trim().trim_start_matches('#').to_lowercase())
                                .filter(|s| !s.is_empty())
                                .collect();
                            task.labels.sort();
                            task.labels.dedup();
                        }
                        if let Some(v) = &action.task_type {
                            task.kind = v.clone();
                        }
                        if let Some(v) = action.priority {
                            task.priority = v;
                        }
                        if let Some(organization) = &action.organization {
                            task.organization = organization.clone();
                        }
                        if let Some(estimate) = &action.estimate {
                            estimate.validate()?;
                            task.estimate = Some(estimate.clone());
                        }
                        if let Some(v) = &action.due_date {
                            task.due_date = if v.is_empty() { None } else { Some(v.clone()) };
                        }
                    }
                    _ => unreachable!(),
                }
                if task.canvas.is_some() && !task.labels.iter().any(|l| l == "canvas") {
                    task.labels.push("canvas".into());
                }
                if action.kind == "create" {
                    if task.description.trim().is_empty() {
                        return Err(
                            "Describe the intended outcome before creating this task.".into()
                        );
                    }
                    if task.labels.is_empty() {
                        task.labels.push("inbox".into());
                    }
                }
                validate_task(&task)?;
                task.updated_at = now.clone();
                if id == 0 {
                    tx.execute(
                        "INSERT INTO tasks(payload) VALUES (?)",
                        [serde_json::to_string(&task).map_err(err)?],
                    )
                    .map_err(err)?;
                    task.id = tx.last_insert_rowid();
                }
                tx.execute(
                    "UPDATE tasks SET payload = ? WHERE id = ?",
                    params![serde_json::to_string(&task).map_err(err)?, task.id],
                )
                .map_err(err)?;
                if task.status == Status::Done
                    && ["complete", "add_time", "set_time", "remove_time"]
                        .contains(&action.kind.as_str())
                {
                    let estimate = task
                        .baseline_estimate
                        .clone()
                        .unwrap_or_else(|| crate::workload::Estimate::for_task(&task));
                    let actual = task.tracked_ms as f64 / 60_000.0;
                    let reliable = (5.0..=960.0).contains(&actual)
                        && !task
                            .time_entries
                            .iter()
                            .any(|e| e.duration_ms > 4 * 60 * 60 * 1000);
                    let old: Option<crate::workload::Outcome> = tx
                        .query_row(
                            "SELECT payload FROM workload_outcomes WHERE task_id = ?",
                            [task.id],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()
                        .map_err(err)?
                        .map(|s| serde_json::from_str(&s).map_err(err))
                        .transpose()?;
                    if (learning_enabled && reliable) || old.is_some() {
                        // Feedback may contain an explicit actual duration. Keep that value
                        // when changing the recorded sessions later.
                        let explicit = old.as_ref().is_some_and(|o| {
                            (o.actual_minutes - previous_tracked_ms as f64 / 60_000.0).abs() > 0.001
                        });
                        let outcome = crate::workload::Outcome {
                            task_id: task.id,
                            kind: task.kind.clone(),
                            estimate,
                            actual_minutes: if explicit {
                                old.as_ref().unwrap().actual_minutes
                            } else {
                                actual
                            },
                            effort_rating: old.as_ref().and_then(|o| o.effort_rating),
                            recorded_at: now.clone(),
                            time_reliable: if explicit {
                                old.as_ref().unwrap().time_reliable
                            } else {
                                reliable
                            },
                        };
                        tx.execute("INSERT INTO workload_outcomes(task_id,payload) VALUES (?,?) ON CONFLICT(task_id) DO UPDATE SET payload=excluded.payload",params![task.id,serde_json::to_string(&outcome).map_err(err)?]).map_err(err)?;
                    }
                }
                if action.kind == "reopen" {
                    tx.execute("DELETE FROM workload_outcomes WHERE task_id = ?", [task.id])
                        .map_err(err)?;
                }
                touched.push(task.id);
            }
        }
        if let Some(checkin) = &plan.checkin {
            tx.execute("INSERT INTO workload_checkins(date,payload) VALUES (?,?) ON CONFLICT(date) DO UPDATE SET payload=excluded.payload",params![checkin.date,serde_json::to_string(checkin).map_err(err)?]).map_err(err)?;
        }
        if let Some(profile) = &plan.capacity {
            tx.execute("INSERT INTO integration_state(key,payload) VALUES ('capacity',?) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(profile).map_err(err)?]).map_err(err)?;
        }
        if let Some(feedback) = &plan.feedback {
            if !learning_enabled {
                return Err("Learning is paused.".into());
            }
            let payload: String = tx
                .query_row(
                    "SELECT payload FROM tasks WHERE id = ?",
                    [feedback.task_id],
                    |r| r.get(0),
                )
                .map_err(|_| "Feedback task does not exist.")?;
            let task: Task = serde_json::from_str(&payload).map_err(err)?;
            let outcome = crate::workload::feedback_outcome(&task, feedback)?;
            tx.execute("INSERT INTO workload_outcomes(task_id,payload) VALUES (?,?) ON CONFLICT(task_id) DO UPDATE SET payload=excluded.payload",params![feedback.task_id,serde_json::to_string(&outcome).map_err(err)?]).map_err(err)?;
        }
        let all_tasks = {
            let mut stmt = tx.prepare("SELECT payload FROM tasks").map_err(err)?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
            let mut result = Vec::new();
            for row in rows {
                result.push(serde_json::from_str::<Task>(&row.map_err(err)?).map_err(err)?);
            }
            result
        };
        crate::organization::validate(&all_tasks)?;
        touched.extend(plan.focus_ids);
        touched.sort();
        touched.dedup();
        if record_chat {
            for (role, content, ids) in [
                ("user", input, Vec::new()),
                ("assistant", plan.reply.as_str(), touched.clone()),
            ] {
                let mut m = Message {
                    id: 0,
                    role: role.into(),
                    content: content.into(),
                    task_ids: ids,
                    created_at: Utc::now().to_rfc3339(),
                };
                tx.execute("INSERT INTO messages(payload) VALUES ('{}')", [])
                    .map_err(err)?;
                m.id = tx.last_insert_rowid();
                tx.execute(
                    "UPDATE messages SET payload = ? WHERE id = ?",
                    params![serde_json::to_string(&m).map_err(err)?, m.id],
                )
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(touched)
    }
    pub fn seed_demo(&mut self) -> Result<(), String> {
        if !self.tasks()?.is_empty() {
            return Ok(());
        }
        let today = Local::now().format("%Y-%m-%d").to_string();
        let examples = [
            (
                "Fix checkout crash",
                "Checkout crashes on payment. Fix it before launch.",
                "bug",
                vec!["work", "coding", "urgent"],
                96,
                Some(today.clone()),
            ),
            (
                "Finish the database assignment",
                "Complete the database assignment and review the queries.",
                "chore",
                vec!["school", "coding"],
                82,
                Some(today.clone()),
            ),
            (
                "Plan the weekend trip",
                "Choose a place to stay and a few things to do.",
                "personal",
                vec!["home", "planning"],
                55,
                None,
            ),
            (
                "Explore vector databases",
                "Compare options for the next project.",
                "research",
                vec!["coding", "work"],
                43,
                None,
            ),
            (
                "Book a dentist appointment",
                "Schedule a routine checkup.",
                "personal",
                vec!["health"],
                30,
                None,
            ),
            (
                "Send the weekly update",
                "Share this week's progress with the team.",
                "chore",
                vec!["work", "writing"],
                50,
                None,
            ),
        ];
        for (title, description, kind, labels, priority, due_date) in examples {
            let c = Classification {
                kind: kind.into(),
                labels: labels.iter().map(|s| s.to_string()).collect(),
                priority,
                confidence: 0.96,

                estimate: None,
            };
            let action = Action {
                kind: "create".into(),
                task_ids: vec![],
                title: Some(title.into()),
                description: Some(description.into()),
                labels: None,
                task_type: None,
                priority: None,
                due_date,
                organization: None,
                estimate: None,
                minutes: None,
                date: None,
                entry_index: None,
            };
            self.apply(
                Plan {
                    reply: String::new(),
                    actions: vec![action],
                    view: None,
                    focus_ids: vec![],
                    ..Default::default()
                },
                description,
                Some(&c),
                false,
            )?;
        }
        self.apply(
            Plan {
                reply: String::new(),
                actions: vec![Action::status("complete", 6), Action::status("start", 1)],
                view: None,
                focus_ids: vec![],
                ..Default::default()
            },
            "",
            None,
            false,
        )?;
        Ok(())
    }
}
fn validate_task(t: &Task) -> Result<(), String> {
    if t.title.is_empty() || t.title.chars().count() > 140 {
        return Err("Task titles must be 1–140 characters.".into());
    }
    if t.description.chars().count() > 20_000 {
        return Err("Task descriptions must be at most 20,000 characters.".into());
    }
    if !TYPES.iter().any(|(kind, _)| *kind == t.kind) {
        return Err("Unknown task type.".into());
    }
    if t.priority > 100 {
        return Err("Priority must be between 0 and 100.".into());
    }
    if t.labels.len() > 20 || t.labels.iter().any(|l| l.chars().count() > 40) {
        return Err("Use up to 20 labels, each under 40 characters.".into());
    }
    if let Some(date) = &t.due_date {
        NaiveDateCheck::validate(date)?;
    }
    Ok(())
}
struct NaiveDateCheck;
impl NaiveDateCheck {
    fn validate(date: &str) -> Result<(), String> {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| "Use a valid date in YYYY-MM-DD format.".to_string())?;
        if date.len() != 10 {
            return Err("Use YYYY-MM-DD for dates.".into());
        }
        Ok(())
    }
}

/// A delayed model response must not overwrite task changes made while it was thinking.
pub fn validate_chat_version(before: &[u8], tasks: &[Task], plan: &Plan) -> Result<(), String> {
    let changes = !plan.actions.is_empty()
        || plan.feedback.is_some()
        || plan.checkin.is_some()
        || plan.capacity.is_some();
    if changes && before != serde_json::to_vec(tasks).map_err(err)? {
        return Err("Tasks changed while the assistant was thinking. Your changes are safe; send the message again to use the latest state.".into());
    }
    Ok(())
}
