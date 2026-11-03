use chrono::Utc;
use jev_tasks_core::{canvas::*, *};
use serde_json::json;
use std::path::Path;

fn source(id: i64) -> CanvasSource {
    CanvasSource {
        requirements: String::new(),
        base_url: "https://pcci.instructure.com".into(),
        course_id: 40017,
        assignment_id: id,
        course_name: "Software Engineering".into(),
        course_code: "CS 351".into(),
        title: format!("Assignment {id}"),
        url: format!("https://pcci.instructure.com/courses/40017/assignments/{id}"),
        due_at: Some("2026-11-05T23:59:00-06:00".into()),
        due_date: Some("2026-11-05".into()),
        submitted: false,
        synced_at: Utc::now().to_rfc3339(),
        suggested_priority: 65,
        submitted_at: None,
    }
}
fn import(assignments: Vec<CanvasSource>) -> CanvasImport {
    CanvasImport {
        courses: 1,
        assignments,
        warnings: vec![],
    }
}
fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        reply: String::new(),
        actions,
        view: None,
        focus_ids: vec![],
        ..Default::default()
    }
}
fn store() -> Store {
    Store::open(Path::new(":memory:")).unwrap()
}

#[test]
fn pagination_uses_all_opaque_next_links_and_rejects_other_origins_and_endpoints() {
    let base = normalize_base("https://pcci.instructure.com").unwrap();
    let initial = base.join("api/v1/courses?per_page=100").unwrap();
    let mut calls = 0;
    let results: Vec<i64> = pages(&base, initial.clone(), |url| {
        calls += 1;
        if calls == 1 {
            Ok(Page {
                body: "[1,2]".into(),
                next: Some("https://pcci.instructure.com/api/v1/courses?opaque=next-token".into()),
            })
        } else {
            assert_eq!(url.query(), Some("opaque=next-token"));
            Ok(Page {
                body: "[3]".into(),
                next: None,
            })
        }
    })
    .unwrap();
    assert_eq!(results, vec![1, 2, 3]);
    assert_eq!(calls, 2);
    for bad in [
        "https://evil.example/api/v1/courses?page=2",
        "https://pcci.instructure.com/api/v1/users",
        "http://pcci.instructure.com/api/v1/courses",
    ] {
        let mut calls = 0;
        let result: Result<Vec<i64>, _> = pages(&base, initial.clone(), |_| {
            calls += 1;
            Ok(Page {
                body: "[1]".into(),
                next: Some(bad.into()),
            })
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }
    let repeated: Result<Vec<i64>, _> = pages(&base, initial.clone(), |_| {
        Ok(Page {
            body: "[1]".into(),
            next: Some(initial.to_string()),
        })
    });
    assert!(repeated.is_err());
}

#[test]
fn header_parser_handles_proxy_headers_case_and_actionable_token_errors() {
    let page = parse_http("HTTP/1.1 200 Connection established\r\n\r\nHTTP/2 200\r\nlInK: <https://pcci.instructure.com/api/v1/courses?opaque=a,b>; rel=\"next\", <https://pcci.instructure.com/api/v1/courses?page=1>; rel=\"current\"\r\n\r\n[1]\n200").unwrap();
    assert_eq!(page.body, "[1]");
    assert!(page.next.unwrap().ends_with("opaque=a,b"));
    assert!(parse_http("{\"private\":\"do-not-return\"}\n401")
        .unwrap_err()
        .contains("rejected the token"));
    assert!(!parse_http("private response\n403")
        .unwrap_err()
        .contains("private response"));
}

#[test]
fn imported_assignments_get_canvas_labels_and_stable_references_without_duplicates() {
    let mut s = store();
    let report = s
        .import_canvas(import(vec![source(101), source(102), source(101)]))
        .unwrap();
    assert_eq!((report.created, report.assignments), (2, 2));
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.kind, "assignment");
    assert_eq!(task.status, Status::Open);
    assert!(task.labels.iter().any(|l| l == "canvas"));
    assert!(task.labels.iter().any(|l| l == "course:cs-351"));
    assert_eq!(task.canvas.as_ref().unwrap().assignment_id, 101);
    let second = s
        .import_canvas(import(vec![source(101), source(102)]))
        .unwrap();
    assert_eq!(
        (second.created, second.updated, second.unchanged),
        (0, 0, 2)
    );
    assert_eq!(
        s.tasks().unwrap().iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(s.canvas_report().unwrap().unwrap().assignments, 2);
}

#[test]
fn resync_preserves_local_edits_dates_priority_timers_and_required_canvas_label() {
    let mut s = store();
    s.import_canvas(import(vec![source(101)])).unwrap();
    let mut edit = Action::status("update", 1);
    edit.title = Some("My implementation plan".into());
    edit.due_date = Some("2026-11-04".into());
    edit.priority = Some(91);
    edit.labels = Some(vec!["project".into()]);
    s.apply(
        plan(vec![edit, Action::status("start", 1)]),
        "",
        None,
        false,
    )
    .unwrap();
    let started = s.tasks().unwrap()[0].timer_started_at.clone();
    let mut remote = source(101);
    remote.title = "Renamed assignment".into();
    remote.due_date = Some("2026-11-06".into());
    remote.suggested_priority = 80;
    s.import_canvas(import(vec![remote])).unwrap();
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.title, "My implementation plan");
    assert_eq!(task.due_date.as_deref(), Some("2026-11-04"));
    assert_eq!(task.priority, 91);
    assert_eq!(task.timer_started_at, started);
    assert!(task.labels.iter().any(|l| l == "canvas"));
    assert!(task.labels.iter().any(|l| l == "project"));
    assert_eq!(task.canvas.as_ref().unwrap().title, "Renamed assignment");
}

#[test]
fn source_changes_update_unedited_fields_and_new_submission_stops_the_timer() {
    let mut s = store();
    s.import_canvas(import(vec![source(101)])).unwrap();
    s.apply(plan(vec![Action::status("start", 1)]), "", None, false)
        .unwrap();
    let mut remote = source(101);
    remote.title = "New assignment title".into();
    remote.due_date = Some("2026-11-07".into());
    remote.submitted = true;
    s.import_canvas(import(vec![remote.clone()])).unwrap();
    let task = &s.tasks().unwrap()[0];
    assert_eq!(task.title, remote.title);
    assert_eq!(task.due_date, remote.due_date);
    assert_eq!(task.status, Status::Done);
    assert!(task.timer_started_at.is_none());
    assert_eq!(task.time_entries.len(), 1);
    s.apply(plan(vec![Action::status("reopen", 1)]), "", None, false)
        .unwrap();
    s.import_canvas(import(vec![remote])).unwrap();
    assert_eq!(s.tasks().unwrap()[0].status, Status::Open);
}

#[test]
fn locally_completed_and_trashed_assignments_are_not_reopened_or_recreated() {
    let mut s = store();
    s.import_canvas(import(vec![source(101), source(102)]))
        .unwrap();
    s.apply(
        plan(vec![
            Action::status("complete", 1),
            Action::status("delete", 2),
        ]),
        "",
        None,
        false,
    )
    .unwrap();
    s.import_canvas(import(vec![source(101), source(102)]))
        .unwrap();
    let tasks = s.tasks().unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].status, Status::Done);
    assert!(tasks[1].deleted);
}

