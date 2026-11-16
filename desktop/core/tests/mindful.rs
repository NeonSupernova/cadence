use chrono::{DateTime, Duration, Utc};
use jev_tasks_core::mindful::*;

fn settings() -> MindfulSettings {
    MindfulSettings::default() // water 45, eyes 20, stretch 60
}
/// Ticks every 30 seconds for `minutes`, returning every reminder shown.
fn run(s: &mut Scheduler, now: &mut DateTime<Utc>, minutes: i64, running: bool, cfg: &MindfulSettings) -> Vec<(i64, Reminder)> {
    let start = *now;
    let mut shown = vec![];
    for _ in 0..minutes * 2 {
        *now += Duration::seconds(30);
        if let Some(r) = s.tick(*now, running, cfg) {
            shown.push(((*now - start).num_minutes(), r));
        }
    }
    shown
}

#[test]
fn reminders_follow_focus_time_and_wait_for_a_response() {
    let mut s = Scheduler::default();
    let mut now = Utc::now();
    s.tick(now, true, &settings());
    let shown = run(&mut s, &mut now, 30, true, &settings());
    assert_eq!(shown, vec![(20, Reminder::Eyes)]);
    // Nothing else appears while the overlay is still up.
    assert!(run(&mut s, &mut now, 30, true, &settings()).is_empty());
    s.respond(Reminder::Eyes, Response::Done, now);
    assert_eq!(s.focus(Reminder::Eyes), Duration::zero());
    // Water (45m) was overdue while eyes was showing; it comes after the 2 minute gap.
    let shown = run(&mut s, &mut now, 3, true, &settings());
    assert_eq!(shown, vec![(2, Reminder::Water)]);
}

#[test]
fn no_reminders_without_a_running_timer_and_rest_resets_eyes() {
    let mut s = Scheduler::default();
    let mut now = Utc::now();
    s.tick(now, true, &settings());
    run(&mut s, &mut now, 15, true, &settings());
    assert!(run(&mut s, &mut now, 60, false, &settings()).is_empty());
    assert_eq!(s.focus(Reminder::Eyes), Duration::zero());
    assert_eq!(s.focus(Reminder::Water), Duration::minutes(15));
    // A short pause does not reset anything.
    let mut s = Scheduler::default();
    s.tick(now, true, &settings());
    run(&mut s, &mut now, 15, true, &settings());
    run(&mut s, &mut now, 3, false, &settings());
    assert_eq!(s.focus(Reminder::Eyes), Duration::minutes(15));
}

#[test]
fn snooze_skip_quiet_and_disabled() {
    let mut s = Scheduler::default();
    let mut now = Utc::now();
    s.tick(now, true, &settings());
    assert_eq!(run(&mut s, &mut now, 20, true, &settings()), vec![(20, Reminder::Eyes)]);
    s.respond(Reminder::Eyes, Response::Snooze, now);
    let shown = run(&mut s, &mut now, 11, true, &settings());
    assert_eq!(shown, vec![(10, Reminder::Eyes)]);
    s.respond(Reminder::Eyes, Response::Timeout, now);
    assert_eq!(s.focus(Reminder::Eyes), Duration::zero());

    s.quiet(Some(now + Duration::hours(1)));
    assert!(run(&mut s, &mut now, 59, true, &settings()).is_empty());
    assert!(!run(&mut s, &mut now, 2, true, &settings()).is_empty());

    let off = MindfulSettings { enabled: false, ..settings() };
    let mut s = Scheduler::default();
    s.tick(now, true, &off);
    assert!(run(&mut s, &mut now, 120, true, &off).is_empty());
    let only_water = MindfulSettings { eyes_minutes: 0, stretch_minutes: 0, ..settings() };
    let mut s = Scheduler::default();
    s.tick(now, true, &only_water);
    assert_eq!(run(&mut s, &mut now, 50, true, &only_water), vec![(45, Reminder::Water)]);
}

#[test]
fn brain_break_done_rests_eyes_and_sleep_gaps_do_not_count() {
    let cfg = MindfulSettings { eyes_minutes: 0, ..settings() };
    let mut s = Scheduler::default();
    let mut now = Utc::now();
    s.tick(now, true, &cfg);
    let shown = run(&mut s, &mut now, 61, true, &cfg);
    assert_eq!(shown, vec![(45, Reminder::Water)]);
    s.respond(Reminder::Water, Response::Done, now);
    let shown = run(&mut s, &mut now, 3, true, &cfg);
    assert_eq!(shown[0].1, Reminder::Stretch);
    s.respond(Reminder::Stretch, Response::Done, now);
    assert_eq!(s.focus(Reminder::Eyes), Duration::zero());
    // The laptop slept for 3 hours with a timer running: at most 2 minutes count.
    let before = s.focus(Reminder::Water);
    now += Duration::hours(3);
    s.tick(now, true, &cfg);
    assert_eq!(s.focus(Reminder::Water) - before, Duration::minutes(2));
}

#[test]
fn settings_are_validated() {
    assert!(settings().validate().is_ok());
    assert!(MindfulSettings { water_minutes: 3, ..settings() }.validate().is_err());
    assert!(MindfulSettings { eyes_minutes: 241, ..settings() }.validate().is_err());
    assert!(MindfulSettings { water_minutes: 0, ..settings() }.validate().is_ok());
    assert!(MindfulSettings { display_seconds: 2, ..settings() }.validate().is_err());
}
