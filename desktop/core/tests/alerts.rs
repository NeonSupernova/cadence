use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone};
use jev_tasks_core::alerts::*;
use jev_tasks_core::{Status, Task};
use serde_json::json;
use std::collections::BTreeMap;

fn task(id: i64, due: Option<&str>) -> Task {
    serde_json::from_value(json!({
        "id": id, "title": format!("Task {id}"), "description": "", "raw": "", "type": "chore",
        "labels": [], "priority": 50, "status": "open", "confidence": 1.0,
        "createdAt": "2026-11-01T00:00:00Z", "updatedAt": "2026-11-01T00:00:00Z",
        "dueDate": due, "deleted": false
    }))
    .unwrap()
}
fn tz() -> FixedOffset {
    FixedOffset::west_opt(5 * 3600).unwrap()
}
fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<FixedOffset> {
    tz().with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}
fn reminder(id: &str, when: DateTime<FixedOffset>, repeat: Repeat) -> Reminder {
    Reminder { id: id.into(), task_id: None, title: "Stretch".into(), note: String::new(), at: when.to_rfc3339(), repeat, enabled: true, last_fired_at: None }
}
fn keys(plan: &Plan) -> Vec<String> {
    plan.sent_keys.clone()
}

#[test]
fn date_only_deadlines_use_the_configured_time_and_each_lead_fires_once() {
    let settings = AlertSettings { lead_minutes: vec![60, 0, 1440], ..Default::default() }.validate().unwrap();
    assert_eq!(settings.lead_minutes, vec![1440, 60, 0]);
    let tasks = vec![task(1, Some("2026-11-21"))];
    let mut sent = BTreeMap::new();

    // 08:05 on the due day: the 1-day alert was 23h ago (outside the 12h grace window), so only the 1-hour alert fires.
    let plan1 = plan(&tasks, &[], &settings, &sent, &at(2026, 11, 21, 8, 5));
    assert_eq!(keys(&plan1).len(), 1, "only the 1-hour alert is current: {:?}", keys(&plan1));
    assert!(plan1.deliveries[0].title.starts_with("Due in 1 hour"));
    for k in plan1.sent_keys { sent.insert(k, "2026-11-21T13:05:00Z".into()); }

    assert!(plan(&tasks, &[], &settings, &sent, &at(2026, 11, 21, 8, 30)).deliveries.is_empty());
    let due = plan(&tasks, &[], &settings, &sent, &at(2026, 11, 21, 9, 0));
    assert_eq!(due.deliveries.len(), 1);
    assert!(due.deliveries[0].title.starts_with("Due now"));
}

#[test]
fn completed_low_priority_and_deleted_tasks_are_silent() {
    let settings = AlertSettings { min_priority: 60, ..Default::default() };
    let mut done = task(1, Some("2026-11-21"));
    done.status = Status::Done;
    let mut gone = task(2, Some("2026-11-21"));
    gone.deleted = true;
    let low = task(3, Some("2026-11-21"));
    let mut high = task(4, Some("2026-11-21"));
    high.priority = 80;
    let p = plan(&[done, gone, low, high], &[], &settings, &BTreeMap::new(), &at(2026, 11, 21, 9, 1));
    assert_eq!(p.deliveries.iter().map(|d| d.task_id).collect::<Vec<_>>(), vec![Some(4)]);
}

#[test]
fn overdue_tasks_nudge_once_each_morning_and_skip_stale_leads() {
    let settings = AlertSettings::default();
    let tasks = vec![task(1, Some("2026-11-18"))];
    let p = plan(&tasks, &[], &settings, &BTreeMap::new(), &at(2026, 11, 21, 9, 30));
    assert_eq!(p.deliveries.len(), 1);
    assert_eq!(p.deliveries[0].title, "Overdue · Task 1");
    assert!(p.deliveries[0].body.contains("3 days ago"));
    // Before the morning time there is nothing to say yet.
    assert!(plan(&tasks, &[], &settings, &BTreeMap::new(), &at(2026, 11, 21, 7, 0)).deliveries.is_empty());
    let off = AlertSettings { overdue_daily: false, ..Default::default() };
    assert!(plan(&tasks, &[], &off, &BTreeMap::new(), &at(2026, 11, 21, 9, 30)).deliveries.is_empty());
}

#[test]
fn canvas_exact_due_times_take_precedence() {
    let mut t = task(7, Some("2026-11-21"));
    t.canvas = Some(serde_json::from_value(json!({
        "baseUrl":"https://x","courseId":1,"assignmentId":2,"courseName":"C","courseCode":"C1","title":"A","url":"https://x/a",
        "dueAt":"2026-11-22T04:59:00Z","dueDate":"2026-11-21","submitted":false,"syncedAt":"2026-11-01T00:00:00Z","suggestedPriority":50
    })).unwrap());
    let settings = AlertSettings::default();
    assert_eq!(due_instant(&t, &settings, &tz()).unwrap(), at(2026, 11, 21, 23, 59));
    let p = plan(&[t], &[], &settings, &BTreeMap::new(), &at(2026, 11, 21, 23, 0));
    assert!(p.deliveries[0].title.starts_with("Due in 1 hour"), "{:?}", p.deliveries);
}

