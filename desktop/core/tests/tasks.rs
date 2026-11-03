use jev_tasks_core::ai::{jev_questions, parse_classification};
use jev_tasks_core::*;
use serde_json::json;
use std::path::Path;

fn store() -> Store {
    Store::open(Path::new(":memory:")).unwrap()
}
fn classification() -> Classification {
    Classification {
        kind: "feature".into(),
        labels: vec!["coding".into()],
        priority: 60,
        confidence: 0.93,
        estimate: None,
    }
}
fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        reply: "Done.".into(),
        actions,
        view: None,
        focus_ids: vec![],
        ..Default::default()
    }
}
fn create(title: &str) -> Action {
    Action {
        kind: "create".into(),
        task_ids: vec![],
        title: Some(title.into()),
        description: Some("A useful description".into()),
        labels: None,
        task_type: None,
        priority: None,
        due_date: None,
        estimate: None,
        organization: None,
    }
}
fn add(s: &mut Store, title: &str) -> i64 {
    s.apply(
        plan(vec![create(title)]),
        title,
        Some(&classification()),
        false,
    )
    .unwrap()[0]
}

#[test]
fn tasks_have_stable_ids_and_chat_references() {
    let mut s = store();
    let ids = s
        .apply(
            plan(vec![create("Build task chat")]),
            "I need a task chat",
            Some(&classification()),
            true,
        )
        .unwrap();
    assert_eq!(ids, vec![1]);
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.raw, "I need a task chat");
    assert_eq!(task.kind, "feature");
    assert_eq!(task.labels, vec!["coding"]);
    let messages = s.messages().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].task_ids, vec![1]);
    s.apply(
        plan(vec![Action::status("complete", 1)]),
        "complete #1",
        None,
        true,
    )
    .unwrap();
    assert_eq!(s.tasks().unwrap()[0].status, Status::Done);
    assert_eq!(s.messages().unwrap()[3].task_ids, vec![1]);
}

#[test]
fn all_changes_rollback_when_a_reference_is_invalid() {
    let mut s = store();
    add(&mut s, "Existing task");
    assert!(s
        .apply(
            plan(vec![
                Action::status("complete", 1),
                Action::status("complete", 999)
            ]),
            "complete",
            None,
            true
        )
        .is_err());
    assert_eq!(s.tasks().unwrap()[0].status, Status::Open);
    assert!(s.messages().unwrap().is_empty());
    assert!(s
        .apply(
            plan(vec![create("New task"), Action::status("complete", 999)]),
            "",
            Some(&classification()),
            true
        )
        .is_err());
    assert_eq!(s.tasks().unwrap().len(), 1);
    assert!(s
        .apply(
            plan(vec![Action::status("complete", 0)]),
            "",
            Some(&classification()),
            true
        )
        .is_err());
}

#[test]
fn edits_preserve_unmentioned_fields_and_normalize_labels() {
    let mut s = store();
    add(&mut s, "Original title");
    let mut action = Action::status("update", 1);
    action.labels = Some(vec!["#Launch".into(), "coding".into(), "launch".into()]);
    action.priority = Some(90);
    action.due_date = Some("2026-10-30".into());
    s.apply(plan(vec![action]), "label and schedule", None, false)
        .unwrap();
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.title, "Original title");
    assert_eq!(task.description, "A useful description");
    assert_eq!(task.labels, vec!["coding", "launch"]);
    assert_eq!(task.priority, 90);
    assert_eq!(task.due_date.as_deref(), Some("2026-10-30"));
    let mut clear = Action::status("update", 1);
    clear.due_date = Some(String::new());
    s.apply(plan(vec![clear]), "clear deadline", None, false)
        .unwrap();
    assert!(s.tasks().unwrap()[0].due_date.is_none());
}

#[test]
fn delete_is_reversible_and_preserves_references() {
    let mut s = store();
    let id = add(&mut s, "Keep this task");
    s.apply(
        plan(vec![Action::status("delete", id)]),
        "delete",
        None,
        false,
    )
    .unwrap();
    let tasks = s.tasks().unwrap();
    assert_eq!(stats(&tasks).total, 0);
    assert!(visible(&tasks, &View::default()).is_empty());
    let trash = View {
        status: "deleted".into(),
        ..View::default()
    };
    assert_eq!(visible(&tasks, &trash)[0].id, id);
    assert!(s
        .apply(plan(vec![Action::status("complete", id)]), "", None, false)
        .is_err());
    s.apply(
        plan(vec![Action::status("restore", id)]),
        "restore",
        None,
        false,
    )
    .unwrap();
    assert_eq!(visible(&s.tasks().unwrap(), &View::default())[0].id, id);
}

