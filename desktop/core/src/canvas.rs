//! Read-only Canvas integration. Credentials never enter task/model context.
use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Write, path::Path, process::Stdio};
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CanvasConfig {
    pub base_url: String,
    pub token: String,
    pub auto_sync: bool,
}
impl Default for CanvasConfig {
    fn default() -> Self {
        Self {
            base_url: "https://pcci.instructure.com".into(),
            token: String::new(),
            auto_sync: true,
        }
    }
}
impl CanvasConfig {
    pub fn load(data_dir: &Path) -> Self {
        let mut config = std::fs::read_to_string(data_dir.join("canvas-settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default();
        if !config.token.is_empty() {
            return config;
        }
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent);
        for path in project
            .map(|p| p.join(".env"))
            .into_iter()
            .chain([data_dir.join(".env")])
        {
            if let Ok(contents) = std::fs::read_to_string(path) {
                for line in contents
                    .lines()
                    .filter(|l| !l.trim_start().starts_with('#'))
                {
                    if let Some((key, value)) = line.split_once('=') {
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        match key.trim() {
                            "CANVAS_API_TOKEN" => config.token = value.into(),
                            "CANVAS_BASE_URL" => config.base_url = value.into(),
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Ok(value) = std::env::var("CANVAS_API_TOKEN") {
            config.token = value;
        }
        if let Ok(value) = std::env::var("CANVAS_BASE_URL") {
            config.base_url = value;
        }
        config
    }
    pub fn validate(&self) -> Result<Url, String> {
        if self.token.trim().is_empty() {
            return Err("Add your Canvas access token in Settings → Canvas.".into());
        }
        if self.token.contains(['\r', '\n']) {
            return Err("Enter a valid Canvas token.".into());
        }
        normalize_base(&self.base_url)
    }
    pub fn save(&mut self, data_dir: &Path) -> Result<(), String> {
        self.base_url = self.validate()?.as_str().trim_end_matches('/').to_string();
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(data_dir.join("canvas-settings.json"))
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        file.write_all(
            serde_json::to_string_pretty(self)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())
    }
}
pub fn normalize_base(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| {
        "Enter your Canvas HTTPS address, for example https://pcci.instructure.com."
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(
            "Use only your Canvas HTTPS origin, without a path, login, query, or fragment.".into(),
        );
    }
    Ok(url)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSource {
    #[serde(default)]
    pub requirements: String,
    pub base_url: String,
    pub course_id: i64,
    pub assignment_id: i64,
    pub course_name: String,
    pub course_code: String,
    pub title: String,
    pub url: String,
    pub due_at: Option<String>,
    pub due_date: Option<String>,
    pub submitted: bool,
    pub synced_at: String,
    pub suggested_priority: u8,
    #[serde(default)]
    pub submitted_at: Option<String>,
}
impl CanvasSource {
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.base_url, self.course_id, self.assignment_id
        )
    }
    pub fn course_label(&self) -> String {
        let label: String = self
            .course_code
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        format!(
            "course:{}",
            label.trim_matches('-').chars().take(30).collect::<String>()
        )
    }
    pub fn description(&self) -> String {
        if !self.requirements.trim().is_empty() {
            return self.requirements.clone();
        }
        format!(
            "Canvas assignment for {}. Open the assignment in Canvas for the full requirements.",
            self.course_name
        )
    }
}
#[derive(Debug, Clone)]
pub struct CanvasImport {
    pub courses: usize,
    pub assignments: Vec<CanvasSource>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasReport {
    pub courses: usize,
    pub assignments: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub warnings: Vec<String>,
    pub synced_at: String,
}
#[derive(Deserialize)]
pub struct Course {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub course_code: String,
    pub workflow_state: Option<String>,
}
#[derive(Deserialize)]
pub struct Assignment {
    pub description: Option<String>,
    pub id: i64,
    pub name: String,
    pub html_url: Option<String>,
    pub due_at: Option<String>,
    pub published: Option<bool>,
    pub submission: Option<Submission>,
}
#[derive(Deserialize)]
pub struct Submission {
    pub workflow_state: Option<String>,
    pub excused: Option<bool>,
    pub submitted_at: Option<String>,
    pub graded_at: Option<String>,
}

pub fn assignment_source(
    base: &Url,
    course: &Course,
    assignment: Assignment,
) -> Result<Option<CanvasSource>, String> {
    if assignment.published == Some(false) {
        return Ok(None);
    }
    if assignment.id <= 0 || course.id <= 0 || assignment.name.trim().is_empty() {
        return Err("Canvas returned an invalid assignment.".into());
    }
    let due_date = match assignment.due_at.as_deref() {
        Some(value) => Some(
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| "Canvas returned an invalid assignment deadline.")?
                .with_timezone(&Local)
                .format("%Y-%m-%d")
                .to_string(),
        ),
        None => None,
    };
    let canonical = base
        .join(&format!(
            "courses/{}/assignments/{}",
            course.id, assignment.id
        ))
        .map_err(|_| "Invalid assignment URL.")?;
    let url = assignment
        .html_url
        .as_deref()
        .and_then(|s| Url::parse(s).ok())
        .filter(|u| {
            u.origin() == base.origin() && u.username().is_empty() && u.password().is_none()
        })
        .unwrap_or(canonical);
    let submitted = assignment.submission.as_ref().is_some_and(|s| {
        s.excused == Some(true)
            || matches!(s.workflow_state.as_deref(), Some("submitted" | "graded"))
    });
    let submitted_at = assignment
        .submission
        .as_ref()
        .and_then(|s| s.submitted_at.as_ref().or(s.graded_at.as_ref()))
        .filter(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
        .cloned();
    Ok(Some(CanvasSource {
        requirements: assignment
            .description
            .as_deref()
            .map(plain_requirements)
            .unwrap_or_default(),
        submitted_at,
        base_url: base.as_str().trim_end_matches('/').into(),
        course_id: course.id,
        assignment_id: assignment.id,
        course_name: course.name.clone(),
        course_code: if course.course_code.is_empty() {
            format!("course-{}", course.id)
        } else {
            course.course_code.clone()
        },
        title: assignment.name.trim().chars().take(140).collect(),
        url: url.into(),
        suggested_priority: deadline_priority(assignment.due_at.as_deref()),
        due_at: assignment.due_at,
        due_date,
        submitted,
        synced_at: Utc::now().to_rfc3339(),
    }))
}
pub fn deadline_priority(due_at: Option<&str>) -> u8 {
    let Some(due) = due_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) else {
        return 40;
    };
    match (due.with_timezone(&Utc) - Utc::now()).num_hours() {
        ..=24 => 95,
        25..=72 => 80,
        73..=168 => 65,
        _ => 45,
    }
}

#[derive(Debug)]
pub struct Page {
    pub body: String,
    pub next: Option<String>,
}
pub fn parse_http(output: &str) -> Result<Page, String> {
    let (mut text, status) = output
        .rsplit_once('\n')
        .ok_or("Canvas returned an invalid response.")?;
    match status.trim() {
        "200" => {}
        "401" => {
            return Err(
                "Canvas rejected the token. Generate a new token in Canvas Account → Settings."
                    .into(),
            )
        }
        "403" => return Err("Your Canvas token does not have access to these assignments.".into()),
        "429" => {
            return Err(
                "Canvas rate limit reached. Wait a few minutes before syncing again.".into(),
            )
        }
        other => {
            return Err(format!(
                "Canvas returned HTTP {other}. No assignments were changed."
            ))
        }
    }
    let mut next = None;
    while text.starts_with("HTTP/") {
        let (headers, body) = text
            .split_once("\r\n\r\n")
            .or_else(|| text.split_once("\n\n"))
            .ok_or("Canvas returned invalid response headers.")?;
        next = None;
        for line in headers.lines() {
            if let Some((key, value)) = line.split_once(':') {
                if key.eq_ignore_ascii_case("link") {
                    for link in link_parts(value) {
                        let mut parts = link.trim().split(';');
                        let target = parts
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_start_matches('<')
                            .trim_end_matches('>');
                        if parts.any(|p| {
                            p.trim().split_once('=').is_some_and(|(key, value)| {
                                key.trim() == "rel"
                                    && value
                                        .trim()
                                        .trim_matches('"')
                                        .split_whitespace()
                                        .any(|r| r == "next")
                            })
                        }) {
                            next = Some(target.to_string());
                        }
                    }
                }
            }
        }
        text = body;
    }
    Ok(Page {
        body: text.to_string(),
        next,
    })
}
fn link_parts(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut start, mut angle, mut quoted) = (0, false, false);
    for (index, c) in value.char_indices() {
        match c {
            '<' if !quoted => angle = true,
            '>' if !quoted => angle = false,
            '"' if !angle => quoted = !quoted,
            ',' if !angle && !quoted => {
                parts.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}
pub fn validated_page_url(base: &Url, expected_path: &str, value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Canvas returned an invalid pagination URL.")?;
    if url.origin() != base.origin()
        || url.path() != expected_path
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Canvas returned a pagination link outside the requested endpoint. Sync stopped."
                .into(),
        );
    }
    Ok(url)
}
pub fn pages<T: serde::de::DeserializeOwned>(
    base: &Url,
    initial: Url,
    mut fetch: impl FnMut(&Url) -> Result<Page, String>,
) -> Result<Vec<T>, String> {
    let mut url = initial;
    let path = url.path().to_string();
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    loop {
        if seen.len() >= 500 || !seen.insert(url.as_str().to_string()) {
            return Err("Canvas pagination repeated or exceeded the sync limit. No assignments were changed.".into());
        }
        let page = fetch(&url)?;
        items.extend(
            serde_json::from_str::<Vec<T>>(&page.body)
                .map_err(|_| "Canvas returned an invalid list. No assignments were changed.")?,
        );
        match page.next {
            Some(next) => url = validated_page_url(base, &path, &next)?,
            None => break,
        }
    }
    Ok(items)
}
pub struct CanvasClient {
    pub config: CanvasConfig,
}
impl CanvasClient {
    fn get(&self, url: &Url) -> Result<Page, String> {
        let quote = |s: &str| serde_json::to_string(s).unwrap();
        let config = format!(
            "url = {}\nrequest = \"GET\"\nheader = {}\nheader = \"Accept: application/json\"\n",
            quote(url.as_str()),
            quote(&format!("Authorization: Bearer {}", self.config.token))
        );
        let mut child = crate::curl_command()
            .args([
                "--config",
                "-",
                "--globoff",
                "--silent",
                "--show-error",
                "--include",
                "--connect-timeout",
                "10",
                "--max-time",
                "60",
                "--max-filesize",
                "8388608",
                "--write-out",
                "\n%{http_code}",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| "Could not start the Canvas HTTPS client.")?;
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
            return Err(
                "Could not reach Canvas. Check your connection. No assignments were changed."
                    .into(),
            );
        }
        parse_http(&String::from_utf8(output.stdout).map_err(|_| "Canvas returned invalid text.")?)
    }
    pub fn assignments(&self) -> Result<CanvasImport, String> {
        let base = self.config.validate()?;
        let courses_url = base
            .join("api/v1/courses?enrollment_state=active&enrollment_type=student&per_page=100")
            .map_err(|_| "Invalid Canvas address.")?;
        let courses: Vec<Course> = pages(&base, courses_url, |url| self.get(url))?;
        let mut import = CanvasImport {
            courses: 0,
            assignments: vec![],
            warnings: vec![],
        };
        for course in courses {
            if course
                .workflow_state
                .as_deref()
                .is_some_and(|s| s != "available")
            {
                continue;
            }
            let url = base.join(&format!("api/v1/courses/{}/assignments?include[]=submission&override_assignment_dates=true&per_page=100", course.id)).map_err(|_| "Invalid course URL.")?;
            let assignments: Vec<Assignment> = pages(&base, url, |url| self.get(url))?;
            import.courses += 1;
            for assignment in assignments {
                if let Some(source) = assignment_source(&base, &course, assignment)? {
                    import.assignments.push(source);
                }
            }
        }
        Ok(import)
    }
}

// Store readable requirements, never render imported HTML in the webview.
pub fn plain_requirements(html: &str) -> String {
    let mut output = String::new();
    let mut rest = html;
    let mut hidden: Option<String> = None;
    while !rest.is_empty() && output.len() < 100_000 {
        if rest.starts_with('<') {
            if let Some(end) = rest.find('>') {
                let tag = rest[1..end].trim().to_ascii_lowercase();
                let name = tag
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/');
                if ["script", "style", "iframe"].contains(&name) {
                    hidden = Some(name.into());
                } else if hidden.as_ref().is_some_and(|h| name == format!("/{h}")) {
                    hidden = None;
                } else if hidden.is_none()
                    && [
                        "p", "/p", "div", "/div", "br", "li", "/li", "h1", "/h1", "h2", "/h2",
                        "h3", "/h3", "tr", "/tr", "ul", "/ul", "ol", "/ol",
                    ]
                    .contains(&name)
                {
                    output.push('\n');
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        if hidden.is_none() {
            output.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    let mut decoded = String::new();
    let mut rest = output.as_str();
    while !rest.is_empty() {
        if rest.starts_with('&') {
            if let Some(end) = rest.find(';').filter(|end| *end <= 12) {
                let entity = &rest[1..end];
                let value = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => entity
                        .strip_prefix("#x")
                        .and_then(|s| u32::from_str_radix(s, 16).ok())
                        .or_else(|| entity.strip_prefix('#').and_then(|s| s.parse().ok()))
                        .and_then(char::from_u32),
                };
                if let Some(value) = value {
                    decoded.push(value);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        let c = rest.chars().next().unwrap();
        decoded.push(c);
        rest = &rest[c.len_utf8()..];
    }
    decoded
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(20_000)
        .collect()
}