#[test]
fn quiet_hours_and_pause_hold_alerts_until_later() {
    let settings = AlertSettings { quiet_hours: true, quiet_start: "22:00".into(), quiet_end: "08:00".into(), ..Default::default() }.validate().unwrap();
    assert!(settings.quiet_at(&at(2026, 11, 21, 23, 0)));
    assert!(settings.quiet_at(&at(2026, 11, 21, 7, 59)));
    assert!(!settings.quiet_at(&at(2026, 11, 21, 8, 0)));
    let r = vec![reminder("r1", at(2026, 11, 21, 23, 0), Repeat::None)];
    assert!(plan(&[], &r, &settings, &BTreeMap::new(), &at(2026, 11, 21, 23, 30)).deliveries.is_empty());
    assert_eq!(plan(&[], &r, &settings, &BTreeMap::new(), &at(2026, 11, 22, 8, 1)).deliveries.len(), 1);

    let paused = AlertSettings { paused_until: Some(at(2026, 11, 21, 12, 0).to_rfc3339()), ..Default::default() };
    assert!(plan(&[], &r, &paused, &BTreeMap::new(), &at(2026, 11, 21, 23, 30)).deliveries.len() == 1);
    assert!(paused.paused_at(at(2026, 11, 21, 11, 0).to_utc()));
    assert!(AlertSettings { quiet_hours: true, quiet_start: "09:00".into(), quiet_end: "09:00".into(), ..Default::default() }.validate().is_err());
}

#[test]
fn repeating_reminders_advance_past_missed_occurrences_and_one_offs_retire() {
    let settings = AlertSettings::default();
    let daily = reminder("d", at(2026, 11, 18, 9, 0), Repeat::Daily);
    let once = reminder("o", at(2026, 11, 21, 9, 0), Repeat::None);
    let p = plan(&[], &[daily, once], &settings, &BTreeMap::new(), &at(2026, 11, 21, 10, 0));
    assert_eq!(p.deliveries.len(), 2);
    let next = p.reminders.unwrap();
    assert_eq!(next[0].at, at(2026, 11, 22, 9, 0).to_rfc3339());
    assert!(next[0].enabled);
    assert!(!next[1].enabled);
    assert!(next[1].last_fired_at.is_some());

    // Friday 09:00 weekday reminder → Monday.
    let weekdays = reminder("w", at(2026, 11, 20, 9, 0), Repeat::Weekdays);
    let p = plan(&[], &[weekdays], &settings, &BTreeMap::new(), &at(2026, 11, 20, 9, 0));
    assert_eq!(p.reminders.unwrap()[0].at, at(2026, 11, 23, 9, 0).to_rfc3339());
}

#[test]
fn reminders_for_finished_tasks_retire_without_notifying() {
    let mut t = task(3, None);
    t.status = Status::Done;
    let mut r = reminder("t", at(2026, 11, 21, 9, 0), Repeat::Daily);
    r.task_id = Some(3);
    let p = plan(&[t], &[r], &AlertSettings::default(), &BTreeMap::new(), &at(2026, 11, 21, 9, 5));
    assert!(p.deliveries.is_empty());
    assert!(!p.reminders.unwrap()[0].enabled);
}

#[test]
fn many_alerts_collapse_into_one_summary_and_disabled_means_silent() {
    let tasks: Vec<_> = (1..=5).map(|i| task(i, Some("2026-11-21"))).collect();
    let p = plan(&tasks, &[], &AlertSettings::default(), &BTreeMap::new(), &at(2026, 11, 21, 9, 0));
    assert_eq!(p.deliveries.len(), 1);
    assert_eq!(p.sent_keys.len(), 5);
    assert!(p.deliveries[0].title.starts_with("5 items"));
    assert!(p.deliveries[0].body.contains("+1 more"));
    let off = AlertSettings { enabled: false, ..Default::default() };
    assert!(plan(&tasks, &[], &off, &BTreeMap::new(), &at(2026, 11, 21, 9, 0)).sent_keys.is_empty());
}

#[test]
fn badge_counts_follow_the_selected_mode() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
    let tasks = vec![task(1, Some("2026-11-20")), task(2, Some("2026-11-21")), task(3, Some("2026-11-22")), task(4, None)];
    assert_eq!(badge_count(&tasks, BadgeMode::Off, today), 0);
    assert_eq!(badge_count(&tasks, BadgeMode::Overdue, today), 1);
    assert_eq!(badge_count(&tasks, BadgeMode::DueToday, today), 2);
    assert_eq!(due_now(&tasks, today).iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn reminder_validation_and_persistence_round_trip() {
    let bad = Reminder { title: "  ".into(), ..reminder("x", at(2026, 11, 21, 9, 0), Repeat::None) };
    assert!(bad.validate().is_err());
    assert!(Reminder { id: "../etc".into(), ..reminder("x", at(2026, 11, 21, 9, 0), Repeat::None) }.validate().is_err());
    let dir = std::env::temp_dir().join(format!("cadence-alerts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let r = vec![reminder("abc-1", at(2026, 11, 21, 9, 0), Repeat::Weekly)];
    save_reminders(&dir, &r).unwrap();
    assert_eq!(load_reminders(&dir), r);
    assert_eq!(load_settings(&dir), AlertSettings::default());
    let mut sent = BTreeMap::from([("old".to_string(), "2026-10-01T00:00:00Z".to_string()), ("new".to_string(), "2026-11-20T00:00:00Z".to_string())]);
    prune_sent(&mut sent, at(2026, 11, 21, 0, 0).to_utc());
    assert_eq!(sent.keys().collect::<Vec<_>>(), vec!["new"]);
    std::fs::remove_dir_all(dir).unwrap();
}
