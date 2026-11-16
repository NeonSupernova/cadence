use jev_tasks_core::sync::*;
use jev_tasks_core::*;
use std::path::Path;

fn store() -> Store {
    Store::open(Path::new(":memory:")).unwrap()
}
fn classification() -> Classification {
    Classification {
        kind: "chore".into(),
        labels: vec!["home".into()],
        priority: 50,
        confidence: 0.9,
        estimate: None,
    }
}
fn create(s: &mut Store, title: &str) -> i64 {
    let mut a = Action::status("create", 0);
    a.task_ids.clear();
    a.title = Some(title.into());
    a.description = Some(format!("Do {title}"));
    s.apply(
        Plan {
            actions: vec![a],
            ..Default::default()
        },
        title,
        Some(&classification()),
        false,
    )
    .unwrap()[0]
}
fn act(s: &mut Store, a: Action) {
    s.apply(
        Plan {
            actions: vec![a],
            ..Default::default()
        },
        "",
        None,
        false,
    )
    .unwrap();
}
fn rename(s: &mut Store, id: i64, title: &str) {
    let mut a = Action::status("update", id);
    a.title = Some(title.into());
    act(s, a);
}
fn sync(from: &Store, to: &mut Store) -> MergeReport {
    to.merge_sync(from.export_sync().unwrap()).unwrap()
}
fn titles(s: &Store) -> Vec<(i64, String)> {
    s.tasks().unwrap().into_iter().map(|t| (t.id, t.title)).collect()
}

#[test]
fn new_tasks_get_uids_and_copy_to_an_empty_device_with_the_same_numbers() {
    let mut a = store();
    create(&mut a, "Laundry");
    create(&mut a, "Groceries");
    assert!(a.tasks().unwrap().iter().all(|t| t.uid.is_some()));
    let mut b = store();
    let report = sync(&a, &mut b);
    assert_eq!(report.created, 2);
    assert_eq!(titles(&b), titles(&a));
    // Merging the same data again changes nothing.
    assert_eq!(sync(&a, &mut b), MergeReport::default());
    assert_eq!(sync(&b, &mut a), MergeReport::default());
}

#[test]
fn the_most_recent_edit_wins_in_both_directions() {
    let mut a = store();
    let id = create(&mut a, "Laundry");
    let mut b = store();
    sync(&a, &mut b);
    rename(&mut a, id, "Old edit on laptop");
    std::thread::sleep(std::time::Duration::from_millis(5));
    rename(&mut b, id, "Newer edit on phone");
    assert_eq!(sync(&a, &mut b).updated, 0);
    assert_eq!(sync(&b, &mut a).updated, 1);
    assert_eq!(a.tasks().unwrap()[0].title, "Newer edit on phone");
    // Timers, steps and completion travel with the task.
    act(&mut a, Action::status("start", id));
    let mut step = Action::status("add_subtasks", id);
    step.subtasks = Some(vec!["Sort colors".into()]);
    act(&mut a, step);
    sync(&a, &mut b);
    let t = &b.tasks().unwrap()[0];
    assert!(t.timer_started_at.is_some());
    assert_eq!(t.subtasks[0].title, "Sort colors");
    act(&mut b, Action::status("complete", id));
    sync(&b, &mut a);
    let t = &a.tasks().unwrap()[0];
    assert_eq!(t.status, Status::Done);
    assert!(t.timer_started_at.is_none());
}

#[test]
fn colliding_numbers_get_new_numbers_and_links_follow_uids() {
    let mut a = store();
    let mut b = store();
    create(&mut b, "Phone task"); // TASK-001 on the phone
    let epic = create(&mut a, "Laptop epic"); // TASK-001 on the laptop too
    let child = create(&mut a, "Laptop child");
    let mut o = organization::Organization {
        role: "epic".into(),
        ..Default::default()
    };
    let mut up = Action::status("update", epic);
    up.organization = Some(o.clone());
    act(&mut a, up);
    o.role = "task".into();
    o.parent_id = Some(epic);
    let mut up = Action::status("update", child);
    up.organization = Some(o);
    act(&mut a, up);

    assert_eq!(sync(&a, &mut b).created, 2);
    let tasks = b.tasks().unwrap();
    let phone = tasks.iter().find(|t| t.title == "Phone task").unwrap();
    let epic_b = tasks.iter().find(|t| t.title == "Laptop epic").unwrap();
    let child_b = tasks.iter().find(|t| t.title == "Laptop child").unwrap();
    assert_eq!(phone.id, 1);
    assert_ne!(epic_b.id, 1);
    assert_eq!(child_b.organization.parent_id, Some(epic_b.id));
    // And the phone's task lands on the laptop with a fresh number.
    assert_eq!(sync(&b, &mut a).created, 1);
    assert_eq!(a.tasks().unwrap().len(), 3);
}

