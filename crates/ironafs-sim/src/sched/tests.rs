// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The baton scheduler, in one process: tasks run one at a time in an
//! order drawn from the seed; grants, wakes and panics never lose the
//! baton; time moves by step cost and jumps; fairness, the budget and the
//! live-task cap hold.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ironafs_time::{AwakeDeadline, BootDeadline};

use super::{
    Config, Failure, Outcome, Polled, Push, SchedPlants, Served, Setup, SimDeadline, TaskCx, Wait,
    Waited, World, run,
};
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
        "ironafs-sim trace 3\n\
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

/// A world for the push tests: a call `push` starts one push to endpoint
/// 0; a call `hold` starts one and holds the call until the push
/// settles, then releases it with `late` if the push was answered, or
/// drops it; any other call is answered `ok`. Attempts up to `lose` are
/// lost.
#[derive(Default)]
struct PushWorld {
    lose: u32,
    pushes: Vec<Push>,
    held: Option<u64>,
    releases: Vec<(u64, Option<Vec<u8>>)>,
    settled: Vec<(u64, Option<Vec<u8>>)>,
}

#[derive(Clone, Default)]
struct SharedWorld(Arc<Mutex<PushWorld>>);

impl SharedWorld {
    fn losing(lose: u32) -> Self {
        Self(Arc::new(Mutex::new(PushWorld {
            lose,
            ..PushWorld::default()
        })))
    }

    fn settled(&self) -> Vec<(u64, Option<Vec<u8>>)> {
        self.0.lock().unwrap().settled.clone()
    }
}

fn push_to_zero(token: u64) -> Push {
    Push {
        token,
        endpoint: 0,
        request: b"brk".to_vec(),
        attempts: 3,
        interval: Duration::from_secs(1),
        timeout: Duration::from_secs(10),
    }
}

impl World for SharedWorld {
    fn serve(&mut self, call: u64, request: &[u8]) -> Served {
        let mut world = self.0.lock().unwrap();
        match request {
            b"push" => {
                world.pushes.push(push_to_zero(7));
                Served::Reply(b"ok".to_vec())
            }
            b"hold" => {
                world.pushes.push(push_to_zero(8));
                world.held = Some(call);
                Served::Hold
            }
            _ => Served::Reply(b"ok".to_vec()),
        }
    }

    fn admit(&mut self, _token: u64, attempt: u32) -> bool {
        attempt > self.0.lock().unwrap().lose
    }

    fn settled(&mut self, token: u64, response: Option<Vec<u8>>) {
        let mut world = self.0.lock().unwrap();
        world.settled.push((token, response.clone()));
        if let Some(call) = world.held.take() {
            let release = response.map(|_| b"late".to_vec());
            world.releases.push((call, release));
        }
    }

    fn poll(&mut self) -> Polled {
        let mut world = self.0.lock().unwrap();
        Polled {
            pushes: std::mem::take(&mut world.pushes),
            releases: std::mem::take(&mut world.releases),
        }
    }
}

/// A caller sends `request`; an acceptor on endpoint 0 answers `ack` to
/// whatever it accepts, if anything arrives within 20 s, and stays 20 s
/// more, since a run ends when its tasks have.
fn caller_and_acceptor(setup: &mut Setup, world: &SharedWorld, request: &'static [u8]) {
    setup.world(world.clone());
    setup.task("caller", move |cx: &TaskCx| {
        match cx.call(request, boot_in(cx, Duration::from_secs(30))) {
            Ok(reply) => cx.op("reply", "c0", Some(u64::try_from(reply.len()).unwrap())),
            Err(waited) => cx.op("no-reply", waited.name(), None),
        }
    });
    setup.task("service", |cx: &TaskCx| {
        match cx.accept(0, boot_in(cx, Duration::from_secs(20))) {
            Ok(incoming) => {
                assert_eq!(incoming.request, b"brk");
                cx.respond(incoming, b"ack");
                // Stay until the answer has reached the world.
                let waited = cx.wait(Wait {
                    signal: None,
                    cancel: None,
                    deadline: boot_in(cx, Duration::from_secs(20)),
                });
                assert_eq!(waited, Waited::DeadlinePassed);
            }
            Err(waited) => cx.op("no-call", waited.name(), None),
        }
    });
}

