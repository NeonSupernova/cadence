use jev_tasks_core::{store::validate_chat_version, *};
use std::path::Path;

#[test]
fn delayed_ai_mutations_cannot_overwrite_intervening_changes() {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    store.seed_demo().unwrap();
    let original = store.tasks().unwrap();
    let version = serde_json::to_vec(&original).unwrap();
    let mut action = Action::status("update", original[0].id);
    action.title = Some("New title from AI".into());
    let plan = Plan {
        actions: vec![action],
        ..Default::default()
    };
    assert!(validate_chat_version(&version, &original, &plan).is_ok());
    let mut changed = original.clone();
    changed[0].title = "User's edit while AI was thinking".into();
    assert!(validate_chat_version(&version, &changed, &plan).is_err());
    assert!(validate_chat_version(&version, &changed, &Plan::default()).is_ok());
    let feedback = Plan {
        feedback: Some(jev_tasks_core::workload::Feedback {
            task_id: original[0].id,
            rating: 3,
            actual_minutes: None,
        }),
        ..Default::default()
    };
    assert!(validate_chat_version(&version, &changed, &feedback).is_err());
}

#[test]
fn open_does_not_request_a_schema_write_while_another_connection_is_writing() {
    let path = std::env::temp_dir().join(format!(
        "cadence-concurrency-{}-{}.sqlite",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    {
        let mut initial = Store::open(&path).unwrap();
        initial.seed_demo().unwrap();
        drop(initial);
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer
            .execute_batch("BEGIN IMMEDIATE; UPDATE tasks SET payload=payload;")
            .unwrap();
        // An initialized WAL reader must open and read despite the uncommitted writer.
        let reader = Store::open(&path).unwrap();
        assert!(!reader.tasks().unwrap().is_empty());
        writer.execute_batch("ROLLBACK").unwrap();
    }
    std::fs::remove_file(path).unwrap();
}
