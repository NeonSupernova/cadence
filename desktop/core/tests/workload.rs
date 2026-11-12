use chrono::{Datelike, Duration, Local, Utc};
use jev_tasks_core::{workload::*, *};
use std::path::Path;

fn now() -> chrono::DateTime<Utc> {
    Local::now()
        .date_naive()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .unwrap()
        .with_timezone(&Utc)
}
fn task(id: i64) -> Task {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    let mut t = s.tasks().unwrap()[2].clone();
    t.id = id;
    t.kind = "chore".into();
    t.labels = vec![];
    t.due_date = None;
    t.status = Status::Open;
    t.tracked_ms = 0;
    t.timer_started_at = None;
    t.time_entries.clear();
    t.estimate = Some(estimate());
    t
}
fn estimate() -> Estimate {
    Estimate {
        minutes: 60,
        technical: 3,
        tedium: 3,
        ambiguity: 3,
        switching: 3,
        source: "user".into(),
    }
}
fn outcome(id: i64, actual: f64, rating: Option<u8>) -> Outcome {
    Outcome {
        task_id: id,
        kind: "chore".into(),
        estimate: estimate(),
        actual_minutes: actual,
        effort_rating: rating,
        time_reliable: true,
        recorded_at: now().to_rfc3339(),
    }
}
fn analyze_tasks(tasks: &[Task]) -> Workload {
    analyze(tasks, CapacityProfile::default(), &[], &[], &[], now())
}
fn plan(actions: Vec<Action>) -> Plan {
    Plan {
        actions,
        ..Default::default()
    }
}

#[test]
fn starts_with_the_users_schedule_and_never_claims_to_be_personalized_without_data() {
    let w = analyze_tasks(&[task(1)]);
    assert_eq!(w.profile.weekday_minutes, [120, 120, 120, 120, 120, 60, 60]);
    assert_eq!(w.calibration.time_samples, 0);
    assert_eq!(w.calibration.stage, "Starter estimates");
    assert_eq!(w.calibration.weekday_factors, [1.0; 7]);
    assert!(w.assessments[&1].high_minutes > w.assessments[&1].predicted_minutes * 1.5);
}

#[test]
fn deadlines_change_urgency_but_do_not_reduce_work_cost() {
    let mut t = task(1);
    let original = analyze_tasks(&[t.clone()]);
    t.due_date = Some(now().with_timezone(&Local).format("%Y-%m-%d").to_string());
    let urgent = analyze_tasks(&[t]);
    assert_eq!(original.assessments[&1].units, urgent.assessments[&1].units);
    assert_eq!(
        original.assessments[&1].predicted_minutes,
        urgent.assessments[&1].predicted_minutes
    );
    assert!(urgent.assessments[&1].deadline_pressure > original.assessments[&1].deadline_pressure);
}

#[test]
fn time_calibration_learns_consistent_underestimation_and_replay_is_idempotent() {
    let observations: Vec<_> = (0..20).map(|id| outcome(id, 120.0, None)).collect();
    let m = Calibration::learn(&observations, &[], true, now());
    assert!(m.minutes(&estimate(), "chore") > 100.0);
    assert!(m.minutes(&estimate(), "chore") < 140.0);
    assert_eq!(m.time_samples, 20);
    assert_eq!(
        m.time_coefficients,
        Calibration::learn(&observations, &[], true, now()).time_coefficients
    );
    let paused = Calibration::learn(&observations, &[], false, now());
    assert_eq!(m.time_coefficients, paused.time_coefficients);
    assert_eq!(paused.stage, "Learning paused");
}

#[test]
fn effort_feedback_can_learn_that_tedium_costs_more_than_technical_difficulty() {
    let observations: Vec<_> = (0..30)
        .map(|id| {
            let tedious = id % 2 == 0;
            let mut o = outcome(id, 60.0, Some(if tedious { 5 } else { 2 }));
            o.estimate.tedium = if tedious { 5 } else { 1 };
            o.estimate.technical = if tedious { 2 } else { 5 };
            o
        })
        .collect();
    let m = Calibration::learn(&observations, &[], true, now());
    assert!(m.load_weights[2] > m.load_weights[1]);
    let mut boring = estimate();
    boring.tedium = 5;
    boring.technical = 2;
    let mut technical = estimate();
    technical.tedium = 1;
    technical.technical = 5;
    assert!(m.load(&boring) > m.load(&technical));
}

#[test]
fn seven_day_plan_respects_both_time_and_load_budgets_and_splits_long_tasks() {
    let mut t = task(1);
    t.estimate.as_mut().unwrap().minutes = 600;
    let w = analyze_tasks(&[t]);
    for day in &w.days {
        assert!(day.blocks.iter().map(|b| b.minutes).sum::<f64>() <= day.remaining_minutes + 0.001);
        assert!(day.blocks.iter().map(|b| b.units).sum::<f64>() <= day.remaining_units + 0.001);
    }
    assert!(w.days[0].blocks[0].partial);
    assert!(
        w.days
            .iter()
            .flat_map(|d| &d.blocks)
            .map(|b| b.minutes)
            .sum::<f64>()
            <= 600.001
    );
}