#[test]
fn canvas_assignments_share_one_identity_and_legacy_tasks_get_uids() {
    let source: canvas::CanvasSource = serde_json::from_value(serde_json::json!({
        "baseUrl": "https://example.instructure.com", "courseId": 7, "assignmentId": 42,
        "courseName": "Databases", "courseCode": "CS 303", "title": "ERD", "url": "https://example.instructure.com/courses/7/assignments/42",
        "dueAt": null, "dueDate": null, "submitted": false, "syncedAt": "2026-11-01T00:00:00Z", "suggestedPriority": 60
    }))
    .unwrap();
    assert_eq!(
        canvas_uid(&source),
        "canvas:https://example.instructure.com:7:42"
    );
    // A task saved before sync existed has no uid until the first sync pass.
    let mut a = store();
    let id = create(&mut a, "Legacy");
    let mut legacy = a.tasks().unwrap()[0].clone();
    legacy.uid = None;
    a.replace_task_payload(id, &legacy).unwrap();
    assert_eq!(a.ensure_uids().unwrap(), 1);
    assert!(a.tasks().unwrap()[0].uid.is_some());
    assert_eq!(a.ensure_uids().unwrap(), 0);
}

#[test]
fn invalid_remote_tasks_are_skipped_and_bad_links_are_dropped() {
    let mut a = store();
    let id = create(&mut a, "Fine");
    let mut records = a.export_sync().unwrap();
    let mut bad = records[0].clone();
    bad.uid = "bad".into();
    bad.task.title = String::new();
    let mut odd = records[0].clone();
    odd.uid = "odd".into();
    odd.task.id = 50;
    // Points at a "parent" that is a plain task, which the workspace rejects.
    odd.links.parent = Some(records[0].uid.clone());
    records.push(bad);
    records.push(odd);
    let mut b = store();
    let report = b.merge_sync(records).unwrap();
    assert_eq!(report.created, 2);
    assert_eq!(report.skipped, 1);
    let odd = b.tasks().unwrap().into_iter().find(|t| t.id == 50).unwrap();
    assert_eq!(odd.organization.parent_id, None);
    assert!(b.tasks().unwrap().iter().any(|t| t.id == id));
}

#[test]
fn device_files_round_trip_through_a_folder() {
    let root = std::env::temp_dir().join(format!("cadence-sync-{}", new_uid()));
    std::fs::create_dir_all(&root).unwrap();
    let mut a = store();
    create(&mut a, "Laundry");
    let file = |s: &Store, id: &str| DeviceFile {
        format: FORMAT,
        device_id: id.into(),
        device_name: id.into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        tasks: s.export_sync().unwrap(),
    };
    assert!(write_device_file(&root, &file(&a, "laptop")).unwrap());
    // Unchanged tasks are not rewritten.
    assert!(!write_device_file(&root, &file(&a, "laptop")).unwrap());
    let dir = devices_dir(&root);
    std::fs::write(dir.join(".phone.json.icloud"), b"").unwrap();
    std::fs::write(dir.join("broken.json"), b"{").unwrap();
    let mut future = file(&a, "future");
    future.format = FORMAT + 1;
    std::fs::write(dir.join("future.json"), serde_json::to_vec(&future).unwrap()).unwrap();

    let own = read_device_files(&root, "laptop").unwrap();
    assert!(own.files.is_empty());
    assert_eq!(own.not_downloaded.len(), 1);
    assert_eq!(own.warnings.len(), 2);
    let other = read_device_files(&root, "phone").unwrap();
    assert_eq!(other.files.len(), 1);
    let mut b = store();
    let records = other.files.into_iter().flat_map(|f| f.tasks).collect();
    assert_eq!(b.merge_sync(records).unwrap().created, 1);
    assert!(write_device_file(&root.join("missing"), &file(&a, "x")).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}
