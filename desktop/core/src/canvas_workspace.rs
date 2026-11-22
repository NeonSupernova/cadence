//! Student Canvas workspace. Named operations, never an arbitrary authenticated HTTP proxy.
use crate::canvas::{pages, parse_http, plain_requirements, CanvasClient, Page};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, process::Stdio};
use url::Url;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceQuery {
    pub resource: String,
    pub course_id: Option<i64>,
    pub id: Option<i64>,
    pub parent_id: Option<i64>,
    pub key: Option<String>,
}
pub struct Endpoint {
    pub path: String,
    pub list: bool,
}
fn positive(id: Option<i64>) -> Result<i64, String> {
    id.filter(|id| *id > 0)
        .ok_or("Choose a valid Canvas item.".into())
}
impl WorkspaceQuery {
    pub fn endpoint(&self) -> Result<Endpoint, String> {
        let course = || positive(self.course_id);
        let id = || positive(self.id);
        let parent = || positive(self.parent_id);
        let (path, list) = match self.resource.as_str() {
            "courses" => ("courses?enrollment_state=active&enrollment_type=student&include[]=total_scores&per_page=100".into(), true),
            "calendar" => ("users/self/upcoming_events?per_page=100".into(), true),
            "todo" => ("users/self/todo?per_page=100".into(), true),
            "groups" => ("users/self/groups?per_page=100".into(), true),
            "inbox" => ("conversations?per_page=100".into(), true),
            "recipients" => {
                let search = self.key.as_deref().filter(|s| s.trim().len() >= 2).ok_or("Enter at least two characters to find a recipient.")?;
                let mut params = url::form_urlencoded::Serializer::new(String::new());
                params.append_pair("search", search).append_pair("type", "user").append_pair("per_page", "50");
                if let Some(course) = self.course_id { params.append_pair("context", &format!("course_{}", positive(Some(course))?)); }
                (format!("search/recipients?{}", params.finish()), true)
            }
            "conversation" => (format!("conversations/{}?auto_mark_as_read=false", id()?), false),
            "course" => (format!("courses/{}?include[]=syllabus_body&include[]=total_scores", course()?), false),
            "tabs" => (format!("courses/{}/tabs", course()?), true),
            "assignments" | "grades" => (format!("courses/{}/assignments?include[]=submission&override_assignment_dates=true&per_page=100", course()?), true),
            "assignment" => (format!("courses/{}/assignments/{}?include[]=submission&override_assignment_dates=true", course()?, id()?), false),
            "submission" => (format!("courses/{}/assignments/{}/submissions/self?include[]=submission_comments&include[]=submission_history&include[]=rubric_assessment", course()?, id()?), false),
            "modules" => (format!("courses/{}/modules?per_page=100", course()?), true),
            "moduleItems" => (format!("courses/{}/modules/{}/items?include[]=content_details&per_page=100", course()?, id()?), true),
            "pages" => (format!("courses/{}/pages?per_page=100", course()?), true),
            "page" => {
                let key = self.key.as_deref().filter(|s| !s.trim().is_empty() && ![".", ".."].contains(s) && !s.contains('\0')).ok_or("Choose a page.")?;
                let encoded: String = url::form_urlencoded::byte_serialize(key.as_bytes()).collect();
                (format!("courses/{}/pages/{}", course()?, encoded.replace('+', "%20")), false)
            }
            "files" => (format!("courses/{}/files?per_page=100", course()?), true),
            "announcements" => (format!("courses/{}/discussion_topics?only_announcements=true&per_page=100", course()?), true),
            "discussions" => (format!("courses/{}/discussion_topics?per_page=100", course()?), true),
            "discussion" => (format!("courses/{}/discussion_topics/{}", course()?, id()?), false),
            "discussionView" => (format!("courses/{}/discussion_topics/{}/view", course()?, id()?), false),
            "discussionReplies" => (format!("courses/{}/discussion_topics/{}/entries/{}/replies?per_page=100", course()?, parent()?, id()?), true),
            "quizzes" => (format!("courses/{}/quizzes?per_page=100", course()?), true),
            _ => return Err("This Canvas view is not supported.".into()),
        };
        Ok(Endpoint {
            path: format!("api/v1/{path}"),
            list,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFile {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceAction {
    Submit {
        course_id: i64,
        assignment_id: i64,
        submission_type: String,
        text: String,
        url: String,
        files: Vec<UploadFile>,
    },
    Comment {
        course_id: i64,
        assignment_id: i64,
        text: String,
    },
    DiscussionPost {
        course_id: i64,
        topic_id: i64,
        entry_id: Option<i64>,
        text: String,
    },
    DiscussionSubscribe {
        course_id: i64,
        topic_id: i64,
        subscribed: bool,
    },
    ModuleDone {
        course_id: i64,
        module_id: i64,
        item_id: i64,
        done: bool,
    },
    ModuleRead {
        course_id: i64,
        module_id: i64,
        item_id: i64,
    },
    Message {
        conversation_id: i64,
        text: String,
    },
    NewMessage {
        recipients: Vec<i64>,
        subject: String,
        text: String,
    },
    ConversationState {
        conversation_id: i64,
        state: String,
    },
}
fn text(value: &str) -> Result<&str, String> {
    if value.trim().is_empty() || value.len() > 200_000 {
        return Err("Enter text between 1 and 200,000 bytes.".into());
    }
    Ok(value)
}
// Textarea content is plain text, including when the Canvas endpoint accepts HTML.
pub fn text_html(value: &str) -> String {
    format!(
        "<p>{}</p>",
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\n', "<br>")
    )
}
pub fn validate_submission(
    assignment: &Value,
    submission_type: &str,
    body: &str,
    link: &str,
    files: &[UploadFile],
) -> Result<(), String> {
    if assignment["locked_for_user"].as_bool() == Some(true)
        || assignment["published"].as_bool() == Some(false)
    {
        return Err("This assignment is locked or unavailable. Open Canvas for details.".into());
    }
    for (key, before) in [("unlock_at", true), ("lock_at", false)] {
        if let Some(time) = assignment[key]
            .as_str()
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        {
            if (before && Utc::now() < time) || (!before && Utc::now() >= time) {
                return Err("This assignment is outside its submission window.".into());
            }
        }
    }
    if let Some(max) = assignment["allowed_attempts"].as_i64().filter(|v| *v >= 0) {
        let extra = assignment["submission"]["extra_attempts"]
            .as_i64()
            .unwrap_or(0)
            .max(0);
        if assignment["submission"]["attempt"].as_i64().unwrap_or(0) >= max.saturating_add(extra) {
            return Err("No submission attempts remain for this assignment.".into());
        }
    }
    if !assignment["submission_types"]
        .as_array()
        .is_some_and(|types| types.iter().any(|t| t.as_str() == Some(submission_type)))
    {
        return Err(
            "This assignment does not accept the selected submission type. Reload its details."
                .into(),
        );
    }
    match submission_type {
        "online_text_entry" => {
            text(body)?;
        }
        "online_url" => {
            let url = Url::parse(link.trim()).map_err(|_| "Enter a complete website URL.")?;
            if !["http", "https"].contains(&url.scheme())
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err("Enter an HTTP or HTTPS website URL without login credentials.".into());
            }
        }
        "online_upload" => {
            if files.is_empty()
                || files.len() > 20
                || files.iter().map(|f| f.bytes.len()).sum::<usize>() > 50 * 1024 * 1024
            {
                return Err("Choose 1–20 files totaling no more than 50 MB.".into());
            }
            for file in files {
                validate_file(file)?;
                if let Some(exts) = assignment["allowed_extensions"]
                    .as_array()
                    .filter(|exts| !exts.is_empty())
                {
                    let ext = file.name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
                    if !exts
                        .iter()
                        .any(|v| v.as_str().is_some_and(|v| v.eq_ignore_ascii_case(ext)))
                    {
                        return Err(format!(
                            "{} has a file type this assignment does not accept.",
                            file.name
                        ));
                    }
                }
            }
        }
        _ => return Err("Complete this submission type in Canvas.".into()),
    }
    Ok(())
}
fn validate_file(file: &UploadFile) -> Result<(), String> {
    if file.name.is_empty()
        || file.name.len() > 255
        || file.name.contains(['\r', '\n', '\0', '/', '\\'])
        || file.bytes.len() > 50 * 1024 * 1024
    {
        return Err("Choose a file with a valid name, no larger than 50 MB.".into());
    }
    if file.content_type.contains(['\r', '\n', '\0']) {
        return Err("Invalid file content type.".into());
    }
    Ok(())
}

impl CanvasClient {
    pub fn workspace(&self, query: &WorkspaceQuery) -> Result<Value, String> {
        let base = self.config.validate()?;
        let endpoint = query.endpoint()?;
        let url = base
            .join(&endpoint.path)
            .map_err(|_| "Invalid Canvas address.")?;
        let value = if endpoint.list {
            Value::Array(pages::<Value>(&base, url, |url| self.workspace_get(url))?)
        } else {
            serde_json::from_str(&self.workspace_get(&url)?.body)
                .map_err(|_| "Canvas returned invalid data.")?
        };
        Ok(readable(value))
    }
    fn workspace_get(&self, url: &Url) -> Result<Page, String> {
        let response = self.workspace_http("GET", url, None, true, None)?;
        parse_http(&response)
    }
    fn workspace_json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let base = self.config.validate()?;
        let url = base
            .join(&format!("api/v1/{path}"))
            .map_err(|_| "Invalid Canvas request.")?;
        let response = self.workspace_http(method, &url, body, true, None)?;
        let page = parse_http(&response)?;
        if page.body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&page.body).map_err(|_| {
            "Canvas returned invalid data. Refresh to check the result before trying again.".into()
        })
    }
    pub fn workspace_action(&self, action: WorkspaceAction) -> Result<Value, String> {
        let check = |id| positive(Some(id));
        let value = match action {
            WorkspaceAction::Submit {
                course_id,
                assignment_id,
                submission_type,
                text: body,
                url,
                files,
            } => {
                let path = format!(
                    "courses/{}/assignments/{}",
                    check(course_id)?,
                    check(assignment_id)?
                );
                let assignment = self.workspace_json(
                    "GET",
                    &format!("{path}?include[]=submission&override_assignment_dates=true"),
                    None,
                )?;
                validate_submission(&assignment, &submission_type, &body, &url, &files)?;
                let submission = match submission_type.as_str() {
                    "online_text_entry" => {
                        json!({"submission_type": submission_type, "body": text_html(&body)})
                    }
                    "online_url" => json!({"submission_type": submission_type, "url": url.trim()}),
                    _ => {
                        let mut ids = Vec::new();
                        for file in files {
                            ids.push(self.upload_assignment_file(&path, file)?);
                        }
                        json!({"submission_type": "online_upload", "file_ids": ids})
                    }
                };
                self.workspace_json(
                    "POST",
                    &format!("{path}/submissions"),
                    Some(json!({"submission": submission})),
                )?
            }
            WorkspaceAction::Comment {
                course_id,
                assignment_id,
                text: body,
            } => self.workspace_json(
                "PUT",
                &format!(
                    "courses/{}/assignments/{}/submissions/self",
                    check(course_id)?,
                    check(assignment_id)?
                ),
                Some(json!({"comment":{"text_comment":text(&body)?}})),
            )?,
            WorkspaceAction::DiscussionPost {
                course_id,
                topic_id,
                entry_id,
                text: body,
            } => {
                let mut path = format!(
                    "courses/{}/discussion_topics/{}/entries",
                    check(course_id)?,
                    check(topic_id)?
                );
                if let Some(id) = entry_id {
                    path.push_str(&format!("/{}/replies", check(id)?));
                }
                self.workspace_json(
                    "POST",
                    &path,
                    Some(json!({"message":text_html(text(&body)?)})),
                )?
            }
            WorkspaceAction::DiscussionSubscribe {
                course_id,
                topic_id,
                subscribed,
            } => self.workspace_json(
                if subscribed { "PUT" } else { "DELETE" },
                &format!(
                    "courses/{}/discussion_topics/{}/subscribed",
                    check(course_id)?,
                    check(topic_id)?
                ),
                None,
            )?,
            WorkspaceAction::ModuleDone {
                course_id,
                module_id,
                item_id,
                done,
            } => self.workspace_json(
                if done { "PUT" } else { "DELETE" },
                &format!(
                    "courses/{}/modules/{}/items/{}/done",
                    check(course_id)?,
                    check(module_id)?,
                    check(item_id)?
                ),
                None,
            )?,
            WorkspaceAction::ModuleRead {
                course_id,
                module_id,
                item_id,
            } => self.workspace_json(
                "POST",
                &format!(
                    "courses/{}/modules/{}/items/{}/mark_read",
                    check(course_id)?,
                    check(module_id)?,
                    check(item_id)?
                ),
                None,
            )?,
            WorkspaceAction::Message {
                conversation_id,
                text: body,
            } => self.workspace_json(
                "POST",
                &format!("conversations/{}/add_message", check(conversation_id)?),
                Some(json!({"body":text(&body)?})),
            )?,
            WorkspaceAction::NewMessage {
                recipients,
                subject,
                text: body,
            } => {
                if recipients.is_empty()
                    || recipients.len() > 20
                    || recipients.iter().any(|id| *id <= 0)
                    || subject.trim().is_empty()
                    || subject.len() > 255
                {
                    return Err("Enter a subject and 1–20 numeric Canvas recipient IDs.".into());
                }
                self.workspace_json("POST", "conversations", Some(json!({"recipients":recipients,"subject":subject,"body":text(&body)?,"group_conversation":true})))?
            }
            WorkspaceAction::ConversationState {
                conversation_id,
                state,
            } => {
                if !["read", "unread", "archived"].contains(&state.as_str()) {
                    return Err("Invalid conversation state.".into());
                }
                self.workspace_json(
                    "PUT",
                    &format!("conversations/{}", check(conversation_id)?),
                    Some(json!({"conversation":{"workflow_state":state}})),
                )?
            }
        };
        Ok(readable(value))
    }
    fn upload_assignment_file(&self, path: &str, file: UploadFile) -> Result<i64, String> {
        validate_file(&file)?;
        let ticket = self.workspace_json("POST", &format!("{path}/submissions/self/files"), Some(json!({"name":file.name,"size":file.bytes.len(),"content_type":file.content_type,"on_duplicate":"rename"})))?;
        let upload_url = upload_url(
            ticket["upload_url"]
                .as_str()
                .ok_or("Canvas did not provide an upload address.")?,
        )?;
        let params = ticket["upload_params"]
            .as_object()
            .ok_or("Canvas did not provide upload parameters.")?;
        let boundary = format!(
            "CadenceUpload{}{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let mut multipart = Vec::new();
        for (key, value) in params {
            if key == "file" || key.contains(['\r', '\n', '\0']) {
                return Err("Canvas returned invalid upload parameters.".into());
            }
            let value = value
                .as_str()
                .ok_or("Canvas returned invalid upload parameters.")?;
            write!(
                multipart,
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n{}\r\n",
                mime_quote(key),
                value
            )
            .map_err(|_| "Could not prepare upload.")?;
        }
        write!(multipart, "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n", mime_quote(&file.name), if file.content_type.is_empty() { "application/octet-stream" } else { &file.content_type }).map_err(|_| "Could not prepare upload.")?;
        multipart.extend_from_slice(&file.bytes);
        write!(multipart, "\r\n--{boundary}--\r\n").map_err(|_| "Could not prepare upload.")?;
        let temp = PrivateBody::new(&multipart)?;
        // Canvas upload tickets can point to S3. Never attach the bearer token to this request.
        let output = self.workspace_http(
            "POST",
            &upload_url,
            None,
            false,
            Some((&temp.path, &boundary)),
        )?;
        let (_, status) = output.rsplit_once('\n').ok_or("Invalid upload response.")?;
        let location = response_location(&output);
        let value: Value = if let Some(location) = location {
            let base = self.config.validate()?;
            let target = upload_url
                .join(&location)
                .map_err(|_| "Invalid upload confirmation address.")?;
            validate_confirmation(&base, &target)?;
            let page = self.workspace_get(&target)?;
            serde_json::from_str(&page.body).map_err(|_| "Invalid upload confirmation.")?
        } else if ["200", "201"].contains(&status.trim()) {
            serde_json::from_str(&parse_http(&output)?.body)
                .map_err(|_| "Invalid upload receipt.")?
        } else {
            return Err(
                "Canvas could not complete the file upload. No submission was sent.".into(),
            );
        };
        positive(value["id"].as_i64())
    }
    fn workspace_http(
        &self,
        method: &str,
        url: &Url,
        body: Option<Value>,
        authenticated: bool,
        multipart: Option<(&PathBuf, &str)>,
    ) -> Result<String, String> {
        let config = request_config(&self.config, method, url, body, authenticated, multipart)?;
        let mut child = crate::curl_command()
            .args([
                "--config",
                "-",
                "--globoff",
                "--silent",
                "--show-error",
                "--include",
                "--proto",
                "=https",
                "--connect-timeout",
                "10",
                "--max-time",
                "180",
                "--max-filesize",
                "16777216",
                "--write-out",
                "\n%{http_code}",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| "Could not start Canvas HTTPS client.")?;
        child
            .stdin
            .take()
            .ok_or("Could not send Canvas request.")?
            .write_all(config.as_bytes())
            .map_err(|_| "Could not send Canvas request.")?;
        let output = child
            .wait_with_output()
            .map_err(|_| "Could not read Canvas response.")?;
        if !output.status.success() {
            return Err("Canvas request was interrupted. Refresh to check its status before retrying; it may have reached Canvas.".into());
        }
        let response =
            String::from_utf8(output.stdout).map_err(|_| "Canvas returned invalid text.")?;
        let (_, status) = response
            .rsplit_once('\n')
            .ok_or("Invalid Canvas response.")?;
        if authenticated
            || !["200", "201", "301", "302", "303", "307", "308"].contains(&status.trim())
        {
            match status.trim() {
                "200" | "201" | "202" | "204" => {},
                "400" | "422" => return Err("Canvas rejected this request. Check submission requirements, attempts, recipient IDs, or course restrictions, then refresh.".into()),
                "401" => return Err("Canvas rejected the token or its API scope. Check your connection in Settings.".into()),
                "403" => return Err("Canvas does not allow this action for your account, or this content is locked.".into()),
                "404" => return Err("This content is unavailable or this feature is not enabled by your school.".into()),
                "429" => return Err("Canvas rate limit reached. Wait a few minutes and try again.".into()),
                code => return Err(format!("Canvas returned HTTP {code}. Refresh to check the result before retrying.")),
            }
        }
        Ok(response)
    }
}

fn request_config(
    connection: &crate::canvas::CanvasConfig,
    method: &str,
    url: &Url,
    body: Option<Value>,
    authenticated: bool,
    multipart: Option<(&PathBuf, &str)>,
) -> Result<String, String> {
    let base = connection.validate()?;
    if authenticated && url.origin() != base.origin() {
        return Err("Canvas credentials cannot be sent to another site.".into());
    }
    let quote = |s: &str| serde_json::to_string(s).unwrap();
    let mut config = format!(
        "url = {}\nrequest = {}\nheader = \"Accept: application/json\"\n",
        quote(url.as_str()),
        quote(method)
    );
    if authenticated {
        config.push_str(&format!(
            "header = {}\n",
            quote(&format!("Authorization: Bearer {}", connection.token))
        ));
    }
    if let Some(body) = body {
        config.push_str(&format!(
            "header = \"Content-Type: application/json\"\ndata-binary = {}\n",
            quote(&body.to_string())
        ));
    }
    if let Some((path, boundary)) = multipart {
        config.push_str(&format!(
            "header = {}\ndata-binary = {}\n",
            quote(&format!(
                "Content-Type: multipart/form-data; boundary={boundary}"
            )),
            quote(&format!("@{}", path.display()))
        ));
    }
    Ok(config)
}

// Add plain text alongside HTML; the frontend never injects remote HTML into its webview.
pub fn readable(mut value: Value) -> Value {
    match &mut value {
        Value::Array(items) => {
            for item in items {
                *item = readable(item.take());
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                *item = readable(item.take());
            }
            for key in [
                "description",
                "body",
                "message",
                "syllabus_body",
                "lock_explanation",
            ] {
                if let Some(html) = fields.get(key).and_then(Value::as_str) {
                    let plain = plain_requirements(html);
                    fields.insert(format!("{key}_text"), Value::String(plain));
                }
            }
        }
        _ => {}
    }
    value
}
fn mime_quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
pub fn upload_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Invalid upload URL.")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Canvas returned an unsafe upload address.".into());
    }
    Ok(url)
}
pub fn validate_confirmation(base: &Url, target: &Url) -> Result<(), String> {
    let parts: Vec<_> = target.path().trim_matches('/').split('/').collect();
    if target.origin() != base.origin()
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
        || !(parts.len() == 4 || parts.len() == 5 && parts[4] == "create_success")
        || parts[..3] != ["api", "v1", "files"]
        || parts[3].parse::<i64>().ok().is_none_or(|id| id <= 0)
    {
        return Err("Canvas returned an upload confirmation outside its files API.".into());
    }
    Ok(())
}
fn response_location(response: &str) -> Option<String> {
    let mut rest = response;
    let mut location = None;
    while rest.starts_with("HTTP/") {
        let (headers, body) = rest
            .split_once("\r\n\r\n")
            .or_else(|| rest.split_once("\n\n"))?;
        location = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.trim().to_string());
        rest = body;
    }
    location
}
struct PrivateBody {
    path: PathBuf,
}
impl PrivateBody {
    fn new(bytes: &[u8]) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "cadence-upload-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|_| "Could not prepare temporary upload.")?;
        let temporary = Self { path };
        file.write_all(bytes)
            .map_err(|_| "Could not prepare temporary upload.")?;
        Ok(temporary)
    }
}
impl Drop for PrivateBody {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::CanvasConfig;

