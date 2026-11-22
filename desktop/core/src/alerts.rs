//! Due-date alerts, custom reminders, and the counts shown in the status bar and dock.
//!
//! Everything here is pure scheduling: the desktop shell decides how to deliver a
//! [`Delivery`] (native notification center, toast, or D-Bus) and when to call [`plan`].
use crate::model::{Status, Task};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Alerts older than this are dropped instead of delivered late (e.g. the app was closed).
/// Long enough to cover a typical overnight quiet period.
pub const GRACE_HOURS: i64 = 12;
/// More alerts than this in one pass are combined into a single summary notification.
pub const SUMMARY_THRESHOLD: usize = 3;
const MAX_LEADS: usize = 8;
const MAX_LEAD_MINUTES: u32 = 30 * 24 * 60;
const MAX_REMINDERS: usize = 500;
const SENT_RETENTION_DAYS: i64 = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BadgeMode {
    Off,
    Overdue,
    #[default]
    DueToday,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrayTitle {
    None,
    #[default]
    Timer,
    Due,
    Both,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlertSettings {
    /// Master switch for every notification Cadence sends.
    pub enabled: bool,
    pub due_alerts: bool,
    /// Minutes before the deadline to alert; 0 means "when due".
    pub lead_minutes: Vec<u32>,
    /// Time of day a date-only deadline counts as due ("HH:MM", local).
    pub all_day_time: String,
    /// Repeat an alert every morning (at `all_day_time`) while a task is overdue.
    pub overdue_daily: bool,
    /// Only alert for tasks at or above this Jev priority (0 = every task).
    pub min_priority: u8,
    pub reminders: bool,
    pub sound: bool,
    pub quiet_hours: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    /// Alerts are held (not lost) until this instant.
    pub paused_until: Option<String>,
    pub badge: BadgeMode,
    pub tray_title: TrayTitle,
    /// Show the running timer's progress against its estimate on the Dock/taskbar icon.
    pub dock_progress: bool,
    /// Windows/Linux: closing the window keeps Cadence in the tray so alerts still arrive.
    pub keep_running: bool,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            due_alerts: true,
            lead_minutes: vec![24 * 60, 60, 0],
            all_day_time: "09:00".into(),
            overdue_daily: true,
            min_priority: 0,
            reminders: true,
            sound: true,
            quiet_hours: false,
            quiet_start: "22:00".into(),
            quiet_end: "08:00".into(),
            paused_until: None,
            badge: BadgeMode::DueToday,
            tray_title: TrayTitle::Timer,
            dock_progress: true,
            keep_running: true,
        }
    }
}

fn parse_clock(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value.trim(), "%H:%M").ok()
}

impl AlertSettings {
    /// Normalizes user input and rejects values the scheduler cannot honor.
    pub fn validate(mut self) -> Result<Self, String> {
        for (name, value) in [
            ("due time", &self.all_day_time),
            ("quiet hours start", &self.quiet_start),
            ("quiet hours end", &self.quiet_end),
        ] {
            if parse_clock(value).is_none() {
                return Err(format!("Enter the {name} as HH:MM."));
            }
        }
        if self.quiet_hours && self.quiet_start.trim() == self.quiet_end.trim() {
            return Err("Quiet hours need different start and end times.".into());
        }
        self.lead_minutes.sort_unstable_by(|a, b| b.cmp(a));
        self.lead_minutes.dedup();
        if self.lead_minutes.len() > MAX_LEADS {
            return Err(format!("Choose at most {MAX_LEADS} alert times."));
        }
        if self.lead_minutes.iter().any(|m| *m > MAX_LEAD_MINUTES) {
            return Err("Alerts can start at most 30 days before a deadline.".into());
        }
        self.min_priority = self.min_priority.min(100);
        if let Some(until) = &self.paused_until {
            if DateTime::parse_from_rfc3339(until).is_err() {
                self.paused_until = None;
            }
        }
        self.all_day_time = self.all_day_time.trim().into();
        self.quiet_start = self.quiet_start.trim().into();
        self.quiet_end = self.quiet_end.trim().into();
        Ok(self)
    }

    fn all_day(&self) -> NaiveTime {
        parse_clock(&self.all_day_time).unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
    }

