use jev_tasks_core::canvas::{normalize_base, parse_http, CanvasClient, CanvasConfig};
use jev_tasks_core::canvas_workspace::*;
use serde_json::json;

fn assignment() -> serde_json::Value {
    json!({"id":101,"published":true,"submission_types":["online_text_entry","online_url","online_upload"],"allowed_attempts":3,"submission":{"attempt":1},"allowed_extensions":["pdf","txt"]})
}
fn file(name: &str) -> UploadFile {
    UploadFile {
        name: name.into(),
        content_type: "application/pdf".into(),
        bytes: vec![0, 255, 10, 0, 13],
    }
}

#[test]
fn named_queries_limit_access_to_student_resources_and_encode_untrusted_searches() {
    for resource in [
        "courses",
        "calendar",
        "todo",
        "groups",
        "inbox",
        "conversation",
        "course",
        "tabs",
        "assignments",
        "grades",
        "assignment",
        "submission",
        "modules",
        "moduleItems",
        "pages",
        "page",
        "files",
        "announcements",
        "discussions",
        "discussion",
        "discussionView",
        "discussionReplies",
        "quizzes",
        "recipients",
    ] {
        let query = WorkspaceQuery {
            resource: resource.into(),
            course_id: Some(42),
            id: Some(7),
            parent_id: Some(8),
            key: Some("My page & search/?next=https://evil.example".into()),
        };
        let endpoint = query.endpoint().unwrap();
        assert!(endpoint.path.starts_with("api/v1/"));
        let base = normalize_base("https://school.instructure.com").unwrap();
        let url = base.join(&endpoint.path).unwrap();
        assert_eq!(url.origin(), base.origin());
        if resource == "submission" {
            assert!(url.path().ends_with("/submissions/self"));
        }
        if resource == "recipients" {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            assert_eq!(
                params.get("search").unwrap(),
                "My page & search/?next=https://evil.example"
            );
            assert_eq!(params.get("type").unwrap(), "user");
        }
    }
    for resource in [
        "users",
        "roster",
        "accounts",
        "https://evil.example",
        "../users/self",
    ] {
        assert!(WorkspaceQuery {
            resource: resource.into(),
            ..Default::default()
        }
        .endpoint()
        .is_err());
    }
    for id in [None, Some(-1), Some(0)] {
        assert!(WorkspaceQuery {
            resource: "assignment".into(),
            course_id: id,
            id: Some(1),
            ..Default::default()
        }
        .endpoint()
        .is_err());
    }
    for key in ["", ".", "..", "\0"] {
        assert!(WorkspaceQuery {
            resource: "page".into(),
            course_id: Some(1),
            key: Some(key.into()),
            ..Default::default()
        }
        .endpoint()
        .is_err());
    }
}

#[test]
fn submissions_enforce_modes_windows_and_attempts_but_allow_individual_extra_attempts() {
    let mut item = assignment();
    assert!(validate_submission(&item, "online_text_entry", "My work", "", &[]).is_ok());
    assert!(validate_submission(&item, "online_url", "", "https://example.com/work", &[]).is_ok());
    assert!(validate_submission(&item, "online_upload", "", "", &[file("Work.PDF")]).is_ok());
    for mode in [
        "online_quiz",
        "external_tool",
        "media_recording",
        "student_annotation",
    ] {
        assert!(validate_submission(&item, mode, "My work", "https://example.com", &[]).is_err());
    }
    item["locked_for_user"] = json!(true);
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_err());
    item["locked_for_user"] = json!(false);
    item["unlock_at"] = json!("2999-01-01T00:00:00Z");
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_err());
    item["unlock_at"] = serde_json::Value::Null;
    item["lock_at"] = json!("2000-01-01T00:00:00Z");
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_err());
    item["lock_at"] = serde_json::Value::Null;
    item["submission"]["attempt"] = json!(3);
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_err());
    item["submission"]["extra_attempts"] = json!(1);
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_ok());
    item["allowed_attempts"] = json!(-1);
    item["submission"]["attempt"] = json!(100);
    assert!(validate_submission(&item, "online_text_entry", "Work", "", &[]).is_ok());
}