#[test]
fn low_energy_can_defer_heavy_work_and_zero_capacity_never_manufactures_time() {
    let mut heavy = task(1);
    heavy.estimate = Some(Estimate {
        minutes: 60,
        technical: 5,
        tedium: 5,
        ambiguity: 5,
        switching: 5,
        source: "user".into(),
    });
    let checkin = Checkin {
        date: now().with_timezone(&Local).format("%Y-%m-%d").to_string(),
        minutes: 120,
        energy: 1,
    };
    let w = analyze(
        &[heavy.clone()],
        CapacityProfile::default(),
        &[],
        &[],
        &[checkin],
        now(),
    );
    assert!(w.days[0].blocks.is_empty());
    heavy.due_date = Some(now().with_timezone(&Local).format("%Y-%m-%d").to_string());
    let profile = CapacityProfile {
        weekday_minutes: [0; 7],
        learning_enabled: true,
    };
    let empty = analyze(&[heavy], profile, &[], &[], &[], now());
    assert!(empty.days.iter().all(|d| d.blocks.is_empty()));
    assert_eq!(empty.next_task_id, None);
    assert_eq!(empty.risk_ids, vec![1]);
}

#[test]
fn untracked_days_do_not_reduce_capacity_but_explicit_overload_reviews_do() {
    let index = now().with_timezone(&Local).weekday().num_days_from_monday() as usize;
    let reviews = vec![DayReview {
        date: now().with_timezone(&Local).format("%Y-%m-%d").to_string(),
        available_minutes: 120,
        energy: 3,
        spent_minutes: 120.0,
        spent_units: 4.8,
        rating: "overloaded".into(),
    }];
    let m = Calibration::learn(&[], &reviews, true, now());
    assert!(m.weekday_factors[index] < 1.0);
    assert!(m
        .weekday_factors
        .iter()
        .enumerate()
        .all(|(i, v)| i == index || *v == 1.0));
    assert_eq!(
        Calibration::learn(&[], &[], true, now()).weekday_factors,
        [1.0; 7]
    );
}

#[test]
fn blocked_done_and_trashed_tasks_are_not_scheduled_and_missed_deadlines_remain_visible() {
    let mut blocked = task(1);
    blocked.labels.push("blocked".into());
    blocked.due_date = Some(now().with_timezone(&Local).format("%Y-%m-%d").to_string());
    let mut done = task(2);
    done.status = Status::Done;
    let mut deleted = task(3);
    deleted.deleted = true;
    let w = analyze_tasks(&[blocked, done, deleted]);
    assert!(w.days.iter().all(|d| d.blocks.is_empty()));
    assert_eq!(w.risk_ids, vec![1]);
}

#[test]
fn unreliable_long_timers_can_train_effort_without_polluting_time_predictions() {
    let mut t = task(1);
    t.status = Status::Done;
    t.tracked_ms = 8 * 60 * 60 * 1000;
    t.time_entries = vec![TimeEntry {
        started_at: (now() - Duration::hours(8)).to_rfc3339(),
        ended_at: now().to_rfc3339(),
        duration_ms: 8 * 60 * 60 * 1000,
        manual: false,
    }];
    let o = feedback_outcome(
        &t,
        &Feedback {
            task_id: 1,
            rating: 5,
            actual_minutes: None,
        },
    )
    .unwrap();
    assert!(!o.time_reliable);
    let m = Calibration::learn(&[o], &[], true, now());
    assert_eq!(m.time_samples, 0);
    assert_eq!(m.effort_samples, 1);
    let corrected = feedback_outcome(
        &t,
        &Feedback {
            task_id: 1,
            rating: 5,
            actual_minutes: Some(90.0),
        },
    )
    .unwrap();
    assert!(corrected.time_reliable);
    assert_eq!(
        Calibration::learn(&[corrected], &[], true, now()).time_samples,
        1
    );
}

#[test]
fn capacity_changes_and_feedback_are_transactional_and_persist() {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    s.seed_demo().unwrap();
    let date = Local::now().format("%Y-%m-%d").to_string();
    let bad = Plan {
        actions: vec![Action::status("complete", 999)],
        checkin: Some(Checkin {
            date: date.clone(),
            minutes: 45,
            energy: 1,
        }),
        ..Default::default()
    };
    assert!(s.apply(bad, "", None, false).is_err());
    let original = s.workload(&s.tasks().unwrap()).unwrap().days[0].minutes;
    s.apply(
        Plan {
            checkin: Some(Checkin {
                date,
                minutes: 45,
                energy: 1,
            }),
            ..Default::default()
        },
        "",
        None,
        false,
    )
    .unwrap();
    assert_eq!(s.workload(&s.tasks().unwrap()).unwrap().days[0].minutes, 45);
    assert_ne!(original, 45);
    s.save_estimate(
        6,
        Estimate {
            minutes: 20,
            ..estimate()
        },
    )
    .unwrap();
    s.save_feedback(6, 4, Some(30.0)).unwrap();
    s.save_feedback(6, 4, Some(30.0)).unwrap();
    assert_eq!(
        s.workload(&s.tasks().unwrap())
            .unwrap()
            .calibration
            .time_samples,
        1
    );
    s.apply(plan(vec![Action::status("reopen", 6)]), "", None, false)
        .unwrap();
    assert_eq!(
        s.workload(&s.tasks().unwrap())
            .unwrap()
            .calibration
            .time_samples,
        0
    );
}

#[test]
fn completed_tasks_preserve_total_cost_without_reserving_more_capacity() {
    let mut t = task(1);
    let open = analyze_tasks(&[t.clone()]);
    t.status = Status::Done;
    let done = analyze_tasks(&[t]);
    assert_eq!(done.assessments[&1].remaining_minutes, 0.0);
    assert_eq!(done.assessments[&1].units, 0.0);
    assert_eq!(
        done.assessments[&1].total_units,
        open.assessments[&1].total_units
    );
    assert!(done.days.iter().all(|d| d.blocks.is_empty()));
}