#[test]
fn invalid_dates_types_and_priority_are_rejected_without_writes() {
    let mut s = store();
    add(&mut s, "Task");
    for (date, priority, kind) in [
        (Some("2026-02-30"), None, None),
        (None, Some(101), None),
        (None, None, Some("mystery")),
    ] {
        let mut action = Action::status("update", 1);
        action.due_date = date.map(str::to_string);
        action.priority = priority;
        action.task_type = kind.map(str::to_string);
        assert!(s.apply(plan(vec![action]), "", None, false).is_err());
    }
    assert_eq!(s.tasks().unwrap()[0].priority, 60);
}

#[test]
fn sorting_filtering_and_statistics_come_from_real_tasks() {
    let mut s = store();
    s.seed_demo().unwrap();
    let tasks = s.tasks().unwrap();
    let stat = stats(&tasks);
    assert_eq!(
        (
            stat.total,
            stat.active,
            stat.completed,
            stat.high_priority,
            stat.due_today
        ),
        (6, 5, 1, 2, 2)
    );
    assert_eq!(stat.completion_rate, 16);
    let mut view = View {
        label: "coding".into(),
        ..View::default()
    };
    assert_eq!(
        visible(&tasks, &view)
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 4]
    );
    view.sort = "newest".into();
    assert_eq!(visible(&tasks, &view)[0].id, 4);
    view = View {
        status: "today".into(),
        ..View::default()
    };
    assert_eq!(visible(&tasks, &view).len(), 2);
    view = View {
        query: "TASK-003".into(),
        ..View::default()
    };
    assert_eq!(visible(&tasks, &view)[0].id, 3);
    view = View {
        task_type: "research".into(),
        ..View::default()
    };
    assert_eq!(visible(&tasks, &view)[0].id, 4);
}

#[test]
fn next_suggestion_excludes_blocked_finished_and_running_tasks() {
    let mut s = store();
    s.seed_demo().unwrap();
    let mut tasks = s.tasks().unwrap();
    // Task 1 is running and task 6 is completed in the demo.
    assert_eq!(next_task(&tasks).unwrap().id, 2);
    tasks[1].labels.push("blocked".into());
    tasks[2].priority = 80;
    assert_eq!(next_task(&tasks).unwrap().id, 3);
    tasks[2].deleted = true;
    assert_eq!(next_task(&tasks).unwrap().id, 4);
    for task in &mut tasks {
        task.status = Status::Done;
    }
    assert!(next_task(&tasks).is_none());
}

#[test]
fn jev_decisions_are_typed_and_probabilities_validated() {
    let questions = jev_questions(&[]);
    assert_eq!(questions.len(), LABELS.len() + 8);
    let mut answers = serde_json::Map::new();
    answers.insert(
        "task_type".into(),
        json!({"type":"choice", "choice":"bug", "confidence":0.96}),
    );
    answers.insert(
        "intent".into(),
        json!({"type":"choice", "choice":"create", "confidence":0.96}),
    );
    answers.insert("priority".into(), json!({"type":"score", "score":3.8}));
    for (label, _) in LABELS {
        answers.insert(
            format!("label_{label}"),
            json!({"type":"noul", "noul":if *label == "coding" {0.9} else {0.1}}),
        );
    }
    let (c, intent) = parse_classification(json!({"answers":answers}), 0.65).unwrap();
    assert_eq!(
        (c.kind.as_str(), c.priority, intent.as_str()),
        ("bug", 95, "create")
    );
    assert_eq!(c.labels, vec!["coding"]);
    answers.insert("label_coding".into(), json!({"type":"noul", "noul":1.5}));
    assert!(parse_classification(json!({"answers":answers}), 0.65).is_err());
}

#[test]
fn sqlite_reopens_without_losing_task_or_chat_history() {
    let path = std::env::temp_dir().join(format!(
        "jev-persist-{}-{}.sqlite",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    {
        let mut s = Store::open(&path).unwrap();
        s.apply(
            plan(vec![create("Persistent task")]),
            "remember this",
            Some(&classification()),
            true,
        )
        .unwrap();
    }
    {
        let s = Store::open(&path).unwrap();
        assert_eq!(s.tasks().unwrap()[0].title, "Persistent task");
        assert_eq!(s.messages().unwrap().len(), 2);
    }
    let _ = std::fs::remove_file(path);
}
