use crate::Task;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalReference {
    pub provider: String,
    pub remote_id: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Organization {
    pub role: String,
    pub parent_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub cycle_id: Option<i64>,
    pub related_ids: Vec<i64>,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub external: Vec<ExternalReference>,
}
impl Default for Organization {
    fn default() -> Self {
        Self {
            role: "task".into(),
            parent_id: None,
            milestone_id: None,
            cycle_id: None,
            related_ids: vec![],
            starts_on: None,
            ends_on: None,
            external: vec![],
        }
    }
}
pub fn validate(tasks: &[Task]) -> Result<(), String> {
    let map: BTreeMap<_, _> = tasks.iter().map(|t| (t.id, t)).collect();
    for task in tasks {
        let o = &task.organization;
        if !["task", "epic", "milestone", "sprint", "iteration"].contains(&o.role.as_str()) {
            return Err("Choose task, epic, milestone, sprint, or iteration.".into());
        }
        if o.role != "task" && task.timer_started_at.is_some() {
            return Err("Pause the timer before changing this task into a planning item.".into());
        }
        for date in [&o.starts_on, &o.ends_on].into_iter().flatten() {
            if date.len() != 10 {
                return Err("Use YYYY-MM-DD planning dates.".into());
            }
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| "Use YYYY-MM-DD planning dates.")?;
        }
        if o.starts_on
            .as_ref()
            .zip(o.ends_on.as_ref())
            .is_some_and(|(s, e)| s > e)
        {
            return Err("The planning end must follow its start.".into());
        }
        for (id, roles) in [
            (o.parent_id, vec!["epic"]),
            (o.milestone_id, vec!["milestone"]),
            (o.cycle_id, vec!["sprint", "iteration"]),
        ] {
            if let Some(id) = id {
                let target = map.get(&id).ok_or("Linked item does not exist.")?;
                if id == task.id || !roles.contains(&target.organization.role.as_str()) {
                    return Err("Link target has the wrong role or refers to itself.".into());
                }
            }
        }
        for id in &o.related_ids {
            if *id == task.id || !map.contains_key(id) {
                return Err("Related items must reference other existing tasks.".into());
            }
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(task.id);
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err("Epic links cannot form a cycle.".into());
            }
            current = map.get(&id).and_then(|t| t.organization.parent_id);
        }
        if o.external.len() > 20 || o.related_ids.len() > 100 {
            return Err("Too many links on one item.".into());
        }
        for link in &o.external {
            let url = url::Url::parse(&link.url).map_err(|_| "Use a full external URL.")?;
            if !["http", "https"].contains(&url.scheme())
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || link.provider.trim().is_empty()
                || link.remote_id.trim().is_empty()
            {
                return Err("External references need a provider, remote ID, and HTTP(S) URL without credentials.".into());
            }
        }
    }
    Ok(())
}
