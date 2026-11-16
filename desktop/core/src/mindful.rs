//! Mindful break reminders while focus timers run: drink water, rest your eyes,
//! take a small brain break. Pure scheduling logic; the app shows the overlay.
//!
//! Only time with at least one running timer counts. A long pause between timers
//! counts as rest, so eye and brain-break reminders start over after it.
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reminder {
    Water,
    Eyes,
    Stretch,
}
impl Reminder {
    pub const ALL: [Reminder; 3] = [Reminder::Stretch, Reminder::Eyes, Reminder::Water];
    fn index(self) -> usize {
        match self {
            Reminder::Water => 0,
            Reminder::Eyes => 1,
            Reminder::Stretch => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    /// The person did it.
    Done,
    /// Ask again in a few minutes.
    Snooze,
    /// Not now; wait a full interval.
    Skip,
    /// The overlay closed on its own; treated like Skip so it never nags.
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MindfulSettings {
    pub enabled: bool,
    /// Minutes of focus between reminders. 0 turns a reminder off.
    pub water_minutes: u32,
    pub eyes_minutes: u32,
    pub stretch_minutes: u32,
    /// Seconds the overlay stays before closing itself.
    #[serde(default = "default_seconds")]
    pub display_seconds: u32,
}
fn default_seconds() -> u32 {
    25
}
impl Default for MindfulSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            water_minutes: 45,
            eyes_minutes: 20,
            stretch_minutes: 60,
            display_seconds: default_seconds(),
        }
    }
}
impl MindfulSettings {
    pub fn validate(&self) -> Result<(), String> {
        for m in [self.water_minutes, self.eyes_minutes, self.stretch_minutes] {
            if m != 0 && !(5..=240).contains(&m) {
                return Err("Choose reminder intervals between 5 minutes and 4 hours, or turn them off.".into());
            }
        }
        if !(5..=120).contains(&self.display_seconds) {
            return Err("Show reminders for 5–120 seconds.".into());
        }
        Ok(())
    }
    pub fn interval(&self, r: Reminder) -> Option<Duration> {
        let m = match r {
            Reminder::Water => self.water_minutes,
            Reminder::Eyes => self.eyes_minutes,
            Reminder::Stretch => self.stretch_minutes,
        };
        (m > 0).then(|| Duration::minutes(i64::from(m)))
    }
}

/// A pause this long without any running timer counts as rest.
pub const REST: Duration = Duration::minutes(5);
/// Snoozed reminders come back after this long.
pub const SNOOZE: Duration = Duration::minutes(10);
/// Never show two reminders closer together than this.
pub const GAP: Duration = Duration::minutes(2);
/// Longer gaps between ticks (sleep, suspended app) are not counted as focus.
const MAX_STEP: Duration = Duration::minutes(2);

#[derive(Debug, Clone, Default)]
pub struct Scheduler {
    focus: [Duration; 3],
    idle: Duration,
    last_tick: Option<DateTime<Utc>>,
    last_shown: Option<DateTime<Utc>>,
    snoozed_until: [Option<DateTime<Utc>>; 3],
    showing: Option<Reminder>,
    quiet_until: Option<DateTime<Utc>>,
}

impl Scheduler {
    /// Advances the clock and returns a reminder to show now, if one is due.
    pub fn tick(
        &mut self,
        now: DateTime<Utc>,
        running: bool,
        settings: &MindfulSettings,
    ) -> Option<Reminder> {
        let step = self
            .last_tick
            .map(|last| (now - last).clamp(Duration::zero(), MAX_STEP))
            .unwrap_or_else(Duration::zero);
        self.last_tick = Some(now);
        if running {
            self.idle = Duration::zero();
            for f in &mut self.focus {
                *f += step;
            }
        } else {
            self.idle += step;
            if self.idle >= REST {
                // A real pause: eyes and body are rested. Water still adds up.
                self.focus[Reminder::Eyes.index()] = Duration::zero();
                self.focus[Reminder::Stretch.index()] = Duration::zero();
            }
        }
        if !settings.enabled
            || !running
            || self.showing.is_some()
            || self.quiet_until.is_some_and(|q| now < q)
            || self.last_shown.is_some_and(|s| now - s < GAP)
        {
            return None;
        }
        let due = Reminder::ALL
            .into_iter()
            .filter(|r| {
                settings
                    .interval(*r)
                    .is_some_and(|i| self.focus[r.index()] >= i)
                    && self.snoozed_until[r.index()].is_none_or(|s| now >= s)
            })
            // Most overdue relative to its interval goes first.
            .max_by(|a, b| {
                let ratio = |r: &Reminder| {
                    self.focus[r.index()].num_seconds() as f64
                        / settings.interval(*r).unwrap().num_seconds().max(1) as f64
                };
                ratio(a).total_cmp(&ratio(b))
            })?;
        self.showing = Some(due);
        self.last_shown = Some(now);
        Some(due)
    }

    pub fn showing(&self) -> Option<Reminder> {
        self.showing
    }

    pub fn respond(&mut self, reminder: Reminder, response: Response, now: DateTime<Utc>) {
        if self.showing == Some(reminder) {
            self.showing = None;
        }
        self.last_shown = Some(now);
        let i = reminder.index();
        match response {
            Response::Snooze => self.snoozed_until[i] = Some(now + SNOOZE),
            Response::Done | Response::Skip | Response::Timeout => {
                self.focus[i] = Duration::zero();
                self.snoozed_until[i] = None;
                // Stepping away for a brain break also rests the eyes.
                if reminder == Reminder::Stretch && response == Response::Done {
                    self.focus[Reminder::Eyes.index()] = Duration::zero();
                }
            }
        }
    }

    /// Pauses all reminders until the given time.
    pub fn quiet(&mut self, until: Option<DateTime<Utc>>) {
        self.quiet_until = until;
        self.showing = None;
    }
    pub fn quiet_until(&self) -> Option<DateTime<Utc>> {
        self.quiet_until
    }

    /// Focus time counted toward a reminder, for display and tests.
    pub fn focus(&self, reminder: Reminder) -> Duration {
        self.focus[reminder.index()]
    }
}
