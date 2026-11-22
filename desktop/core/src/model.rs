use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const TYPES: &[(&str, &str)] = &[
    ("bug", "Fix broken or incorrect behavior"),
    ("feature", "Build a new capability"),
    ("chore", "Maintenance, setup or routine work"),
    ("research", "Investigate, learn or compare"),
    ("meeting", "Schedule or attend a conversation"),
    ("personal", "Personal errands, home, health or life"),
    ("assignment", "Coursework with a deliverable or submission"),
];
pub const LABELS: &[(&str, &str)] = &[
    ("work", "Professional or job related work"),
    ("school", "School, coursework or studying"),
    ("canvas", "An assignment or task sourced from Canvas LMS"),
    ("coding", "Programming or software"),
    ("writing", "Writing, editing or documentation"),
    ("planning", "Organizing or planning"),
    ("finance", "Money, bills or budgets"),
    ("health", "Health, exercise or appointments"),
    ("home", "Household, family or errands"),
    (
        "urgent",
        "Explicitly time sensitive or blocking important work",
    ),
    ("blocked", "Waiting for a dependency"),
    ("quick-win", "Explicitly a quick, low effort task"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Stable identity across synced devices. Local task numbers can differ between devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default)]
    pub organization: crate::organization::Organization,
    pub id: i64,
    pub title: String,
    pub description: String,
    pub raw: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub labels: Vec<String>,
    pub priority: u8,
    pub status: Status,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    pub due_date: Option<String>,
    pub deleted: bool,
    #[serde(default)]
    pub tracked_ms: u64,
    #[serde(default)]
    pub timer_started_at: Option<String>,
    #[serde(default)]
    pub time_entries: Vec<TimeEntry>,
    #[serde(default)]
    pub canvas: Option<crate::canvas::CanvasSource>,
    #[serde(default)]
    pub estimate: Option<crate::workload::Estimate>,
    #[serde(default)]
    pub baseline_estimate: Option<crate::workload::Estimate>,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}

/// A small, checkable step inside a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
    pub title: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub done_at: Option<String>,
}

pub const MAX_SUBTASKS: usize = 30;

/// A random 128-bit identifier, hex encoded. Uses std's per-process random hash keys.
pub fn new_uid() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// How timers behave across the workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerSettings {
    /// Allow several tasks to run timers at the same time.
    pub multiple: bool,
    /// Automatically stop a session after this many minutes. 0 disables the cutoff.
    pub cap_minutes: u32,
}
impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            multiple: true,
            cap_minutes: 240,
        }
    }
}
impl TimerSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.cap_minutes != 0 && !(15..=1440).contains(&self.cap_minutes) {
            return Err("Choose a timer cutoff between 15 minutes and 24 hours, or turn it off.".into());
        }
        Ok(())
    }
    pub fn cap_ms(&self) -> Option<u64> {
        (self.cap_minutes > 0).then(|| u64::from(self.cap_minutes) * 60_000)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub manual: bool,
    /// The session hit the timer cutoff and was stopped automatically.
    #[serde(default)]
    pub auto_stopped: bool,
}
impl Task {
    pub fn elapsed_ms_at(&self, now: chrono::DateTime<chrono::Utc>) -> u64 {
        self.tracked_ms.saturating_add(
            self.timer_started_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|start| {
                    (now - start.with_timezone(&chrono::Utc))
                        .num_milliseconds()
                        .max(0) as u64
                })
                .unwrap_or(0),
        )
    }
    pub fn stop_timer_at(&mut self, now: chrono::DateTime<chrono::Utc>) {
        self.stop_timer_capped(now, None);
    }
    /// Stops the running session, ending it at the cutoff when it ran longer than `cap_ms`.
    pub fn stop_timer_capped(&mut self, now: chrono::DateTime<chrono::Utc>, cap_ms: Option<u64>) {
        if let Some(started_at) = self.timer_started_at.take() {
            if let Ok(start) = chrono::DateTime::parse_from_rfc3339(&started_at) {
                let start = start.with_timezone(&chrono::Utc);
                let mut end = now.max(start);
                let mut auto_stopped = false;
                if let Some(cap) = cap_ms {
                    let limit = start + chrono::Duration::milliseconds(cap as i64);
                    if end > limit {
                        end = limit;
                        auto_stopped = true;
                    }
                }
                let duration_ms = (end - start).num_milliseconds().max(0) as u64;
                self.tracked_ms = self.tracked_ms.saturating_add(duration_ms);
                self.time_entries.push(TimeEntry {
                    started_at,
                    ended_at: end.to_rfc3339(),
                    duration_ms,
                    manual: false,
                    auto_stopped,
                });
            }
        }
    }
    /// True when the running session has passed the cutoff.
    pub fn timer_over_cap(&self, now: chrono::DateTime<chrono::Utc>, cap_ms: Option<u64>) -> bool {
        let (Some(cap), Some(start)) = (
            cap_ms,
            self.timer_started_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()),
        ) else {
            return false;
        };
        (now - start.with_timezone(&chrono::Utc)).num_milliseconds() > cap as i64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub total: usize,
    pub active: usize,
    pub completed: usize,
    pub high_priority: usize,
    pub due_today: usize,
    pub overdue: usize,
    pub completion_rate: usize,
    pub labels: BTreeMap<String, usize>,
    pub types: BTreeMap<String, usize>,
    pub completed_this_week: usize,
    pub tracked_ms: u64,
    pub tracked_today_ms: u64,
    pub running_task_id: Option<i64>,
    pub running_task_ids: Vec<i64>,
}