#[test]
fn invalid_text_urls_and_files_are_rejected_before_uploading() {
    let item = assignment();
    assert!(validate_submission(&item, "online_text_entry", " \n ", "", &[]).is_err());
    for link in [
        "javascript:alert(1)",
        "file:///secret",
        "https://user:password@example.com",
        "example.com",
    ] {
        assert!(validate_submission(&item, "online_url", "", link, &[]).is_err());
    }
    assert!(validate_submission(&item, "online_upload", "", "", &[]).is_err());
    for name in [
        "work.exe",
        "bad\r\nHeader.pdf",
        "../secret.pdf",
        "bad\0.pdf",
    ] {
        assert!(validate_submission(&item, "online_upload", "", "", &[file(name)]).is_err());
    }
    let mut bad = file("work.pdf");
    bad.content_type = "application/pdf\r\nInjected: yes".into();
    assert!(validate_submission(&item, "online_upload", "", "", &[bad]).is_err());
    let too_many: Vec<_> = (0..21).map(|_| file("work.pdf")).collect();
    assert!(validate_submission(&item, "online_upload", "", "", &too_many).is_err());
    let mut huge = file("work.pdf");
    huge.bytes = vec![0; 50 * 1024 * 1024 + 1];
    assert!(validate_submission(&item, "online_upload", "", "", &[huge]).is_err());
}

#[test]
fn upload_destinations_are_https_and_confirmation_stays_on_canvas_files_api() {
    let base = normalize_base("https://school.instructure.com").unwrap();
    assert!(upload_url("https://uploads.s3.amazonaws.com/signed?signature=opaque").is_ok());
    for value in [
        "http://uploads.example.com/",
        "file:///etc/passwd",
        "https://user:pass@uploads.example.com/",
        "https://uploads.example.com/#fragment",
    ] {
        assert!(upload_url(value).is_err());
    }
    for path in [
        "/api/v1/files/3",
        "/api/v1/files/3/create_success?uuid=opaque",
    ] {
        assert!(validate_confirmation(&base, &base.join(path).unwrap()).is_ok());
    }
    for value in [
        "https://evil.example/api/v1/files/3/create_success",
        "http://school.instructure.com/api/v1/files/3",
        "https://school.instructure.com/api/v1/courses/3",
        "https://school.instructure.com/api/v1/files/0",
        "https://school.instructure.com/api/v1/files/3/anything",
        "https://school.instructure.com/",
    ] {
        assert!(validate_confirmation(&base, &url::Url::parse(value).unwrap()).is_err());
    }
}

#[test]
fn remote_html_is_readable_and_user_text_is_escaped_before_posting() {
    assert_eq!(
        text_html("<script>alert('x')</script>\nA & B"),
        "<p>&lt;script&gt;alert('x')&lt;/script&gt;<br>A &amp; B</p>"
    );
    let result = readable(
        json!({"description":"<p>Requirements</p><script>bad()</script>","submission":{"body":"<p>My work</p>","submission_comments":[{"message":"<b>Feedback</b>"}]}}),
    );
    assert_eq!(result["description_text"], "Requirements");
    assert_eq!(result["submission"]["body_text"], "My work");
    assert_eq!(
        result["submission"]["submission_comments"][0]["message_text"],
        "Feedback"
    );
}

#[test]
fn writes_accept_created_empty_and_accepted_http_responses() {
    for status in [200, 201, 202, 204] {
        let page = parse_http(&format!(
            "HTTP/2 {status}\r\nContent-Type: application/json\r\n\r\n{{\"id\":7}}\n{status}"
        ))
        .unwrap();
        assert_eq!(page.body, "{\"id\":7}");
    }
}

#[test]
fn invalid_actions_fail_locally_and_frontend_action_names_deserialize() {
    let client = CanvasClient {
        config: CanvasConfig {
            token: "synthetic-not-a-real-token".into(),
            ..Default::default()
        },
    };
    let invalid_actions = [
        json!({"kind":"submit","courseId":0,"assignmentId":1,"submissionType":"online_text_entry","text":"Work","url":"","files":[]}),
        json!({"kind":"comment","courseId":1,"assignmentId":1,"text":""}),
        json!({"kind":"discussionPost","courseId":1,"topicId":1,"entryId":null,"text":""}),
        json!({"kind":"discussionSubscribe","courseId":0,"topicId":1,"subscribed":true}),
        json!({"kind":"moduleDone","courseId":1,"moduleId":1,"itemId":0,"done":true}),
        json!({"kind":"moduleRead","courseId":1,"moduleId":0,"itemId":1}),
        json!({"kind":"message","conversationId":1,"text":""}),
        json!({"kind":"newMessage","recipients":[],"subject":"Test","text":"Work"}),
        json!({"kind":"conversationState","conversationId":1,"state":"deleted"}),
    ];
    for value in invalid_actions {
        let action: WorkspaceAction = serde_json::from_value(value).unwrap();
        assert!(client.workspace_action(action).is_err());
    }
}