#[test]
fn failed_import_rolls_back_tasks_links_and_sync_report() {
    let mut s = store();
    s.import_canvas(import(vec![source(101)])).unwrap();
    let before = serde_json::to_string(&s.canvas_report().unwrap()).unwrap();
    let mut invalid = source(103);
    invalid.title.clear();
    assert!(s.import_canvas(import(vec![source(102), invalid])).is_err());
    assert_eq!(s.tasks().unwrap().len(), 1);
    assert_eq!(
        serde_json::to_string(&s.canvas_report().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        s.import_canvas(import(vec![source(102)])).unwrap().created,
        1
    );
}

#[test]
fn published_is_not_completion_and_unsafe_assignment_links_are_not_kept() {
    let base = normalize_base("https://pcci.instructure.com").unwrap();
    let course: Course = serde_json::from_value(json!({"id":40017,"name":"Software Engineering","course_code":"CS351","workflow_state":"available"})).unwrap();
    let assignment = |submission: serde_json::Value| {
        serde_json::from_value::<Assignment>(json!({"id":101,"name":"Project","published":true,"due_at":"2026-11-05T23:59:00-06:00","html_url":"https://evil.example/steal","submission":submission})).unwrap()
    };
    let open = assignment_source(
        &base,
        &course,
        assignment(json!({"workflow_state":"unsubmitted"})),
    )
    .unwrap()
    .unwrap();
    assert!(!open.submitted);
    assert_eq!(open.due_at.as_deref(), Some("2026-11-05T23:59:00-06:00"));
    assert!(open.url.starts_with("https://pcci.instructure.com/"));
    let submitted = assignment_source(
        &base,
        &course,
        assignment(json!({"workflow_state":"submitted"})),
    )
    .unwrap()
    .unwrap();
    assert!(submitted.submitted);
    let mut unpublished = assignment(serde_json::Value::Null);
    unpublished.published = Some(false);
    assert!(assignment_source(&base, &course, unpublished)
        .unwrap()
        .is_none());
}

#[test]
fn config_rejects_insecure_or_credential_bearing_origins() {
    for bad in [
        "http://pcci.instructure.com",
        "https://user:password@pcci.instructure.com",
        "https://pcci.instructure.com/api/v1",
        "https://pcci.instructure.com?access_token=secret",
        "file:///etc/passwd",
    ] {
        assert!(normalize_base(bad).is_err());
    }
    assert_eq!(
        normalize_base("https://PCCI.instructure.com/")
            .unwrap()
            .host_str(),
        Some("pcci.instructure.com")
    );
    let config = CanvasConfig {
        token: "invalid\nheader".into(),
        ..CanvasConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn old_canvas_submissions_do_not_count_as_work_finished_this_week() {
    let mut s = store();
    let mut old = source(101);
    old.submitted = true;
    old.submitted_at = Some((Utc::now() - chrono::Duration::days(30)).to_rfc3339());
    s.import_canvas(import(vec![old.clone()])).unwrap();
    assert_eq!(stats(&s.tasks().unwrap()).completed, 1);
    assert_eq!(stats(&s.tasks().unwrap()).completed_this_week, 0);
    old.title = "Renamed old assignment".into();
    s.import_canvas(import(vec![old])).unwrap();
    assert_eq!(stats(&s.tasks().unwrap()).completed_this_week, 0);
}

#[test]
fn imports_readable_requirements_and_refreshes_only_unedited_descriptions() {
    let mut s = store();
    let mut remote = source(80);
    s.import_canvas(import(vec![remote.clone()])).unwrap();
    remote.requirements=plain_requirements("<h2>Build a parser</h2><p>Handle &lt;input&gt; &amp; Unicode &#x1F600;.</p><script>steal()</script><ul><li>Submit tests</li></ul>");
    assert_eq!(
        remote.requirements,
        "Build a parser\nHandle <input> & Unicode 😀.\nSubmit tests"
    );
    s.import_canvas(import(vec![remote.clone()])).unwrap();
    let task = s.tasks().unwrap().remove(0);
    assert_eq!(task.description, remote.requirements);
    remote.requirements = "é".repeat(6000);
    s.import_canvas(import(vec![remote.clone()])).unwrap();
    assert_eq!(s.tasks().unwrap()[0].description, remote.requirements);
    let mut edit = Action::status("update", task.id);
    edit.description = Some("My own breakdown".into());
    s.apply(plan(vec![edit]), "", None, false).unwrap();
    remote.requirements = "New requirements again".into();
    s.import_canvas(import(vec![remote])).unwrap();
    assert_eq!(s.tasks().unwrap()[0].description, "My own breakdown");
}

#[test]
fn assignment_api_description_becomes_the_task_requirements() {
    let base = normalize_base("https://pcci.instructure.com").unwrap();
    let course: Course =
        serde_json::from_value(json!({"id":1,"name":"Course","course_code":"CS"})).unwrap();
    let assignment: Assignment = serde_json::from_value(
        json!({"id":2,"name":"Essay","description":"<p>Write <strong>500 words</strong>.</p>"}),
    )
    .unwrap();
    let source = assignment_source(&base, &course, assignment)
        .unwrap()
        .unwrap();
    assert_eq!(source.description(), "Write 500 words.");
    assert!(plain_requirements(&"é".repeat(30_000)).chars().count() <= 20_000);
}