pub fn stats(tasks: &[Task]) -> Stats {
    let mut s = Stats::default();
    let today = Local::now().date_naive();
    let now = chrono::Utc::now();
    let midnight = today
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(Local).earliest())
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or(now);
    for t in tasks.iter().filter(|t| !t.deleted) {
        s.tracked_ms = s.tracked_ms.saturating_add(t.elapsed_ms_at(now));
        if t.timer_started_at.is_some() {
            s.running_task_id = Some(t.id);
            s.running_task_ids.push(t.id);
        }
        for entry in &t.time_entries {
            if entry.manual {
                if chrono::DateTime::parse_from_rfc3339(&entry.ended_at)
                    .ok()
                    .is_some_and(|end| end.with_timezone(&Local).date_naive() == today)
                {
                    s.tracked_today_ms = s.tracked_today_ms.saturating_add(entry.duration_ms);
                }
                continue;
            }
            if let (Ok(start), Ok(end)) = (
                chrono::DateTime::parse_from_rfc3339(&entry.started_at),
                chrono::DateTime::parse_from_rfc3339(&entry.ended_at),
            ) {
                s.tracked_today_ms = s.tracked_today_ms.saturating_add(
                    (end.with_timezone(&chrono::Utc).min(now)
                        - start.with_timezone(&chrono::Utc).max(midnight))
                    .num_milliseconds()
                    .max(0) as u64,
                );
            }
        }
        if let Some(start) = t
            .timer_started_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        {
            s.tracked_today_ms = s.tracked_today_ms.saturating_add(
                (now - start.with_timezone(&chrono::Utc).max(midnight))
                    .num_milliseconds()
                    .max(0) as u64,
            );
        }
        s.total += 1;
        if t.status == Status::Done {
            s.completed += 1;
            let completed = t.completed_at.as_deref().or_else(|| {
                if t.canvas.is_none() {
                    Some(t.updated_at.as_str())
                } else {
                    None
                }
            });
            if let Some(date) = completed.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            {
                if date.with_timezone(&Local).date_naive() >= today - chrono::Duration::days(6) {
                    s.completed_this_week += 1;
                }
            }
        } else {
            s.active += 1;
            if t.priority >= 75 {
                s.high_priority += 1;
            }
            if let Some(date) = &t.due_date {
                if let Ok(date) = NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                    if date == today {
                        s.due_today += 1;
                    }
                    if date < today {
                        s.overdue += 1;
                    }
                }
            }
        }
        for label in &t.labels {
            *s.labels.entry(label.clone()).or_default() += 1;
        }
        *s.types.entry(t.kind.clone()).or_default() += 1;
    }
    s.completion_rate = (s.completed * 100).checked_div(s.total).unwrap_or(0);
    s
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub status: String,
    pub query: String,
    pub label: String,
    pub task_type: String,
    pub sort: String,
}
impl Default for View {
    fn default() -> Self {
        Self {
            status: "open".into(),
            query: String::new(),
            label: String::new(),
            task_type: String::new(),
            sort: "fit".into(),
        }
    }
}
impl View {
    pub fn validate(&self) -> Result<(), String> {
        if ![
            "all",
            "open",
            "in_progress",
            "done",
            "deleted",
            "today",
            "high",
        ]
        .contains(&self.status.as_str())
        {
            return Err("Unknown task view.".into());
        }
        if ![
            "priority", "newest", "oldest", "title", "due", "time", "fit",
        ]
        .contains(&self.sort.as_str())
        {
            return Err("Unknown sort order.".into());
        }
        Ok(())
    }
}
pub fn visible(tasks: &[Task], view: &View) -> Vec<Task> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let query = view.query.to_lowercase();
    let now = chrono::Utc::now();
    let mut items: Vec<_> = tasks
        .iter()
        .filter(|t| {
            if view.status == "deleted" {
                if !t.deleted {
                    return false;
                }
            } else if t.deleted {
                return false;
            }
            let matches_status = match view.status.as_str() {
                "open" => t.status != Status::Done,
                "in_progress" => t.status == Status::InProgress,
                "done" => t.status == Status::Done,
                "today" => {
                    t.status != Status::Done && t.due_date.as_ref().is_some_and(|d| d <= &today)
                }
                "high" => t.status != Status::Done && t.priority >= 75,
                _ => true,
            };
            matches_status
                && (view.label.is_empty() || t.labels.contains(&view.label))
                && (view.task_type.is_empty() || t.kind == view.task_type)
                && (query.is_empty()
                    || format!(
                        "TASK-{:03} #{} {} {} {} {}",
                        t.id,
                        t.id,
                        t.title,
                        t.description,
                        t.raw,
                        t.labels.join(" ")
                    )
                    .to_lowercase()
                    .contains(&query))
        })
        .cloned()
        .collect();
    items.sort_by(|a, b| {
        (a.status == Status::Done)
            .cmp(&(b.status == Status::Done))
            .then_with(|| match view.sort.as_str() {
                "newest" => b.id.cmp(&a.id),
                "oldest" => a.id.cmp(&b.id),
                "title" => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                "time" => b.elapsed_ms_at(now).cmp(&a.elapsed_ms_at(now)),
                "due" => a
                    .due_date
                    .as_deref()
                    .unwrap_or("9999")
                    .cmp(b.due_date.as_deref().unwrap_or("9999")),
                _ => b.priority.cmp(&a.priority),
            })
            .then(a.id.cmp(&b.id))
    });
    items
}

