use chrono::{Duration, Utc};
use jev_tasks_core::ai::parse_breakdown;
use jev_tasks_core::*;
use std::path::Path;

fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        actions,
        ..Default::default()
    }
}
fn store() -> Store {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    let running: Vec<_> = s
        .tasks()
        .unwrap()
        .iter()
        .filter(|t| t.timer_started_at.is_some())
        .map(|t| Action::status("pause", t.id))
        .collect();
    if !running.is_empty() {
        s.apply(plan(running), "", None, false).unwrap();
    }
    s
}
fn running(s: &Store) -> Vec<i64> {
    s.tasks()
        .unwrap()
        .iter()
        .filter(|t| t.timer_started_at.is_some())
        .map(|t| t.id)
        .collect()
}
fn step(kind: &str, id: i64, index: Option<usize>, titles: Option<Vec<&str>>) -> Action {
    let mut a = Action::status(kind, id);
    a.entry_index = index;
    a.subtasks = titles.map(|t| t.into_iter().map(String::from).collect());
    a
}

#[test]
fn several_timers_run_at_once_by_default() {
    let mut s = store();
    assert!(s.timer_settings().unwrap().multiple);
    s.apply(
        plan(vec![Action::status("start", 1), Action::status("start", 2)]),
        "",
        None,
        false,
    )
    .unwrap();
    assert_eq!(running(&s), vec![1, 2]);
    assert_eq!(stats(&s.tasks().unwrap()).running_task_ids, vec![1, 2]);
    s.apply(plan(vec![Action::status("start", 3)]), "", None, false)
        .unwrap();
    assert_eq!(running(&s), vec![1, 2, 3]);
}

#[test]
fn switching_to_one_timer_keeps_only_the_newest() {
    let mut s = store();
    s.apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    s.apply(plan(vec![Action::status("start", 2)]), "", None, false)
        .unwrap();
    s.save_timer_settings(&TimerSettings {
        multiple: false,
        cap_minutes: 240,
    })
    .unwrap();
    assert_eq!(running(&s), vec![2]);
}

#[test]
fn stopping_past_the_cutoff_records_only_the_cutoff() {
    let mut task = store().tasks().unwrap()[0].clone();
    let now = Utc::now();
    task.tracked_ms = 0;
    task.time_entries.clear();
    task.timer_started_at = Some((now - Duration::hours(9)).to_rfc3339());
    assert!(task.timer_over_cap(now, Some(4 * 3_600_000)));
    task.stop_timer_capped(now, Some(4 * 3_600_000));
    assert_eq!(task.tracked_ms, 4 * 3_600_000);
    assert!(task.time_entries[0].auto_stopped);
    // Under the cap nothing is clipped.
    task.timer_started_at = Some((now - Duration::minutes(30)).to_rfc3339());
    task.stop_timer_capped(now, Some(4 * 3_600_000));
    assert_eq!(task.time_entries[1].duration_ms, 30 * 60_000);
    assert!(!task.time_entries[1].auto_stopped);
}

#[test]
fn enforcing_the_cutoff_stops_forgotten_timers_only() {
    let mut s = store();
    s.apply(
        plan(vec![Action::status("start", 1), Action::status("start", 2)]),
        "",
        None,
        false,
    )
    .unwrap();
    // Backdate task 1 as if its timer was forgotten overnight.
    let mut forgotten = s.tasks().unwrap()[0].clone();
    let tracked_before = forgotten.tracked_ms;
    forgotten.timer_started_at = Some((Utc::now() - Duration::hours(12)).to_rfc3339());
    s.replace_task_payload(1, &forgotten).unwrap();
    assert_eq!(s.enforce_timer_cap().unwrap(), vec![1]);
    assert_eq!(running(&s), vec![2]);
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.tracked_ms, tracked_before + 4 * 3_600_000);
    assert!(task.time_entries.last().unwrap().auto_stopped);
    // Pausing after the cutoff also clips the session.
    let mut late = s.tasks().unwrap()[1].clone();
    late.timer_started_at = Some((Utc::now() - Duration::hours(6)).to_rfc3339());
    let before = late.tracked_ms;
    s.replace_task_payload(2, &late).unwrap();
    s.apply(plan(vec![Action::status("pause", 2)]), "", None, false)
        .unwrap();
    assert_eq!(s.tasks().unwrap()[1].tracked_ms, before + 4 * 3_600_000);
}

#[test]
fn timer_cutoff_must_be_reasonable() {
    let mut s = store();
    for bad in [1, 14, 1441] {
        assert!(s
            .save_timer_settings(&TimerSettings {
                multiple: true,
                cap_minutes: bad,
            })
            .is_err());
    }
    s.save_timer_settings(&TimerSettings {
        multiple: true,
        cap_minutes: 0,
    })
    .unwrap();
    assert_eq!(s.timer_settings().unwrap().cap_ms(), None);
}

#[test]
fn steps_can_be_added_checked_and_removed() {
    let mut s = store();
    s.apply(
        plan(vec![step(
            "add_subtasks",
            1,
            None,
            Some(vec!["Open the doc", "  Write the outline  "]),
        )]),
        "",
        None,
        false,
    )
    .unwrap();
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.subtasks.len(), 2);
    assert_eq!(task.subtasks[1].title, "Write the outline");
    s.apply(plan(vec![step("toggle_subtask", 1, Some(0), None)]), "", None, false)
        .unwrap();
    let task = &s.tasks().unwrap()[0];
    assert!(task.subtasks[0].done && task.subtasks[0].done_at.is_some());
    s.apply(plan(vec![step("toggle_subtask", 1, Some(0), None)]), "", None, false)
        .unwrap();
    assert!(!s.tasks().unwrap()[0].subtasks[0].done);
    s.apply(plan(vec![step("remove_subtask", 1, Some(0), None)]), "", None, false)
        .unwrap();
    assert_eq!(s.tasks().unwrap()[0].subtasks[0].title, "Write the outline");
    assert!(s
        .apply(plan(vec![step("toggle_subtask", 1, Some(5), None)]), "", None, false)
        .is_err());
    assert!(s
        .apply(plan(vec![step("add_subtasks", 1, None, Some(vec![" "]))]), "", None, false)
        .is_err());
}

#[test]
fn older_tasks_load_without_steps_and_breakdown_parsing_is_bounded() {
    let task: Task = serde_json::from_str(r#"{"id":1,"title":"T","description":"D","raw":"","type":"chore","labels":[],"priority":1,"status":"open","confidence":1,"createdAt":"","updatedAt":"","dueDate":null,"deleted":false}"#).unwrap();
    assert!(task.subtasks.is_empty());
    let steps = parse_breakdown(r#"{"steps":["a"," b ","","c","d","e","f","g","h","i","j"]}"#, 0).unwrap();
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[1], "b");
    assert_eq!(parse_breakdown(r#"{"steps":["a","b"]}"#, 29).unwrap().len(), 1);
    assert!(parse_breakdown(r#"{"steps":["a"]}"#, 30).is_err());
    assert!(parse_breakdown("nope", 0).is_err());
}
