// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The baton scheduler, in one process: tasks run one at a time in an
//! order drawn from the seed; grants, wakes and panics never lose the
//! baton; time moves by step cost and jumps; fairness, the budget and the
//! live-task cap hold.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ironafs_time::{AwakeDeadline, BootDeadline};

use super::{Config, Failure, Outcome, SchedPlants, Setup, SimDeadline, TaskCx, Wait, Waited, run};
use crate::trace::{Entry, How, TaskId, chosen_identities, decisions, terminal};

fn boot_in(cx: &TaskCx, after: Duration) -> SimDeadline {
    let now = cx.clock().boot().unwrap();
    SimDeadline::Boot(BootDeadline::after(now, after).unwrap())
}

/// Yields once, to a deadline that has already passed: the task is
/// enabled again at the next decision.
fn step(cx: &TaskCx) {
    let waited = cx.wait(Wait {
        signal: None,
        cancel: None,
        deadline: boot_in(cx, Duration::ZERO),
    });
    assert_eq!(waited, Waited::DeadlinePassed);
}

/// Three tasks of three steps each, every step an operation event.
fn three_by_three(setup: &mut Setup) {
    for name in ["a", "b", "c"] {
        setup.task("worker", move |cx: &TaskCx| {
            for round in 0..3_u64 {
                cx.op("work", name, Some(round));
                if round < 2 {
                    step(cx);
                }
            }
        });
    }
}

fn run_with(config: &Config, scenario: impl FnOnce(&mut Setup)) -> Outcome {
    run(config, None, None, scenario)
}

#[test]
fn one_task_runs_to_its_end() {
    let outcome = run_with(&Config::new("one", 5), |setup| {
        setup.task("only", |cx: &TaskCx| cx.op("hello", "world", None));
    });
    assert_eq!(outcome.failure, None);
    assert_eq!(
        outcome.trace,
        "ironafs-sim trace 2\n\
         d0 sched run one seed=5 incarnation=0 plants=-\n\
         d0 sched spawn t0 only\n\
         d0 sched decide [t0] pick=0 drawn draws=1 at="
            .to_owned()
            + &decisions(&outcome.trace)[0].at.to_string()
            + "\nd0 t0 op hello world\n\
               d0 t0 end returned\n\
               d1 sched end ok\n"
    );
}

#[test]
fn same_seed_gives_the_same_trace_and_seeds_explore_schedules() {
    let first = run_with(&Config::new("three", 1), three_by_three);
    let second = run_with(&Config::new("three", 1), three_by_three);
    assert_eq!(first.failure, None);
    assert_eq!(first.trace, second.trace);
    let schedules: std::collections::BTreeSet<Vec<Entry>> = (0..16)
        .map(|seed| chosen_identities(&run_with(&Config::new("three", seed), three_by_three).trace))
        .collect();
    assert!(schedules.len() > 8, "only {} schedules", schedules.len());
}

#[test]
fn grant_posted_before_the_thread_exists_is_kept() {
    let plain = run_with(&Config::new("three", 3), three_by_three);
    let mut config = Config::new("three", 3);
    config.plants = SchedPlants {
        grant_before_spawn: true,
        ..SchedPlants::default()
    };
    let planted = run_with(&config, three_by_three);
    assert_eq!(planted.failure, None);
    assert_eq!(planted.trace, plain.trace);
}

#[test]
fn spurious_wakes_change_nothing() {
    let plain = run_with(&Config::new("three", 4), three_by_three);
    let mut config = Config::new("three", 4);
    config.plants.spurious_wakes = true;
    let planted = run_with(&config, three_by_three);
    assert_eq!(planted.failure, None);
    assert_eq!(planted.trace, plain.trace);
}

#[test]
fn signal_raised_before_the_wait_is_not_lost() {
    for seed in 0..8 {
        let cell = Arc::new(Mutex::new(None));
        let raiser = Arc::clone(&cell);
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&outcomes);
        let outcome = run_with(&Config::new("signal", seed), move |setup| {
            setup.task("raiser", move |cx: &TaskCx| {
                let signal = cx.signal();
                cx.raise(signal);
                *raiser.lock().unwrap() = Some(signal);
            });
            setup.task("waiter", move |cx: &TaskCx| {
                let signal = loop {
                    if let Some(signal) = *cell.lock().unwrap() {
                        break signal;
                    }
                    step(cx);
                };
                let waited = cx.wait(Wait {
                    signal: Some(signal),
                    cancel: None,
                    deadline: boot_in(cx, Duration::from_secs(3_600)),
                });
                record.lock().unwrap().push(waited);
            });
        });
        assert_eq!(outcome.failure, None, "seed {seed}");
        assert_eq!(
            *outcomes.lock().unwrap(),
            vec![Waited::Signalled],
            "seed {seed}"
        );
    }
}