#[test]
fn a_world_push_reaches_an_accepting_task_and_its_answer_settles_it() {
    let world = SharedWorld::default();
    let outcome = run_with(&Config::new("push", 3), |setup| {
        caller_and_acceptor(setup, &world, b"push");
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    for line in [
        " world push p0 e0 62726b\n",
        " world arrive p0 try 1\n",
        " t1 accept p0 62726b\n",
        " t1 respond p0 61636b\n",
        " world answer p0 61636b\n",
    ] {
        assert!(outcome.trace.contains(line), "{line}: {}", outcome.trace);
    }
    assert_eq!(world.settled(), vec![(7, Some(b"ack".to_vec()))]);
    let again = run_with(&Config::new("push", 3), |setup| {
        caller_and_acceptor(setup, &SharedWorld::default(), b"push");
    });
    assert_eq!(outcome.trace, again.trace);
}

#[test]
fn a_lost_attempt_is_retried_and_every_attempt_lost_expires_as_failed() {
    let world = SharedWorld::losing(1);
    let outcome = run_with(&Config::new("lose-one", 3), |setup| {
        caller_and_acceptor(setup, &world, b"push");
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    assert!(outcome.trace.contains(" world lose p0 try 1\n"));
    assert!(outcome.trace.contains(" world arrive p0 try 2\n"));
    assert_eq!(outcome.trace.matches(" accept p0 ").count(), 1);
    assert_eq!(world.settled(), vec![(7, Some(b"ack".to_vec()))]);

    let world = SharedWorld::losing(3);
    let outcome = run_with(&Config::new("lose-all", 3), |setup| {
        caller_and_acceptor(setup, &world, b"push");
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    for attempt in 1..=3 {
        assert!(
            outcome
                .trace
                .contains(&format!(" world lose p0 try {attempt}\n"))
        );
    }
    assert!(!outcome.trace.contains(" world arrive p0"));
    assert!(outcome.trace.contains(" world expire p0\n"));
    assert!(outcome.trace.contains(" t1 op no-call deadline-passed\n"));
    assert_eq!(world.settled(), vec![(7, None)]);
}

#[test]
fn a_held_call_is_answered_when_the_world_releases_it_and_dropped_on_none() {
    let world = SharedWorld::default();
    let outcome = run_with(&Config::new("hold", 2), |setup| {
        caller_and_acceptor(setup, &world, b"hold");
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    let hold = outcome.trace.find(" world hold c0\n").unwrap();
    let answer = outcome.trace.find(" world answer p0 ").unwrap();
    let reply = outcome.trace.find(" world reply c0 6c617465\n").unwrap();
    assert!(hold < answer && answer < reply, "{}", outcome.trace);
    assert!(outcome.trace.contains(" t0 op reply c0 4\n"));

    let world = SharedWorld::losing(3);
    let outcome = run_with(&Config::new("hold-lost", 2), |setup| {
        caller_and_acceptor(setup, &world, b"hold");
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    let expire = outcome.trace.find(" world expire p0\n").unwrap();
    let drop = outcome.trace.find(" world drop c0\n").unwrap();
    assert!(expire < drop);
    assert!(outcome.trace.contains(" t0 op no-reply deadline-passed\n"));
}

#[test]
fn a_push_no_task_accepts_expires_and_the_run_still_ends() {
    let world = SharedWorld::default();
    let outcome = run_with(&Config::new("no-acceptor", 4), |setup| {
        setup.world(world.clone());
        setup.task("caller", |cx: &TaskCx| {
            let reply = cx.call(b"push", boot_in(cx, Duration::from_secs(1)));
            assert_eq!(reply, Ok(b"ok".to_vec()));
            step(cx);
            let waited = cx.wait(Wait {
                signal: None,
                cancel: None,
                deadline: boot_in(cx, Duration::from_secs(15)),
            });
            assert_eq!(waited, Waited::DeadlinePassed);
        });
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    assert!(outcome.trace.contains(" world arrive p0 try 1\n"));
    assert!(outcome.trace.contains(" world expire p0\n"));
    assert_eq!(world.settled(), vec![(7, None)]);
    assert_eq!(terminal(&outcome.trace), Some("end ok"));
}

#[test]
fn a_late_respond_after_expiry_changes_nothing() {
    let world = SharedWorld::default();
    let outcome = run_with(&Config::new("late", 1), |setup| {
        setup.world(world.clone());
        setup.task("caller", |cx: &TaskCx| {
            let _ = cx.call(b"push", boot_in(cx, Duration::from_secs(1)));
        });
        setup.task("slow", |cx: &TaskCx| {
            let incoming = cx.accept(0, boot_in(cx, Duration::from_secs(20))).unwrap();
            let waited = cx.wait(Wait {
                signal: None,
                cancel: None,
                deadline: boot_in(cx, Duration::from_secs(30)),
            });
            assert_eq!(waited, Waited::DeadlinePassed);
            cx.respond(incoming, b"ack");
        });
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    assert!(outcome.trace.contains(" t1 respond p0 61636b\n"));
    assert!(!outcome.trace.contains(" world answer p0"));
    assert_eq!(world.settled(), vec![(7, None)]);
}

/// A world that releases a call it never held, at its first step.
struct BadRelease;

impl World for BadRelease {
    fn serve(&mut self, _call: u64, _request: &[u8]) -> Served {
        Served::Reply(b"ok".to_vec())
    }

    fn admit(&mut self, _token: u64, _attempt: u32) -> bool {
        true
    }

    fn settled(&mut self, _token: u64, _response: Option<Vec<u8>>) {}

    fn poll(&mut self) -> Polled {
        Polled {
            pushes: Vec::new(),
            releases: vec![(99, Some(b"x".to_vec()))],
        }
    }
}

#[test]
fn a_world_releasing_a_call_it_never_held_is_a_misuse() {
    let outcome = run_with(&Config::new("bad-release", 1), |setup| {
        setup.world(BadRelease);
        setup.task("caller", |cx: &TaskCx| {
            let _ = cx.call(b"go", boot_in(cx, Duration::from_secs(1)));
        });
    });
    assert_eq!(
        outcome.failure,
        Some(Failure::Misuse("release-unheld-call"))
    );
    assert!(!outcome.trace.contains(" world reply c99 "));
    assert_eq!(
        terminal(&outcome.trace),
        Some("end failed misuse:release-unheld-call")
    );
}
