use chrono::Utc;
use jev_tasks_core::ai::{chat_context, conversation};
use jev_tasks_core::*;
use std::path::Path;

fn demo() -> (Vec<Task>, workload::Workload) {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    let tasks = s.tasks().unwrap();
    let w = s.workload(&tasks).unwrap();
    (tasks, w)
}

#[test]
fn board_is_compact_and_hides_trash_details() {
    let (mut tasks, w) = demo();
    tasks[0].description = "x".repeat(2000);
    tasks[1].deleted = true;
    tasks[1].description = "secret details".into();
    let board = chat_context(&tasks, None, &w, Utc::now());
    let list = board["tasks"].as_array().unwrap();
    assert_eq!(list.len(), tasks.len());
    let first = &list[0];
    assert!(first["description"].as_str().unwrap().chars().count() <= 401);
    assert!(first.get("timeEntries").is_none());
    assert!(first.get("raw").is_none());
    let trashed = &list[1];
    assert_eq!(trashed["deleted"], true);
    assert!(trashed.get("description").is_none());
    assert!(board["workload"]["days"].as_array().is_some());
}

#[test]
fn selected_task_gets_full_description_and_indexed_sessions() {
    let (mut tasks, w) = demo();
    let id = tasks[0].id;
    tasks[0].description = "y".repeat(2000);
    tasks[0].time_entries.push(TimeEntry {
        started_at: Utc::now().to_rfc3339(),
        ended_at: Utc::now().to_rfc3339(),
        duration_ms: 25 * 60_000,
        ..serde_json::from_value(serde_json::json!({"startedAt":"","endedAt":"","durationMs":0})).unwrap()
    });
    let board = chat_context(&tasks, Some(id), &w, Utc::now());
    let first = &board["tasks"][0];
    assert_eq!(first["description"].as_str().unwrap().len(), 2000);
    let entries = first["timeEntries"].as_array().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last["index"], entries.len() - 1);
    assert_eq!(last["minutes"], 25.0);
}

fn msg(role: &str, content: &str, ids: Vec<i64>) -> Message {
    serde_json::from_value(serde_json::json!({
        "id": 0, "role": role, "content": content, "taskIds": ids, "createdAt": Utc::now().to_rfc3339(),
    }))
    .unwrap()
}

#[test]
fn conversation_maps_roles_and_keeps_the_latest_turns() {
    let mut messages = vec![msg("user", "old", vec![])];
    for i in 0..25 {
        messages.push(msg("user", &format!("q{i}"), vec![]));
        messages.push(msg("assistant", &format!("a{i}"), vec![7]));
    }
    let turns = conversation(&messages, 20);
    assert_eq!(turns.len(), 20);
    assert_eq!(turns[0]["role"], "user");
    assert_eq!(turns[19]["role"], "assistant");
    assert!(turns[19]["content"].as_str().unwrap().contains("TASK-007"));
    assert!(turns.iter().all(|t| t["content"] != "old"));
}