#[test]
fn cancel_takes_precedence_over_a_raised_signal_and_a_passed_deadline() {
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&outcomes);
    let outcome = run_with(&Config::new("precedence", 1), move |setup| {
        setup.task("waiter", move |cx: &TaskCx| {
            let signal = cx.signal();
            let cancel = cx.signal();
            cx.raise(signal);
            let both = |cx: &TaskCx| Wait {
                signal: Some(signal),
                cancel: Some(cancel),
                deadline: boot_in(cx, Duration::ZERO),
            };
            let mut seen = vec![cx.wait(Wait {
                deadline: boot_in(cx, Duration::from_secs(1)),
                ..both(cx)
            })];
            seen.push(cx.wait(both(cx)));
            cx.raise(cancel);
            seen.push(cx.wait(both(cx)));
            *record.lock().unwrap() = seen;
        });
    });
    assert_eq!(outcome.failure, None);
    assert_eq!(
        *outcomes.lock().unwrap(),
        vec![Waited::Signalled, Waited::DeadlinePassed, Waited::Cancelled]
    );
}

#[test]
fn a_task_panic_fails_the_run_and_returns_the_baton() {
    let mut config = Config::new("panic", 2);
    config.watchdog = Duration::from_secs(20);
    let outcome = run_with(&config, |setup| {
        setup.task("panics", |_: &TaskCx| {
            std::panic::panic_any("planted panic");
        });
        setup.task("sleeper", |cx: &TaskCx| {
            let _ = cx.wait(Wait {
                signal: None,
                cancel: None,
                deadline: boot_in(cx, Duration::from_secs(3_600)),
            });
        });
    });
    assert_eq!(
        outcome.failure,
        Some(Failure::TaskPanicked {
            task: TaskId(0),
            message: "planted panic".to_owned(),
        })
    );
    assert_eq!(
        terminal(&outcome.trace),
        Some("end failed task-panicked:t0")
    );
    assert!(outcome.trace.contains(" t0 end panicked\n"));
}

#[test]
fn endless_yielding_reaches_the_budget() {
    let mut config = Config::new("loop", 1);
    config.budget = 50;
    let outcome = run_with(&config, |setup| {
        setup.task("spinner", |cx: &TaskCx| {
            loop {
                step(cx);
            }
        });
    });
    assert_eq!(outcome.failure, Some(Failure::Budget));
    assert_eq!(decisions(&outcome.trace).len(), 50);
    assert_eq!(terminal(&outcome.trace), Some("end failed budget"));
}

#[test]
fn time_jumps_to_the_earliest_deadline_and_steps_cost_time() {
    let times = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&times);
    let mut config = Config::new("jump", 9);
    config.step_cost = Duration::from_micros(10);
    let outcome = run_with(&config, move |setup| {
        setup.task("sleeper", move |cx: &TaskCx| {
            let start = cx.clock().awake().unwrap();
            let deadline = AwakeDeadline::after(start, Duration::from_millis(5)).unwrap();
            let waited = cx.wait(Wait {
                signal: None,
                cancel: None,
                deadline: SimDeadline::Awake(deadline),
            });
            let end = cx.clock().awake().unwrap();
            record
                .lock()
                .unwrap()
                .push((waited, end.checked_duration_since(start).unwrap()));
        });
    });
    assert_eq!(outcome.failure, None);
    // The first step cost 10 us; the jump covers the rest of the 5 ms.
    assert!(
        outcome.trace.contains(" sched time +4990000\n"),
        "{}",
        outcome.trace
    );
    assert_eq!(
        *times.lock().unwrap(),
        vec![(Waited::DeadlinePassed, Duration::from_millis(5))]
    );
}

#[test]
fn an_overdue_entry_runs_without_a_draw_within_k_plus_m() {
    let mut config = Config::new("fair", 6);
    config.fairness = 3;
    config.plants.segregate = true;
    let outcome = run_with(&config, three_by_three);
    assert_eq!(outcome.failure, None);
    let all = decisions(&outcome.trace);
    assert!(all.iter().any(|decision| decision.how == How::Overdue));
    // Every enabled entry runs within k + m counted decisions (5.6,
    // point 6): here k = 3 and m = 3.
    let mut waiting: std::collections::BTreeMap<Entry, u64> = std::collections::BTreeMap::new();
    for decision in &all {
        let chosen = decision.chosen().unwrap();
        for entry in &decision.enabled {
            if *entry == chosen {
                waiting.remove(entry);
            } else {
                let count = waiting.entry(*entry).or_default();
                *count += 1;
                assert!(*count < 6, "{entry} waited {count} decisions");
            }
        }
        waiting.retain(|entry, _| decision.enabled.contains(entry));
    }
}

#[test]
fn more_tasks_than_the_cap_fail_the_run() {
    let mut config = Config::new("cap", 1);
    config.live_cap = 2;
    let outcome = run_with(&config, three_by_three);
    assert_eq!(outcome.failure, Some(Failure::TooManyTasks));
    assert_eq!(terminal(&outcome.trace), Some("end failed too-many-tasks"));
}