    pub fn paused_at(&self, now: DateTime<Utc>) -> bool {
        self.paused_until
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|until| until.with_timezone(&Utc) > now)
    }

    /// Quiet hours wrap past midnight when start > end (e.g. 22:00–08:00).
    pub fn quiet_at<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> bool {
        let (Some(start), Some(end)) = (parse_clock(&self.quiet_start), parse_clock(&self.quiet_end)) else {
            return false;
        };
        if !self.quiet_hours || start == end {
            return false;
        }
        let t = now.time();
        if start < end {
            t >= start && t < end
        } else {
            t >= start || t < end
        }
    }

    /// Whether delivery is currently held back. Held alerts stay pending.
    pub fn holding<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> bool {
        self.paused_at(now.with_timezone(&Utc)) || self.quiet_at(now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    None,
    Hourly,
    Daily,
    Weekdays,
    Weekly,
}

impl Repeat {
    fn next<Tz: TimeZone>(self, from: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        match self {
            Repeat::None => None,
            Repeat::Hourly => Some(from.clone() + Duration::hours(1)),
            Repeat::Daily => shift_days(from, 1),
            Repeat::Weekly => shift_days(from, 7),
            Repeat::Weekdays => {
                let mut next = shift_days(from, 1)?;
                while matches!(next.weekday(), Weekday::Sat | Weekday::Sun) {
                    next = shift_days(&next, 1)?;
                }
                Some(next)
            }
        }
    }
}

/// Keeps wall-clock time across DST changes ("every day at 9:00" stays at 9:00).
fn shift_days<Tz: TimeZone>(from: &DateTime<Tz>, days: i64) -> Option<DateTime<Tz>> {
    let local = from.naive_local() + Duration::days(days);
    from.timezone()
        .from_local_datetime(&local)
        .earliest()
        .or_else(|| Some(from.clone() + Duration::days(days)))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    #[serde(default)]
    pub task_id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub note: String,
    /// Next time this reminder fires (RFC 3339).
    pub at: String,
    #[serde(default)]
    pub repeat: Repeat,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub last_fired_at: Option<String>,
}
fn yes() -> bool {
    true
}

impl Reminder {
    pub fn validate(mut self) -> Result<Self, String> {
        self.title = self.title.trim().into();
        self.note = self.note.trim().into();
        if self.title.is_empty() || self.title.chars().count() > 200 {
            return Err("Give the reminder a title of 1–200 characters.".into());
        }
        if self.note.chars().count() > 1000 {
            return Err("Keep reminder notes under 1,000 characters.".into());
        }
        let at = DateTime::parse_from_rfc3339(&self.at).map_err(|_| "Choose a valid reminder time.")?;
        self.at = at.to_rfc3339();
        if self.id.trim().is_empty() || self.id.len() > 64 || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("Reminder id is invalid.".into());
        }
        Ok(self)
    }
    pub fn at_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.at).ok().map(|d| d.with_timezone(&Utc))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub key: String,
    pub title: String,
    pub body: String,
    pub task_id: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Plan {
    /// Notifications to show now (already summarized if there were many).
    pub deliveries: Vec<Delivery>,
    /// Keys to record as delivered so they never repeat.
    pub sent_keys: Vec<String>,
    /// Reminders after advancing the ones that fired. `None` when nothing changed.
    pub reminders: Option<Vec<Reminder>>,
}

/// The instant a task is due: Canvas' exact `due_at` when known, otherwise the
/// task's date at the configured all-day time.
pub fn due_instant<Tz: TimeZone>(task: &Task, settings: &AlertSettings, tz: &Tz) -> Option<DateTime<Tz>> {
    if let Some(exact) = task
        .canvas
        .as_ref()
        .and_then(|c| c.due_at.as_deref())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    {
        return Some(exact.with_timezone(tz));
    }
    let date = NaiveDate::parse_from_str(task.due_date.as_deref()?, "%Y-%m-%d").ok()?;
    tz.from_local_datetime(&date.and_time(settings.all_day())).earliest()
}

fn reference(id: i64) -> String {
    format!("TASK-{id:03}")
}

