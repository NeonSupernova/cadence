use jev_tasks_core::{organization::*, workload::*, *};
use std::path::Path;

fn store() -> Store {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    let pauses = s
        .tasks()
        .unwrap()
        .iter()
        .filter(|t| t.timer_started_at.is_some())
        .map(|t| Action::status("pause", t.id))
        .collect();
    s.apply(
        Plan {
            actions: pauses,
            ..Default::default()
        },
        "",
        None,
        false,
    )
    .unwrap();
    s
}
fn update(id: i64, o: Organization) -> Action {
    let mut a = Action::status("update", id);
    a.organization = Some(o);
    a
}
fn apply(s: &mut Store, actions: Vec<Action>) -> Result<Vec<i64>, String> {
    s.apply(
        Plan {
            actions,
            ..Default::default()
        },
        "",
        None,
        false,
    )
}
#[test]
fn links_persist_and_planning_items_do_not_consume_focus_capacity() {
    let mut s = store();
    let ids: Vec<_> = s.tasks().unwrap().iter().map(|t| t.id).collect();
    let epic = Organization {
        role: "epic".into(),
        ..Default::default()
    };
    let sprint = Organization {
        role: "sprint".into(),
        starts_on: Some("2026-10-26".into()),
        ends_on: Some("2026-11-06".into()),
        ..Default::default()
    };
    let child = Organization {
        parent_id: Some(ids[0]),
        cycle_id: Some(ids[1]),
        external: vec![ExternalReference {
            provider: "GitLab".into(),
            remote_id: "group/project#42".into(),
            url: "https://gitlab.com/group/project/-/issues/42".into(),
        }],
        ..Default::default()
    };
    apply(
        &mut s,
        vec![
            update(ids[0], epic),
            update(ids[1], sprint),
            update(ids[2], child),
        ],
    )
    .unwrap();
    let tasks = s.tasks().unwrap();
    assert_eq!(
        tasks[2].organization.external[0].remote_id,
        "group/project#42"
    );
    let w = analyze(
        &tasks,
        CapacityProfile::default(),
        &[],
        &[],
        &[],
        chrono::Utc::now(),
    );
    assert!(w
        .days
        .iter()
        .flat_map(|d| &d.blocks)
        .all(|b| b.task_id != ids[0] && b.task_id != ids[1]));
    assert!(apply(&mut s, vec![Action::status("start", ids[0])]).is_err());
    apply(&mut s, vec![Action::status("start", ids[2])]).unwrap();
    assert!(apply(
        &mut s,
        vec![update(
            ids[2],
            Organization {
                role: "epic".into(),
                ..Default::default()
            }
        )]
    )
    .is_err());
    assert!(s
        .tasks()
        .unwrap()
        .iter()
        .find(|t| t.id == ids[2])
        .unwrap()
        .timer_started_at
        .is_some());
}
#[test]
fn dangling_links_role_changes_and_epic_cycles_roll_back_the_entire_batch() {
    let mut s = store();
    let ids: Vec<_> = s.tasks().unwrap().iter().map(|t| t.id).collect();
    let epic = Organization {
        role: "epic".into(),
        ..Default::default()
    };
    apply(
        &mut s,
        vec![update(ids[0], epic.clone()), update(ids[1], epic.clone())],
    )
    .unwrap();
    let one = Organization {
        parent_id: Some(ids[1]),
        ..epic.clone()
    };
    let two = Organization {
        parent_id: Some(ids[0]),
        ..epic
    };
    assert!(apply(&mut s, vec![update(ids[0], one), update(ids[1], two)]).is_err());
    assert_eq!(s.tasks().unwrap()[0].organization.parent_id, None);
    let child = Organization {
        parent_id: Some(ids[0]),
        ..Default::default()
    };
    apply(&mut s, vec![update(ids[2], child)]).unwrap();
    assert!(apply(&mut s, vec![update(ids[0], Organization::default())]).is_err());
    assert_eq!(s.tasks().unwrap()[0].organization.role, "epic");
    assert!(apply(
        &mut s,
        vec![update(
            ids[2],
            Organization {
                related_ids: vec![999],
                ..Default::default()
            }
        )]
    )
    .is_err());
}
#[test]
fn external_references_and_planning_dates_are_validated() {
    let mut s = store();
    let id = s.tasks().unwrap()[0].id;
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://name:secret@example.com/issue",
    ] {
        assert!(apply(
            &mut s,
            vec![update(
                id,
                Organization {
                    external: vec![ExternalReference {
                        provider: "Jira".into(),
                        remote_id: "TEAM-1".into(),
                        url: url.into()
                    }],
                    ..Default::default()
                }
            )]
        )
        .is_err());
    }
    assert!(apply(
        &mut s,
        vec![update(
            id,
            Organization {
                role: "iteration".into(),
                starts_on: Some("2026-11-10".into()),
                ends_on: Some("2026-11-01".into()),
                ..Default::default()
            }
        )]
    )
    .is_err());
}
#[test]
fn old_payloads_default_to_a_plain_task() {
    let s = store();
    let task = s.tasks().unwrap().remove(0);
    let mut value = serde_json::to_value(task).unwrap();
    value.as_object_mut().unwrap().remove("organization");
    let task: Task = serde_json::from_value(value).unwrap();
    assert_eq!(task.organization.role, "task");
}