#[test]
fn a_call_gets_the_server_reply_through_a_world_event() {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&replies);
    let outcome = run_with(&Config::new("call", 3), move |setup| {
        setup.server(|request: &[u8]| request.iter().rev().copied().collect());
        setup.task("caller", move |cx: &TaskCx| {
            let reply = cx.call(&[1, 2, 0xab], boot_in(cx, Duration::from_secs(1)));
            record.lock().unwrap().push(reply);
        });
    });
    assert_eq!(outcome.failure, None);
    assert_eq!(*replies.lock().unwrap(), vec![Ok(vec![0xab, 2, 1])]);
    assert!(outcome.trace.contains(" t0 call c0 0102ab\n"));
    assert!(outcome.trace.contains(" world reply c0 ab0201\n"));
    assert!(outcome.trace.contains("[delivery#0]"));
}

#[test]
fn a_call_whose_deadline_passes_first_gets_no_reply() {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&replies);
    let mut config = Config::new("late", 3);
    config.latency = (10_000_000, 10_000_000);
    let outcome = run_with(&config, move |setup| {
        setup.server(|request: &[u8]| request.to_vec());
        setup.task("caller", move |cx: &TaskCx| {
            let reply = cx.call(&[7], boot_in(cx, Duration::from_millis(1)));
            record.lock().unwrap().push(reply);
        });
    });
    assert_eq!(outcome.failure, None);
    assert_eq!(*replies.lock().unwrap(), vec![Err(Waited::DeadlinePassed)]);
    assert!(!outcome.trace.contains(" world reply "));
}

#[test]
fn a_dropped_reply_ends_the_call_at_its_deadline() {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&replies);
    let outcome = run_with(&Config::new("drop", 3), move |setup| {
        setup.server_may_drop(|request: &[u8]| (request != [9]).then(|| request.to_vec()));
        setup.task("caller", move |cx: &TaskCx| {
            for request in [[9], [7]] {
                let reply = cx.call(&request, boot_in(cx, Duration::from_millis(50)));
                record.lock().unwrap().push(reply);
            }
        });
    });
    assert_eq!(outcome.failure, None);
    assert_eq!(
        *replies.lock().unwrap(),
        vec![Err(Waited::DeadlinePassed), Ok(vec![7])]
    );
    assert!(outcome.trace.contains(" world drop c0\n"));
    assert!(!outcome.trace.contains(" world reply c0 "));
    assert!(outcome.trace.contains(" world reply c1 07\n"));
}

#[test]
fn a_broken_invariant_fails_the_run_at_its_step() {
    let counter = Arc::new(Mutex::new(0_u32));
    let bump = Arc::clone(&counter);
    let outcome = run_with(&Config::new("check", 1), move |setup| {
        setup.task("bumper", move |cx: &TaskCx| {
            for _ in 0..5 {
                *bump.lock().unwrap() += 1;
                step(cx);
            }
        });
        setup.check_each_step(move || {
            if *counter.lock().unwrap() < 3 {
                Ok(())
            } else {
                Err("counter-below-three")
            }
        });
    });
    assert_eq!(
        outcome.failure,
        Some(Failure::Invariant("counter-below-three"))
    );
    assert_eq!(
        terminal(&outcome.trace),
        Some("end failed invariant:counter-below-three")
    );
    assert_eq!(decisions(&outcome.trace).len(), 3);
}

#[test]
fn an_operation_outside_the_alphabet_is_a_misuse() {
    let outcome = run_with(&Config::new("word", 1), |setup| {
        setup.task("writer", |cx: &TaskCx| cx.op("No such file", "x", None));
    });
    assert_eq!(outcome.failure, Some(Failure::Misuse("op-word")));
    assert!(!outcome.trace.contains("No such file"));
}

#[test]
fn a_failed_spawn_at_the_first_grant_is_a_host_failure_not_a_lost_baton() {
    let mut config = Config::new("spawn", 1);
    config.watchdog = Duration::from_secs(2);
    config.plants.grant_before_spawn = true;
    config.plants.fail_spawn = true;
    let outcome = run_with(&config, three_by_three);
    assert_eq!(outcome.failure, Some(Failure::Host("thread-spawn")));
    assert_eq!(
        terminal(&outcome.trace),
        Some("end failed host:thread-spawn")
    );
}

/// The planted hidden block of the specification's 10.5, point 7: a real
/// sleep in a task, past a short watchdog. A named departure from coding
/// standards rule 15's "no real time or sleeps": it is the only way to
/// show the watchdog fires, and it lives in test code alone.
#[test]
fn a_real_sleep_past_the_watchdog_is_an_unconfirmed_lost_baton() {
    let mut config = Config::new("sleep", 1);
    config.watchdog = Duration::from_millis(50);
    let outcome = run_with(&config, |setup| {
        setup.task("sleeper", |cx: &TaskCx| {
            cx.op("before", "sleep", None);
            std::thread::sleep(Duration::from_millis(500));
        });
        setup.task("other", |cx: &TaskCx| cx.op("other", "work", None));
    });
    assert_eq!(
        outcome.failure,
        Some(Failure::LostBaton {
            task: TaskId(0),
            decision: decisions(&outcome.trace)
                .iter()
                .find(|decision| decision.chosen() == Some(Entry::Task(TaskId(0))))
                .unwrap()
                .number,
        })
    );
    assert_eq!(terminal(&outcome.trace), Some("end failed lost-baton:t0"));
    assert!(!outcome.trace.contains(" t0 end returned"));
}
