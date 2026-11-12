//! Personal workload calibration. Due dates affect scheduling, never estimated effort.
use crate::{Status, Task, TYPES};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    pub minutes: u32,
    pub technical: u8,
    pub tedium: u8,
    pub ambiguity: u8,
    pub switching: u8,
    pub source: String,
}
impl Estimate {
    pub fn validate(&self) -> Result<(), String> {
        if !(5..=2400).contains(&self.minutes)
            || [self.technical, self.tedium, self.ambiguity, self.switching]
                .iter()
                .any(|v| !(1..=5).contains(v))
        {
            return Err("Use 5–2400 minutes and factor ratings from 1 to 5.".into());
        }
        if !["user", "jev", "starter"].contains(&self.source.as_str()) {
            return Err("Invalid estimate source.".into());
        }
        Ok(())
    }
    pub fn for_task(task: &Task) -> Self {
        if let Some(estimate) = &task.estimate {
            return estimate.clone();
        }
        let (minutes, technical, tedium, ambiguity, switching) = match task.kind.as_str() {
            "bug" => (90, 4, 2, 4, 3),
            "feature" => (120, 4, 2, 3, 3),
            "assignment" => (90, 3, 3, 4, 2),
            "research" => (60, 3, 2, 4, 2),
            "meeting" => (30, 1, 2, 2, 4),
            "personal" => (25, 1, 2, 2, 2),
            _ => (30, 2, 4, 2, 2),
        };
        Self {
            minutes,
            technical,
            tedium,
            ambiguity,
            switching,
            source: "starter".into(),
        }
    }
    fn factors(&self) -> [f64; 5] {
        [
            1.0,
            (self.technical as f64 - 3.0) / 2.0,
            (self.tedium as f64 - 3.0) / 2.0,
            (self.ambiguity as f64 - 3.0) / 2.0,
            (self.switching as f64 - 3.0) / 2.0,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CapacityProfile {
    pub weekday_minutes: [u32; 7],
    pub learning_enabled: bool,
}
impl Default for CapacityProfile {
    fn default() -> Self {
        Self {
            weekday_minutes: [120, 120, 120, 120, 120, 60, 60],
            learning_enabled: true,
        }
    }
}
impl CapacityProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.weekday_minutes.iter().any(|m| *m > 720) {
            return Err("Daily focus capacity must be between 0 and 720 minutes.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkin {
    pub date: String,
    pub minutes: u32,
    pub energy: u8,
}
impl Checkin {
    pub fn validate(&self) -> Result<(), String> {
        NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").map_err(|_| "Choose a valid date.")?;
        if self.minutes > 720 || !(1..=5).contains(&self.energy) {
            return Err("Use 0–720 available minutes and energy from 1 to 5.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub task_id: i64,
    pub kind: String,
    pub estimate: Estimate,
    pub actual_minutes: f64,
    pub effort_rating: Option<u8>,
    pub recorded_at: String,
    pub time_reliable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayReview {
    pub date: String,
    pub available_minutes: u32,
    pub energy: u8,
    pub spent_minutes: f64,
    pub spent_units: f64,
    pub rating: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feedback {
    pub task_id: i64,
    pub rating: u8,
    pub actual_minutes: Option<f64>,
}
pub fn feedback_outcome(task: &Task, feedback: &Feedback) -> Result<Outcome, String> {
    if !(1..=5).contains(&feedback.rating) {
        return Err("Effort feedback must be between 1 and 5.".into());
    }
    if feedback
        .actual_minutes
        .is_some_and(|m| !m.is_finite() || !(5.0..=960.0).contains(&m))
    {
        return Err("Actual work time must be between 5 and 960 minutes.".into());
    }
    if task.status != Status::Done {
        return Err("Complete the task before rating its effort.".into());
    }
    let tracked = task.tracked_ms as f64 / 60_000.0;
    let reliable = feedback.actual_minutes.is_some()
        || ((5.0..=960.0).contains(&tracked)
            && !task
                .time_entries
                .iter()
                .any(|e| e.duration_ms > 4 * 60 * 60 * 1000));
    Ok(Outcome {
        task_id: task.id,
        kind: task.kind.clone(),
        estimate: task
            .baseline_estimate
            .clone()
            .unwrap_or_else(|| Estimate::for_task(task)),
        actual_minutes: feedback.actual_minutes.unwrap_or(tracked),
        time_reliable: reliable,
        effort_rating: Some(feedback.rating),
        recorded_at: Utc::now().to_rfc3339(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub time_samples: usize,
    pub effort_samples: usize,
    pub day_samples: usize,
    pub time_coefficients: Vec<f64>,
    pub load_weights: [f64; 5],
    pub weekday_factors: [f64; 7],
    pub uncertainty: f64,
    pub stage: String,
}
fn features(estimate: &Estimate, kind: &str) -> Vec<f64> {
    let mut x = estimate.factors().to_vec();
    x.push((estimate.minutes as f64 / 60.0).ln().clamp(-2.0, 2.0));
    x.extend(
        TYPES
            .iter()
            .map(|(name, _)| if *name == kind { 1.0 } else { 0.0 }),
    );
    x
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn energy_factor(energy: u8) -> f64 {
    [0.60, 0.80, 1.0, 1.12, 1.25][energy.clamp(1, 5) as usize - 1]
}
impl Calibration {
    pub fn learn(
        outcomes: &[Outcome],
        reviews: &[DayReview],
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Self {
        let prior = [1.0, 0.20, 0.15, 0.12, 0.08];
        let mut model = Self {
            time_samples: 0,
            effort_samples: 0,
            day_samples: 0,
            time_coefficients: vec![0.0; 6 + TYPES.len()],
            load_weights: prior,
            weekday_factors: [1.0; 7],
            uncertainty: 0.70,
            stage: "Starter estimates".into(),
        };
        let valid: Vec<_> = outcomes
            .iter()
            .filter(|o| {
                o.actual_minutes.is_finite()
                    && ((!o.time_reliable && o.effort_rating.is_some())
                        || (5.0..=960.0).contains(&o.actual_minutes))
                    && o.estimate.validate().is_ok()
            })
            .rev()
            .take(200)
            .collect();
        model.time_samples = valid.iter().filter(|o| o.time_reliable).count();
        model.effort_samples = valid.iter().filter(|o| o.effort_rating.is_some()).count();
        // Replay recent observations with bounded, regularized Huber updates. Replaying
        // the same history is deterministic; reading a snapshot never trains twice.
        for _ in 0..80 {
            for observation in valid.iter().rev() {
                let age = DateTime::parse_from_rfc3339(&observation.recorded_at)
                    .map(|d| (now - d.with_timezone(&Utc)).num_days().max(0) as f64)
                    .unwrap_or(0.0);
                let decay = 0.5_f64.powf(age / 90.0);
                let x = features(&observation.estimate, &observation.kind);
                let target = (observation.actual_minutes / observation.estimate.minutes as f64)
                    .ln()
                    .clamp(-1.6, 1.6);
                let residual = (dot(&model.time_coefficients, &x) - target).clamp(-0.7, 0.7);
                if observation.time_reliable {
                    for (i, coefficient) in model.time_coefficients.iter_mut().enumerate() {
                        *coefficient = (*coefficient
                            - 0.025 * (decay * residual * x[i] + 0.04 * *coefficient))
                            .clamp(-1.0, 1.0);
                    }
                }
                if let Some(rating) = observation.effort_rating.filter(|r| (1..=5).contains(r)) {
                    let target = [0.6, 0.8, 1.0, 1.3, 1.65][rating as usize - 1];
                    let factors = observation.estimate.factors();
                    let residual = (dot(&model.load_weights, &factors) - target).clamp(-0.6, 0.6);
                    for i in 0..5 {
                        model.load_weights[i] = (model.load_weights[i]
                            - 0.025
                                * (decay * residual * factors[i]
                                    + 0.08 * (model.load_weights[i] - prior[i])))
                            .clamp(
                                if i == 0 { 0.6 } else { 0.0 },
                                if i == 0 { 1.8 } else { 0.8 },
                            );
                    }
                }
            }
        }
        if model.time_samples > 0 {
            let error = valid
                .iter()
                .filter(|o| o.time_reliable)
                .map(|o| {
                    let residual = (o.actual_minutes / model.minutes(&o.estimate, &o.kind)).ln();
                    residual * residual
                })
                .sum::<f64>()
                / model.time_samples as f64;
            model.uncertainty = error
                .sqrt()
                .clamp(if model.time_samples < 5 { 0.60 } else { 0.25 }, 1.0);
        }
        for review in reviews.iter().rev().take(56).rev() {
            if review.spent_minutes < 15.0 || review.available_minutes == 0 {
                continue;
            }
            let Ok(day) = NaiveDate::parse_from_str(&review.date, "%Y-%m-%d") else {
                continue;
            };
            let index = day.weekday().num_days_from_monday() as usize;
            let used = review.spent_units
                / (review.available_minutes as f64 / 25.0 * energy_factor(review.energy));
            let target = match review.rating.as_str() {
                "overloaded" => Some(used * 0.80),
                "sustainable" => Some(used),
                "light" if review.spent_minutes >= review.available_minutes as f64 * 0.70 => {
                    Some(used * 1.12)
                }
                _ => None,
            };
            if let Some(target) = target.filter(|t| t.is_finite()) {
                let current = model.weekday_factors[index];
                model.weekday_factors[index] = (current * 0.80
                    + target.clamp(current * 0.75, current * 1.25) * 0.20)
                    .clamp(0.55, 1.45);
                model.day_samples += 1;
            }
        }
        model.stage = if !enabled {
            "Learning paused"
        } else if model.time_samples == 0 {
            "Starter estimates"
        } else if model.time_samples < 8 {
            "Early calibration"
        } else {
            "Personal calibration"
        }
        .into();
        model
    }
    pub fn minutes(&self, estimate: &Estimate, kind: &str) -> f64 {
        (estimate.minutes as f64
            * dot(&self.time_coefficients, &features(estimate, kind))
                .clamp(-1.1, 1.6)
                .exp())
        .clamp(5.0, 4800.0)
    }
    pub fn load(&self, estimate: &Estimate) -> f64 {
        dot(&self.load_weights, &estimate.factors()).clamp(0.5, 2.5)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub estimate: Estimate,
    pub predicted_minutes: f64,
    pub remaining_minutes: f64,
    pub units: f64,
    pub total_units: f64,
    pub low_minutes: f64,
    pub high_minutes: f64,
    pub load_factor: f64,
    pub deadline_pressure: f64,
    pub fit_score: f64,
    pub exceeds_estimate: bool,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanBlock {
    pub task_id: i64,
    pub minutes: f64,
    pub units: f64,
    pub partial: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityDay {
    pub date: String,
    pub weekday: usize,
    pub minutes: u32,
    pub energy: u8,
    pub capacity_units: f64,
    pub spent_minutes: f64,
    pub spent_units: f64,
    pub remaining_minutes: f64,
    pub remaining_units: f64,
    pub blocks: Vec<PlanBlock>,
    pub learned_factor: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workload {
    pub profile: CapacityProfile,
    pub calibration: Calibration,
    pub assessments: BTreeMap<i64, Assessment>,
    pub days: Vec<CapacityDay>,
    pub risk_ids: Vec<i64>,
    pub next_task_id: Option<i64>,
}
pub fn spent_on(
    tasks: &[Task],
    date: NaiveDate,
    model: &Calibration,
    now: DateTime<Utc>,
) -> (f64, f64) {
    let Some(start) = date
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(Local).earliest())
        .map(|d| d.with_timezone(&Utc))
    else {
        return (0.0, 0.0);
    };
    let Some(end) = (date + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(Local).earliest())
        .map(|d| d.with_timezone(&Utc))
    else {
        return (0.0, 0.0);
    };
    let mut minutes = 0.0;
    let mut units = 0.0;
    for task in tasks.iter().filter(|task| !task.deleted) {
        let estimate = Estimate::for_task(task);
        for entry in task.time_entries.iter().filter(|entry| entry.manual) {
            if DateTime::parse_from_rfc3339(&entry.ended_at)
                .ok()
                .is_some_and(|at| at.with_timezone(&Local).date_naive() == date)
            {
                let duration = entry.duration_ms as f64 / 60_000.0;
                minutes += duration;
                units += duration / 25.0 * model.load(&estimate);
            }
        }
        let mut intervals: Vec<_> = task
            .time_entries
            .iter()
            .filter(|entry| !entry.manual)
            .filter_map(|e| {
                Some((
                    DateTime::parse_from_rfc3339(&e.started_at)
                        .ok()?
                        .with_timezone(&Utc),
                    DateTime::parse_from_rfc3339(&e.ended_at)
                        .ok()?
                        .with_timezone(&Utc),
                ))
            })
            .collect();
        if let Some(running) = task
            .timer_started_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        {
            intervals.push((running.with_timezone(&Utc), now));
        }
        for (a, b) in intervals {
            let duration = (b.min(end).min(now) - a.max(start))
                .num_milliseconds()
                .max(0) as f64
                / 60_000.0;
            minutes += duration;
            units += duration / 25.0 * model.load(&estimate);
        }
    }
    (minutes, units)
}
fn due_pressure(task: &Task, date: NaiveDate) -> f64 {
    task.due_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|due| match (due - date).num_days() {
            ..=0 => 1.0,
            1 => 0.9,
            2..=3 => 0.7,
            4..=7 => 0.4,
            _ => 0.1,
        })
        .unwrap_or(0.0)
}
pub fn analyze(
    tasks: &[Task],
    profile: CapacityProfile,
    outcomes: &[Outcome],
    reviews: &[DayReview],
    checkins: &[Checkin],
    now: DateTime<Utc>,
) -> Workload {
    let model = Calibration::learn(outcomes, reviews, profile.learning_enabled, now);
    let today = now.with_timezone(&Local).date_naive();
    let mut assessments = BTreeMap::new();
    for task in tasks {
        let estimate = Estimate::for_task(task);
        let predicted = model.minutes(&estimate, &task.kind);
        let elapsed = task.elapsed_ms_at(now) as f64 / 60_000.0;
        let remaining = if task.status == Status::Done {
            0.0
        } else {
            (predicted - elapsed).max((predicted * 0.15).clamp(5.0, 30.0))
        };
        let load = model.load(&estimate);
        assessments.insert(
            task.id,
            Assessment {
                low_minutes: predicted * (-model.uncertainty).exp(),
                high_minutes: predicted * model.uncertainty.exp(),
                predicted_minutes: predicted,
                remaining_minutes: remaining,
                units: remaining / 25.0 * load,
                total_units: predicted / 25.0 * load,
                load_factor: load,
                deadline_pressure: due_pressure(task, today),
                fit_score: 0.0,
                exceeds_estimate: elapsed > predicted,
                reason: if estimate.source == "starter" {
                    "Starter estimate — review the scope before relying on it.".into()
                } else if model.time_samples == 0 && model.effort_samples == 0 {
                    "Initial assessment — calibration starts with your recorded work.".into()
                } else {
                    "Estimate adjusted using your recorded work.".into()
                },
                estimate,
            },
        );
    }
    let mut days = Vec::new();
    let mut remaining: BTreeMap<i64, f64> = tasks
        .iter()
        .filter(|t| {
            !t.deleted
                && t.organization.role == "task"
                && t.status != Status::Done
                && !t.labels.iter().any(|l| l == "blocked")
        })
        .map(|t| (t.id, assessments[&t.id].remaining_minutes))
        .collect();
    for offset in 0..7 {
        let date = today + Duration::days(offset);
        let weekday = date.weekday().num_days_from_monday() as usize;
        let key = date.format("%Y-%m-%d").to_string();
        let checkin = checkins.iter().find(|c| c.date == key);
        let minutes = checkin
            .map(|c| c.minutes)
            .unwrap_or(profile.weekday_minutes[weekday]);
        let energy = checkin.map(|c| c.energy).unwrap_or(3);
        let capacity =
            minutes as f64 / 25.0 * model.weekday_factors[weekday] * energy_factor(energy);
        let (spent_minutes, spent_units) = spent_on(tasks, date, &model, now);
        let budget_minutes = (minutes as f64 - spent_minutes).max(0.0);
        let budget_units = (capacity - spent_units).max(0.0);
        let mut day = CapacityDay {
            date: key,
            weekday,
            minutes,
            energy,
            capacity_units: capacity,
            spent_minutes,
            spent_units,
            remaining_minutes: budget_minutes,
            remaining_units: budget_units,
            blocks: vec![],
            learned_factor: model.weekday_factors[weekday],
        };
        let mut candidates: Vec<_> = tasks
            .iter()
            .filter(|t| remaining.get(&t.id).is_some_and(|m| *m > 0.5))
            .collect();
        candidates.sort_by(|a, b| {
            let score = |t: &Task| {
                let a = &assessments[&t.id];
                45.0 * due_pressure(t, date)
                    + 0.20 * t.priority as f64
                    + 20.0 / (1.0 + a.load_factor)
                    + if offset == 0 && t.timer_started_at.is_some() {
                        1000.0
                    } else {
                        0.0
                    }
            };
            score(b).total_cmp(&score(a)).then(a.id.cmp(&b.id))
        });
        let (mut time, mut units) = (budget_minutes, budget_units);
        for task in candidates {
            let a = &assessments[&task.id];
            let left = remaining[&task.id];
            // Low-energy days prefer lighter work, but still surface impending deadlines.
            if energy <= 2 && a.load_factor > 1.25 && due_pressure(task, date) < 0.9 {
                continue;
            }
            let allocation = left.min(time).min(units * 25.0 / a.load_factor);
            if allocation + 0.01 < left.min(25.0) {
                continue;
            }
            let allocation = if allocation >= left {
                left
            } else {
                (allocation / 5.0).floor() * 5.0
            };
            if allocation < 1.0 {
                continue;
            }
            let cost = allocation / 25.0 * a.load_factor;
            day.blocks.push(PlanBlock {
                task_id: task.id,
                minutes: allocation,
                units: cost,
                partial: allocation + 0.5 < left,
            });
            time = (time - allocation).max(0.0);
            units = (units - cost).max(0.0);
            remaining.insert(task.id, (left - allocation).max(0.0));
        }
        days.push(day);
    }
    let current = &days[0];
    for task in tasks {
        let a = assessments.get_mut(&task.id).unwrap();
        let fit = (current.remaining_units / a.units.max(0.1)).min(1.0);
        a.fit_score = (a.deadline_pressure * 40.0
            + task.priority as f64 * 0.15
            + fit * 35.0
            + 10.0 / a.load_factor)
            .clamp(0.0, 100.0);
        let mut reason = if let Some(block) = current.blocks.iter().find(|b| b.task_id == task.id) {
            format!(
                "A {:.0}-minute {}fits today’s capacity.",
                block.minutes,
                if block.partial {
                    "work block "
                } else {
                    "task "
                }
            )
        } else if current.remaining_minutes < 5.0 || current.remaining_units < 0.2 {
            "Your planned capacity is used. Rest or explicitly adjust today’s budget.".into()
        } else {
            "Doesn’t fit today’s remaining time and load budget; consider a smaller block.".into()
        };
        if a.deadline_pressure >= 0.9 {
            reason.push_str(" Deadline needs attention.");
        }
        if a.estimate.source == "starter" {
            reason.push_str(" Scope estimate is still provisional.");
        }
        if a.exceeds_estimate {
            reason.push_str(" Tracked time exceeded the estimate; update the remaining scope.");
        }
        a.reason = reason;
    }
    let risk_ids = tasks
        .iter()
        .filter(|t| {
            if t.organization.role != "task"
                || t.deleted
                || t.status == Status::Done
                || !t
                    .due_date
                    .as_deref()
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .is_some_and(|d| (d - today).num_days() <= 6)
            {
                return false;
            }
            let scheduled = days
                .iter()
                .filter(|d| t.due_date.as_ref().is_some_and(|due| &d.date <= due))
                .flat_map(|d| &d.blocks)
                .filter(|b| b.task_id == t.id)
                .map(|b| b.minutes)
                .sum::<f64>();
            scheduled + 0.5 < assessments[&t.id].remaining_minutes
        })
        .map(|t| t.id)
        .collect();
    let next_task_id = current
        .blocks
        .iter()
        .find(|b| {
            tasks
                .iter()
                .find(|t| t.id == b.task_id)
                .is_some_and(|t| t.timer_started_at.is_none())
        })
        .map(|b| b.task_id);
    Workload {
        profile,
        calibration: model,
        assessments,
        days,
        risk_ids,
        next_task_id,
    }
}