fn lead_phrase(minutes: u32) -> String {
    match minutes {
        0 => "now".into(),
        m if m % (7 * 1440) == 0 => plural(m / (7 * 1440), "week"),
        m if m % 1440 == 0 => plural(m / 1440, "day"),
        m if m % 60 == 0 => plural(m / 60, "hour"),
        m => plural(m, "minute"),
    }
}
fn plural(n: u32, unit: &str) -> String {
    format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
}

fn alertable(task: &Task, settings: &AlertSettings) -> bool {
    !task.deleted && task.status != Status::Done && task.priority >= settings.min_priority
}

fn due_deliveries<Tz: TimeZone>(tasks: &[Task], settings: &AlertSettings, now: &DateTime<Tz>) -> Vec<Delivery>
where
    Tz::Offset: std::fmt::Display,
{
    let grace = Duration::hours(GRACE_HOURS);
    let tz = now.timezone();
    let mut out = Vec::new();
    for task in tasks.iter().filter(|t| alertable(t, settings)) {
        let Some(due) = due_instant(task, settings, &tz) else { continue };
        let stamp = due.with_timezone(&Utc).format("%Y%m%dT%H%M");
        for &lead in &settings.lead_minutes {
            let fire_at = due.clone() - Duration::minutes(lead as i64);
            // A "1 day before" alert is noise once the deadline itself has passed.
            if fire_at > *now || now.clone() - fire_at > grace || (lead > 0 && *now >= due) {
                continue;
            }
            out.push(Delivery {
                key: format!("due:{}:{stamp}:{lead}", task.id),
                title: if lead == 0 { format!("Due now · {}", task.title) } else { format!("Due in {} · {}", lead_phrase(lead), task.title) },
                body: format!("{} is due {}.", reference(task.id), due.format("%a %b %-d, %-I:%M %p")),
                task_id: Some(task.id),
            });
        }
        if settings.overdue_daily && now.date_naive() > due.date_naive() {
            let Some(fire_at) = tz.from_local_datetime(&now.date_naive().and_time(settings.all_day())).earliest() else { continue };
            if fire_at <= *now && now.clone() - fire_at <= grace {
                let days = (now.date_naive() - due.date_naive()).num_days().max(1) as u32;
                out.push(Delivery {
                    key: format!("overdue:{}:{stamp}:{}", task.id, now.date_naive()),
                    title: format!("Overdue · {}", task.title),
                    body: format!("{} was due {} ago.", reference(task.id), plural(days, "day")),
                    task_id: Some(task.id),
                });
            }
        }
    }
    out
}

/// Decides what to deliver right now. `sent` holds keys already delivered.
pub fn plan<Tz: TimeZone>(
    tasks: &[Task],
    reminders: &[Reminder],
    settings: &AlertSettings,
    sent: &BTreeMap<String, String>,
    now: &DateTime<Tz>,
) -> Plan
where
    Tz::Offset: std::fmt::Display,
{
    let mut plan = Plan::default();
    if !settings.enabled || settings.holding(now) {
        return plan;
    }
    let mut pending: Vec<Delivery> = Vec::new();
    if settings.due_alerts {
        pending.extend(due_deliveries(tasks, settings, now).into_iter().filter(|d| !sent.contains_key(&d.key)));
    }
    if settings.reminders {
        let mut changed = false;
        let mut next = reminders.to_vec();
        let tz = now.timezone();
        for reminder in next.iter_mut().filter(|r| r.enabled) {
            let Some(at) = reminder.at_utc() else { continue };
            if at > now.with_timezone(&Utc) {
                continue;
            }
            let task = reminder.task_id.and_then(|id| tasks.iter().find(|t| t.id == id));
            // Reminders for finished or deleted tasks retire quietly.
            if task.is_some_and(|t| t.deleted || t.status == Status::Done) {
                reminder.enabled = false;
                changed = true;
                continue;
            }
            let body = match (reminder.note.is_empty(), task) {
                (false, _) => reminder.note.clone(),
                (true, Some(t)) => format!("{} · {}", reference(t.id), t.title),
                (true, None) => "Reminder from Cadence".into(),
            };
            pending.push(Delivery {
                key: format!("reminder:{}:{}", reminder.id, at.format("%Y%m%dT%H%M")),
                title: reminder.title.clone(),
                body,
                task_id: reminder.task_id,
            });
            reminder.last_fired_at = Some(now.with_timezone(&Utc).to_rfc3339());
            // Skip occurrences missed while the app was closed instead of replaying them.
            let mut cursor = at.with_timezone(&tz);
            loop {
                match reminder.repeat.next(&cursor) {
                    Some(n) if n <= *now => cursor = n,
                    Some(n) => {
                        reminder.at = n.to_rfc3339();
                        break;
                    }
                    None => {
                        reminder.enabled = false;
                        break;
                    }
                }
            }
            changed = true;
        }
        if changed {
            plan.reminders = Some(next);
        }
    }
    plan.sent_keys = pending.iter().map(|d| d.key.clone()).collect();
    plan.deliveries = if pending.len() > SUMMARY_THRESHOLD {
        let titles: Vec<_> = pending.iter().take(4).map(|d| d.title.clone()).collect();
        let more = pending.len().saturating_sub(4);
        vec![Delivery {
            key: "summary".into(),
            title: format!("{} items need your attention", pending.len()),
            body: format!("{}{}", titles.join("\n"), if more > 0 { format!("\n+{more} more") } else { String::new() }),
            task_id: None,
        }]
    } else {
        pending
    };
    plan
}