/// The next unblocked task, ranked from actual deadlines and Jev priority.
pub fn next_task(tasks: &[Task]) -> Option<&Task> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    tasks
        .iter()
        .filter(|t| {
            !t.deleted
                && t.status != Status::Done
                && t.timer_started_at.is_none()
                && !t.labels.iter().any(|l| l == "blocked")
        })
        .min_by(|a, b| {
            let due = |t: &Task| t.due_date.as_ref().is_some_and(|d| d <= &today);
            due(b)
                .cmp(&due(a))
                .then(b.priority.cmp(&a.priority))
                .then(
                    a.due_date
                        .as_deref()
                        .unwrap_or("9999")
                        .cmp(b.due_date.as_deref().unwrap_or("9999")),
                )
                .then(a.id.cmp(&b.id))
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    #[serde(default)]
    pub organization: Option<crate::organization::Organization>,
    pub kind: String,
    pub task_ids: Vec<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub labels: Option<Vec<String>>,
    pub task_type: Option<String>,
    pub priority: Option<u8>,
    pub due_date: Option<String>,
    #[serde(default)]
    pub estimate: Option<crate::workload::Estimate>,
    #[serde(default)]
    pub minutes: Option<u32>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub entry_index: Option<usize>,
    #[serde(default)]
    pub subtasks: Option<Vec<String>>,
}
/// Fields of the manual "New task" form.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskDraft {
    pub title: String,
    pub description: String,
    pub task_type: String,
    pub labels: Vec<String>,
    pub priority: u8,
    pub due_date: Option<String>,
    pub estimate_minutes: Option<u32>,
}
impl Action {
    pub(crate) fn without_ids(mut self) -> Self {
        self.task_ids.clear();
        self
    }
    pub fn status(kind: &str, id: i64) -> Self {
        Self {
            kind: kind.into(),
            task_ids: vec![id],
            title: None,
            description: None,
            labels: None,
            task_type: None,
            priority: None,
            due_date: None,
            organization: None,
            estimate: None,
            minutes: None,
            date: None,
            entry_index: None,
            subtasks: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub reply: String,
    pub actions: Vec<Action>,
    pub view: Option<View>,
    pub focus_ids: Vec<i64>,
    #[serde(default)]
    pub checkin: Option<crate::workload::Checkin>,
    #[serde(default)]
    pub capacity: Option<crate::workload::CapacityProfile>,
    #[serde(default)]
    pub feedback: Option<crate::workload::Feedback>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub task_ids: Vec<i64>,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tasks: Vec<Task>,
    pub visible: Vec<Task>,
    pub stats: Stats,
    pub messages: Vec<Message>,
    pub view: View,
    pub focus_ids: Vec<i64>,
    pub configured: bool,
    pub model: String,
    pub demo: bool,
    pub next_task_id: Option<i64>,
    pub show_list: bool,
    pub workload: crate::workload::Workload,
    #[serde(default)]
    pub timer: TimerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub kind: String,
    pub labels: Vec<String>,
    pub priority: u8,
    pub confidence: f64,
    #[serde(default)]
    pub estimate: Option<crate::workload::Estimate>,
}

// Typed Jev primitives; these are decision questions, never chat completions.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JevQuestion {
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    Noul {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JevAnswer {
    Choice { choice: String, confidence: f64 },
    Noul { noul: f64 },
    Score { score: f64 },
}
#[derive(Debug, Deserialize)]
pub struct JevResponse {
    pub answers: BTreeMap<String, JevAnswer>,
}