    #[test]
    fn bearer_token_only_goes_to_canvas_and_upload_request_uses_private_binary_body() {
        let connection = CanvasConfig {
            token: "synthetic-secret".into(),
            ..Default::default()
        };
        let base = connection.validate().unwrap();
        let canvas = base
            .join("api/v1/courses/1/assignments/2/submissions")
            .unwrap();
        let upload = upload_url("https://uploads.example.com/signed?opaque=1").unwrap();
        let config = request_config(
            &connection,
            "POST",
            &canvas,
            Some(json!({"submission":{"body":"Line 1\nLine 2 \"quoted\""}})),
            true,
            None,
        )
        .unwrap();
        assert!(config.contains("Authorization: Bearer synthetic-secret"));
        assert!(!config.contains("location"));
        assert!(request_config(&connection, "POST", &upload, None, true, None).is_err());
        let bytes = [0, 255, 13, 10, 34];
        let body = PrivateBody::new(&bytes).unwrap();
        let path = body.path.clone();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let config = request_config(
            &connection,
            "POST",
            &upload,
            None,
            false,
            Some((&body.path, "test-boundary")),
        )
        .unwrap();
        assert!(!config.contains("synthetic-secret"));
        assert!(!config.contains("Authorization"));
        assert!(config.contains("multipart/form-data; boundary=test-boundary"));
        drop(body);
        assert!(!path.exists());
    }

    #[test]
    fn upload_confirmation_uses_final_response_headers_only() {
        let response="HTTP/1.1 200 Connection established\r\nLocation: https://wrong.example\r\n\r\nHTTP/2 302\r\nlocation: https://pcci.instructure.com/api/v1/files/7/create_success?uuid=opaque\r\n\r\n\n302";
        assert_eq!(
            response_location(response).unwrap(),
            "https://pcci.instructure.com/api/v1/files/7/create_success?uuid=opaque"
        );
        assert_eq!(response_location("HTTP/2 201\r\n\r\n{\"id\":7}\n201"), None);
    }
}