/// Count shown on the Dock / taskbar / launcher icon.
pub fn badge_count(tasks: &[Task], mode: BadgeMode, today: NaiveDate) -> i64 {
    let today = today.format("%Y-%m-%d").to_string();
    tasks
        .iter()
        .filter(|t| !t.deleted && t.status != Status::Done)
        .filter(|t| match (mode, t.due_date.as_deref()) {
            (BadgeMode::Off, _) | (_, None) => false,
            (BadgeMode::Overdue, Some(d)) => d < today.as_str(),
            (BadgeMode::DueToday, Some(d)) => d <= today.as_str(),
        })
        .count() as i64
}

/// Open tasks due today or earlier, most overdue first.
pub fn due_now(tasks: &[Task], today: NaiveDate) -> Vec<&Task> {
    let today = today.format("%Y-%m-%d").to_string();
    let mut due: Vec<_> = tasks
        .iter()
        .filter(|t| !t.deleted && t.status != Status::Done && t.due_date.as_deref().is_some_and(|d| d <= today.as_str()))
        .collect();
    due.sort_by(|a, b| a.due_date.cmp(&b.due_date).then(b.priority.cmp(&a.priority)));
    due
}

pub fn next_reminder(reminders: &[Reminder]) -> Option<&Reminder> {
    reminders.iter().filter(|r| r.enabled).filter_map(|r| r.at_utc().map(|at| (at, r))).min_by_key(|(at, _)| *at).map(|(_, r)| r)
}

/// Drops delivery records old enough that their alerts can no longer recur.
pub fn prune_sent(sent: &mut BTreeMap<String, String>, now: DateTime<Utc>) {
    let cutoff = now - Duration::days(SENT_RETENTION_DAYS);
    sent.retain(|_, at| DateTime::parse_from_rfc3339(at).map(|t| t.with_timezone(&Utc) > cutoff).unwrap_or(false));
}

// ----- persistence -------------------------------------------------------

fn read<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn load_settings(dir: &Path) -> AlertSettings {
    read(&dir.join("alerts.json"))
}
pub fn save_settings(dir: &Path, settings: &AlertSettings) -> Result<(), String> {
    write(&dir.join("alerts.json"), settings)
}
pub fn load_reminders(dir: &Path) -> Vec<Reminder> {
    read(&dir.join("reminders.json"))
}
pub fn save_reminders(dir: &Path, reminders: &[Reminder]) -> Result<(), String> {
    if reminders.len() > MAX_REMINDERS {
        return Err(format!("Keep at most {MAX_REMINDERS} reminders."));
    }
    write(&dir.join("reminders.json"), &reminders)
}
pub fn load_sent(dir: &Path) -> BTreeMap<String, String> {
    read(&dir.join("alerts-sent.json"))
}
pub fn save_sent(dir: &Path, sent: &BTreeMap<String, String>) -> Result<(), String> {
    write(&dir.join("alerts-sent.json"), sent)
}
