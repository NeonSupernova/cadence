use chrono::{Duration, Local, Utc};
use jev_tasks_core::*;
use std::path::Path;

fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        reply: "Timer updated.".into(),
        actions,
        view: None,
        focus_ids: vec![],
        ..Default::default()
    }
}
fn store() -> Store {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    s.apply(plan(vec![Action::status("pause", 1)]), "", None, false)
        .unwrap();
    s
}
#[test]
fn elapsed_time_and_pausing_are_exact_and_idempotent() {
    let mut task = store().tasks().unwrap()[0].clone();
    let now = Utc::now();
    task.tracked_ms = 10_000;
    task.timer_started_at = Some((now - Duration::seconds(65)).to_rfc3339());
    task.time_entries.clear();
    assert_eq!(task.elapsed_ms_at(now), 75_000);
    task.stop_timer_at(now);
    assert_eq!(task.tracked_ms, 75_000);
    assert_eq!(task.time_entries[0].duration_ms, 65_000);
    assert!(task.timer_started_at.is_none());
    task.stop_timer_at(now + Duration::seconds(30));
    assert_eq!(task.tracked_ms, 75_000);
    assert_eq!(task.time_entries.len(), 1);
}
#[test]
fn starting_another_task_pauses_the_previous_timer_in_one_transaction() {
    let mut s = store();
    s.apply(
        plan(vec![Action::status("start", 1)]),
        "start #1",
        None,
        true,
    )
    .unwrap();
    let started = s.tasks().unwrap()[0].timer_started_at.clone();
    s.apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .unwrap();
    assert_eq!(s.tasks().unwrap()[0].timer_started_at, started);
    s.apply(
        plan(vec![Action::status("start", 2)]),
        "start #2",
        None,
        true,
    )
    .unwrap();
    let tasks = s.tasks().unwrap();
    assert!(tasks[0].timer_started_at.is_none());
    assert!(tasks[1].timer_started_at.is_some());
    assert_eq!(tasks[0].status, Status::InProgress);
    assert_eq!(stats(&tasks).running_task_id, Some(2));
    assert_eq!(
        tasks
            .iter()
            .filter(|t| t.timer_started_at.is_some())
            .count(),
        1
    );
    s.apply(
        plan(vec![Action::status("pause", 2)]),
        "pause #2",
        None,
        true,
    )
    .unwrap();
    assert!(s.tasks().unwrap()[1].timer_started_at.is_none());
    assert_eq!(s.tasks().unwrap()[1].time_entries.len(), 1);
}
#[test]
fn completion_and_trash_save_running_time_and_rollback_preserves_timer() {
    let mut s = store();
    s.apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .unwrap();
    let original = s.tasks().unwrap()[0].timer_started_at.clone();
    assert!(s
        .apply(
            plan(vec![
                Action::status("start", 2),
                Action::status("pause", 999)
            ]),
            "",
            None,
            false
        )
        .is_err());
    assert_eq!(s.tasks().unwrap()[0].timer_started_at, original);
    assert!(s.tasks().unwrap()[1].timer_started_at.is_none());
    s.apply(plan(vec![Action::status("complete", 1)]), "", None, false)
        .unwrap();
    assert!(s.tasks().unwrap()[0].timer_started_at.is_none());
    assert!(s
        .apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .is_err());
    s.apply(
        plan(vec![
            Action::status("start", 2),
            Action::status("delete", 2),
        ]),
        "",
        None,
        false,
    )
    .unwrap();
    let task = &s.tasks().unwrap()[1];
    assert!(task.deleted);
    assert!(task.timer_started_at.is_none());
    assert_eq!(task.time_entries.len(), 1);
}
#[test]
fn older_tasks_load_without_time_fields_and_clock_changes_never_subtract_time() {
    let task = store().tasks().unwrap()[0].clone();
    let mut value = serde_json::to_value(task).unwrap();
    for field in ["trackedMs", "timerStartedAt", "timeEntries"] {
        value.as_object_mut().unwrap().remove(field);
    }
    let mut restored: Task = serde_json::from_value(value).unwrap();
    assert_eq!(restored.tracked_ms, 0);
    assert!(restored.time_entries.is_empty());
    let now = Utc::now();
    restored.tracked_ms = 10_000;
    restored.timer_started_at = Some((now + Duration::hours(1)).to_rfc3339());
    assert_eq!(restored.elapsed_ms_at(now), 10_000);
    restored.stop_timer_at(now);
    assert_eq!(restored.tracked_ms, 10_000);
}
#[test]
fn running_timer_survives_database_reopen() {
    let path = std::env::temp_dir().join(format!(
        "jev-timer-{}-{}.sqlite",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let started;
    {
        let mut s = Store::open(&path).unwrap();
        s.seed_demo().unwrap();
        started = s.tasks().unwrap()[0].timer_started_at.clone();
    }
    {
        let mut s = Store::open(&path).unwrap();
        assert_eq!(s.tasks().unwrap()[0].timer_started_at, started);
        s.apply(plan(vec![Action::status("pause", 1)]), "pause", None, true)
            .unwrap();
        assert!(s.tasks().unwrap()[0].timer_started_at.is_none());
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn time_totals_and_time_sort_include_running_sessions() {
    let mut task = store().tasks().unwrap()[0].clone();
    let now = Utc::now();
    task.tracked_ms = 60_000;
    task.timer_started_at = Some((now - Duration::seconds(30)).to_rfc3339());
    task.time_entries.clear();
    let mut longer = task.clone();
    longer.id = 20;
    longer.tracked_ms = 300_000;
    longer.timer_started_at = None;
    let tasks = vec![task, longer];
    let stats = stats(&tasks);
    assert!((390_000..392_000).contains(&stats.tracked_ms));
    assert_eq!(stats.running_task_id, Some(1));
    assert!(stats.tracked_today_ms <= stats.tracked_ms);
    let view = View {
        sort: "time".into(),
        ..View::default()
    };
    assert_eq!(visible(&tasks, &view)[0].id, 20);
}

#[test]
fn manual_time_can_be_logged_on_a_past_day_corrected_and_removed() {
    let mut s = store();
    let yesterday = Local::now().date_naive() - Duration::days(1);
    let date = yesterday.format("%Y-%m-%d").to_string();
    let before = s.tasks().unwrap()[0].tracked_ms;
    let today_before = stats(&s.tasks().unwrap()).tracked_today_ms;
    let mut add = Action::status("add_time", 1);
    add.minutes = Some(45);
    add.date = Some(date);
    s.apply(plan(vec![add]), "", None, false).unwrap();
    let task = &s.tasks().unwrap()[0];
    let index = task.time_entries.len() - 1;
    assert_eq!(task.tracked_ms, before + 45 * 60_000);
    assert!(task.time_entries[index].manual);
    let tasks = s.tasks().unwrap();
    let model = s.workload(&tasks).unwrap().calibration;
    assert_eq!(
        jev_tasks_core::workload::spent_on(&tasks, yesterday, &model, Utc::now()).0,
        45.0
    );
    assert!(stats(&tasks).tracked_today_ms < 45 * 60_000);

    let mut set = Action::status("set_time", 1);
    set.entry_index = Some(index);
    set.minutes = Some(20);
    s.apply(plan(vec![set]), "", None, false).unwrap();
    assert_eq!(s.tasks().unwrap()[0].tracked_ms, before + 20 * 60_000);

    let mut remove = Action::status("remove_time", 1);
    remove.entry_index = Some(index);
    s.apply(plan(vec![remove]), "", None, false).unwrap();
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.tracked_ms, before);
    assert_eq!(task.time_entries.len(), index);
    let mut today = Action::status("add_time", 1);
    today.minutes = Some(15);
    today.date = Some(Local::now().date_naive().to_string());
    s.apply(plan(vec![today]), "", None, false).unwrap();
    assert_eq!(
        stats(&s.tasks().unwrap()).tracked_today_ms,
        today_before + 15 * 60_000
    );
}

#[test]
fn invalid_time_changes_roll_back_and_do_not_stop_a_running_timer() {
    let mut s = store();
    s.apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .unwrap();
    let started = s.tasks().unwrap()[0].timer_started_at.clone();
    let mut add = Action::status("add_time", 1);
    add.minutes = Some(30);
    add.date = Some(Local::now().date_naive().format("%Y-%m-%d").to_string());
    s.apply(plan(vec![add.clone()]), "", None, false).unwrap();
    let tracked = s.tasks().unwrap()[0].tracked_ms;
    assert_eq!(s.tasks().unwrap()[0].timer_started_at, started);
    let mut bad = Action::status("remove_time", 1);
    bad.entry_index = Some(999);
    assert!(s.apply(plan(vec![add, bad]), "", None, false).is_err());
    assert_eq!(s.tasks().unwrap()[0].tracked_ms, tracked);
    let mut future = Action::status("add_time", 1);
    future.minutes = Some(10);
    future.date = Some((Local::now().date_naive() + Duration::days(1)).to_string());
    assert!(s.apply(plan(vec![future]), "", None, false).is_err());
    assert_eq!(s.tasks().unwrap()[0].timer_started_at, started);
}

#[test]
fn corrections_refresh_completed_task_time_calibration() {
    let mut s = store();
    s.apply(plan(vec![Action::status("complete", 1)]), "", None, false)
        .unwrap();
    let mut add = Action::status("add_time", 1);
    add.minutes = Some(30);
    add.date = Some(Local::now().date_naive().to_string());
    s.apply(plan(vec![add]), "", None, false).unwrap();
    let tasks = s.tasks().unwrap();
    assert_eq!(s.workload(&tasks).unwrap().calibration.time_samples, 1);
    let index = tasks[0].time_entries.len() - 1;
    let mut remove = Action::status("remove_time", 1);
    remove.entry_index = Some(index);
    s.apply(plan(vec![remove]), "", None, false).unwrap();
    assert_eq!(
        s.workload(&s.tasks().unwrap())
            .unwrap()
            .calibration
            .time_samples,
        0
    );
}
