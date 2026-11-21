// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fileserver model's callback calls driven through the simulator:
//! breaks, resets and probes on demand to a callback service task,
//! forged and dropped breaks, callback continuity (assumption A14, as
//! `docs/specs/callbacks.md` 5.8 states it) with every retransmission of
//! a break dropped, and the model front end applying epoch advances on
//! its own steps (ADR 0004, O45, point 3; C16, points 3 and 6).
//!
//! The client side here is a toy scripted in each test: the client's
//! own callback service and cache are M2a-27's and M2a-39's (planned).
//! Every scenario runs twice per seed and must give identical traces.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ironafs_sim::model::callback::{
    CbAnswer, CbKind, CbProc, CbRequest, Reset, decode_cb_request, encode_cb_answer,
};
use ironafs_sim::model::cell::{Cell, PUSH_ATTEMPTS, serve_in};
use ironafs_sim::model::check::{BREAK_ORDER, Binding, BreakOrder, CONTINUITY, Continuity};
use ironafs_sim::model::envelope::{decode_answer, encode_request};
use ironafs_sim::model::front::{self, FrontEnd, Item};
use ironafs_sim::model::fs::{FileServer, FileSpec};
use ironafs_sim::model::types::{
    Addr, Answer, ClientId, ContextId, Fid, Procedure, Request, Results, Service, VolumeId,
};
use ironafs_sim::sched::{Config, Failure, Outcome, Setup, SimDeadline, TaskCx, Wait, Waited, run};
use ironafs_sim::trace::{ops, terminal};
use ironafs_time::BootDeadline;

const FS1: Addr = Addr([10, 0, 0, 2]);
const ATTACKER: Addr = Addr([192, 0, 2, 9]);
const ME: ClientId = ClientId(0);
const PEER: ClientId = ClientId(1);

type Shared = Arc<Mutex<Cell>>;
type Seen = Arc<Mutex<Vec<CbRequest>>>;

fn small() -> Fid {
    Fid::new(100, 2, 1)
}

fn big() -> Fid {
    Fid::new(100, 4, 1)
}

/// FS1 holding volume 100 with two small files.
fn cell() -> Cell {
    let mut cell = Cell::new();
    let mut one = FileServer::new(FS1);
    one.add_volume(VolumeId(100), b"user.a");
    assert!(one.add_file(small(), FileSpec::file(10)));
    assert!(one.add_file(big(), FileSpec::file(20)));
    cell.add_file_server(one);
    cell
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn after(cx: &TaskCx, wait: Duration) -> SimDeadline {
    let now = cx.clock().boot().unwrap();
    SimDeadline::Boot(BootDeadline::after(now, wait).unwrap())
}

/// Waits `wait` of virtual time.
fn sleep(cx: &TaskCx, wait: Duration) {
    let waited = cx.wait(Wait {
        signal: None,
        cancel: None,
        deadline: after(cx, wait),
    });
    assert_eq!(waited, Waited::DeadlinePassed);
}

/// Sends `procedure` to `to`'s file service as `client`.
fn ask(cx: &TaskCx, to: Addr, client: ClientId, procedure: Procedure) -> Result<Answer, Waited> {
    cx.op("ask", procedure.name().name(), None);
    let request = encode_request(&Request {
        to,
        service: Service::Fs,
        client,
        context: ContextId(1),
        procedure,
    });
    cx.call(&request, after(cx, Duration::from_secs(10)))
        .map(|reply| decode_answer(&reply).unwrap())
}

fn version_of(answer: &Answer) -> u64 {
    match answer {
        Answer::Result(Results::Status { status, .. }) => status.data_version,
        other => panic!("no status: {other:?}"),
    }
}

/// Yields until the client holds a promise on `small()`, then has the
/// peer write it, so FS1 owes the client a break.
fn owe_a_break(cx: &TaskCx, cell: &Shared) {
    while lock(cell)
        .file_server(FS1)
        .unwrap()
        .promise_holders(small())
        .is_empty()
    {
        sleep(cx, Duration::from_millis(1));
    }
    let write = Procedure::PeerWrite {
        fid: small(),
        offset: 0,
        bytes: b"new".to_vec(),
    };
    ask(cx, FS1, PEER, write).unwrap();
}

/// A callback service on the client's endpoint: accepts every call until
/// `idle` passes with none, records it, lets `apply` act on it, and
/// answers each with an empty result.
fn service(cx: &TaskCx, seen: &Seen, idle: Duration, apply: &dyn Fn(&CbRequest)) {
    while let Ok(incoming) = cx.accept(ME.0, after(cx, idle)) {
        let request = decode_cb_request(&incoming.request).unwrap();
        cx.op("service", request.procedure.name(), None);
        apply(&request);
        lock(seen).push(request);
        cx.respond(incoming, &encode_cb_answer(&CbAnswer::Empty));
    }
}

fn runs(
    name: &'static str,
    seed: u64,
    cell: &Shared,
    scenario: impl FnOnce(&mut Setup),
) -> Outcome {
    run(&Config::new(name, seed), None, None, |setup| {
        serve_in(setup, cell);
        scenario(setup);
    })
}

/// Runs `scenario` from a fresh cell, made by `prepare`, twice per seed
/// for seeds 1 to 3; asserts each passes with identical traces, and
/// returns the cell and the trace of seed 1.
fn deterministic(
    name: &'static str,
    prepare: fn(&mut Cell),
    scenario: fn(&mut Setup, &Shared, &Seen),
) -> (Cell, String, Vec<CbRequest>) {
    let mut first = None;
    for seed in 1..=3 {
        let mut traces = Vec::new();
        for _ in 0..2 {
            let mut fresh = cell();
            prepare(&mut fresh);
            let shared: Shared = Arc::new(Mutex::new(fresh));
            let seen = Seen::default();
            let outcome = runs(name, seed, &shared, |setup| scenario(setup, &shared, &seen));
            assert_eq!(outcome.failure, None, "seed {seed}: {}", outcome.trace);
            assert_eq!(terminal(&outcome.trace), Some("end ok"));
            if first.is_none() {
                let seen = lock(&seen).clone();
                first = Some((lock(&shared).clone(), outcome.trace.clone(), seen));
            }
            traces.push(outcome.trace);
        }
        assert_eq!(traces.first(), traces.last(), "seed {seed}");
    }
    first.unwrap()
}

fn on_demand(setup: &mut Setup, cell: &Shared, seen: &Seen) {
    setup.task("client", |cx: &TaskCx| {
        ask(cx, FS1, ME, Procedure::FsFetchStatus { fid: small() }).unwrap();
    });
    let operator = Arc::clone(cell);
    setup.task("operator", move |cx: &TaskCx| {
        owe_a_break(cx, &operator);
        for procedure in [
            Procedure::SimDeliverBreaks,
            Procedure::SimReset { client: ME },
            Procedure::SimProbe { client: ME },
        ] {
            assert_eq!(
                ask(cx, FS1, PEER, procedure),
                Ok(Answer::Result(Results::Started { pushes: 1 }))
            );
        }
    });
    let seen = Arc::clone(seen);
    setup.task("callbacks", move |cx: &TaskCx| {
        service(cx, &seen, Duration::from_secs(20), &|_| {});
    });
}

#[test]
fn breaks_resets_and_probes_reach_the_callback_service_on_demand() {
    let (mut cell, trace, seen) = deterministic("cb-on-demand", |_| {}, on_demand);
    let mut names: Vec<&str> = seen.iter().map(|call| call.procedure.name()).collect();
    names.sort_unstable();
    assert_eq!(names, ["cb.callback", "cb.init_state", "cb.probe"]);
    assert!(seen.iter().all(|call| call.from == FS1 && call.to == ME));
    assert!(seen.contains(&CbRequest {
        from: FS1,
        to: ME,
        procedure: CbProc::CallBack {
            fids: vec![small()]
        },
    }));
    let server = cell.file_server(FS1).unwrap();
    assert_eq!(server.delivered().len(), 3);
    assert_eq!(server.owed_breaks(), []);
    assert_eq!(server.failed_breaks(), []);
    for line in [
        " world push p0 e0 ",
        " world arrive p0 try 1\n",
        " world answer p0 ",
    ] {
        assert!(trace.contains(line), "{line}");
    }
}

fn forged(setup: &mut Setup, _cell: &Shared, seen: &Seen) {
    setup.task("attacker", |cx: &TaskCx| {
        let forge = Procedure::SimForge {
            from: FS1,
            to: ME,
            procedure: CbProc::CallBack {
                fids: vec![small()],
            },
        };
        assert_eq!(
            ask(cx, ATTACKER, PEER, forge),
            Ok(Answer::Result(Results::Started { pushes: 1 }))
        );
    });
    let seen = Arc::clone(seen);
    setup.task("callbacks", move |cx: &TaskCx| {
        service(cx, &seen, Duration::from_secs(20), &|_| {});
    });
}

#[test]
fn a_forged_break_reaches_the_service_and_no_server_hears_its_answer() {
    let (mut cell, _, seen) = deterministic("cb-forged", |_| {}, forged);
    let forged = CbRequest {
        from: FS1,
        to: ME,
        procedure: CbProc::CallBack {
            fids: vec![small()],
        },
    };
    assert_eq!(seen, std::slice::from_ref(&forged));
    assert_eq!(cell.forged_answers(), [(forged, Some(CbAnswer::Empty))]);
    assert_eq!(cell.file_server(FS1).unwrap().delivered(), []);
}

#[test]
fn a_dropped_break_is_retransmitted_and_arrives_once() {
    let (mut cell, trace, seen) = deterministic(
        "cb-dropped",
        |cell| cell.drop_callbacks(ME, CbKind::Break, 1),
        |setup, cell, seen| {
            setup.task("client", |cx: &TaskCx| {
                ask(cx, FS1, ME, Procedure::FsFetchStatus { fid: small() }).unwrap();
            });
            let operator = Arc::clone(cell);
            setup.task("operator", move |cx: &TaskCx| {
                owe_a_break(cx, &operator);
                ask(cx, FS1, PEER, Procedure::SimDeliverBreaks).unwrap();
            });
            let seen = Arc::clone(seen);
            setup.task("callbacks", move |cx: &TaskCx| {
                service(cx, &seen, Duration::from_secs(20), &|_| {});
            });
        },
    );
    assert!(trace.contains(" world lose p0 try 1\n"));
    assert!(trace.contains(" world arrive p0 try 2\n"));
    assert_eq!(seen.len(), 1);
    let server = cell.file_server(FS1).unwrap();
    assert_eq!(server.failed_breaks(), []);
    assert_eq!(server.lost_clients(), []);
}

/// The toy client's cache of `small()`: the version it holds, whether
/// that entry is still valid, and whether it holds callback trust for
/// FS1.
#[derive(Debug, Default)]
struct Toy {
    version: u64,
    valid: bool,
    trusted: bool,
}

/// The A14 run: the client caches `small()` under a promise; a peer
/// writes it; every attempt of FS1's break is lost while a forged probe
/// from FS1's address gets through. The client reads every 2 s, serving
/// its cached copy while it is valid and trusted, and calls FS1 after
/// each read (its keepalive here: the keepalive procedure is the
/// keepalive specification's, not yet written). Its callback service
/// suspends the files a break names, and a reset ends its trust and
/// every entry; nothing it receives raises trust. The continuity check
/// runs after every step.
fn a14_run(name: &'static str, seed: u64, a14: bool) -> (Outcome, Cell, Vec<CbRequest>) {
    let mut fresh = cell();
    fresh.drop_callbacks(ME, CbKind::Break, PUSH_ATTEMPTS);
    if !a14 {
        fresh.file_server(FS1).unwrap().plant_no_a14();
    }
    let cell: Shared = Arc::new(Mutex::new(fresh));
    let calls = Seen::default();
    let toy = Arc::new(Mutex::new(Toy::default()));
    let check = Arc::new(Mutex::new(Continuity::default()));
    let outcome = runs(name, seed, &cell, |setup| {
        let (client_cell, client_toy, client_check) =
            (Arc::clone(&cell), Arc::clone(&toy), Arc::clone(&check));
        setup.task("client", move |cx: &TaskCx| {
            for round in 0..7_u64 {
                let (valid, trusted, cached) = {
                    let toy = lock(&client_toy);
                    (toy.valid, toy.trusted, toy.version)
                };
                let served = if valid && trusted {
                    cached
                } else {
                    let answer = ask(cx, FS1, ME, Procedure::FsFetchStatus { fid: small() });
                    let version = version_of(&answer.unwrap());
                    let mut toy = lock(&client_toy);
                    toy.version = version;
                    toy.valid = true;
                    version
                };
                let now = lock(&client_cell)
                    .file_server(FS1)
                    .unwrap()
                    .data_version(small())
                    .unwrap();
                cx.op("read", "small", Some(served));
                lock(&client_check).read(FS1, served, now, valid && trusted);
                if ask(cx, FS1, ME, Procedure::FsFetchStatus { fid: big() }).is_ok() {
                    lock(&client_check).call_completed(FS1);
                    lock(&client_toy).trusted = true;
                }
                if round < 6 {
                    sleep(cx, Duration::from_secs(2));
                }
            }
        });
        let operator = Arc::clone(&cell);
        setup.task("operator", move |cx: &TaskCx| {
            owe_a_break(cx, &operator);
            ask(cx, FS1, PEER, Procedure::SimDeliverBreaks).unwrap();
            let probe = Procedure::SimForge {
                from: FS1,
                to: ME,
                procedure: CbProc::Probe,
            };
            ask(cx, ATTACKER, PEER, probe).unwrap();
        });
        let (service_seen, service_toy) = (Arc::clone(&calls), Arc::clone(&toy));
        setup.task("callbacks", move |cx: &TaskCx| {
            service(cx, &service_seen, Duration::from_secs(30), &|request| {
                let mut toy = lock(&service_toy);
                match &request.procedure {
                    CbProc::CallBack { fids } if fids.contains(&small()) => toy.valid = false,
                    CbProc::InitState(_) => {
                        toy.valid = false;
                        toy.trusted = false;
                    }
                    CbProc::CallBack { .. } | CbProc::Probe => {}
                }
            });
        });
        let (step_cell, step_check) = (Arc::clone(&cell), Arc::clone(&check));
        let mut failures_seen = 0;
        setup.check_each_step(move || {
            let failed = lock(&step_cell)
                .file_server(FS1)
                .unwrap()
                .failed_breaks()
                .len();
            let mut check = lock(&step_check);
            if failed > failures_seen {
                failures_seen = failed;
                check.break_failed(FS1);
            }
            check.check()
        });
    });
    let cell = lock(&cell).clone();
    let calls = lock(&calls).clone();
    (outcome, cell, calls)
}

#[test]
fn a14_every_retransmission_of_a_break_dropped_while_calls_succeed() {
    for seed in 1..=3 {
        let (outcome, mut cell, seen) = a14_run("cb-a14", seed, true);
        let (again, _, _) = a14_run("cb-a14", seed, true);
        assert_eq!(outcome.trace, again.trace, "seed {seed}");
        let trace = &outcome.trace;
        assert_eq!(outcome.failure, None, "seed {seed}: {trace}");
        for attempt in 1..=PUSH_ATTEMPTS {
            assert!(trace.contains(&format!(" world lose p0 try {attempt}\n")));
        }
        assert!(trace.contains(" world expire p0\n"));
        let server = cell.file_server(FS1).unwrap();
        assert_eq!(server.failed_breaks(), [(ME, small())]);
        assert_eq!(server.lost_clients(), []);
        let names: Vec<&str> = seen.iter().map(|call| call.procedure.name()).collect();
        assert!(!names.contains(&"cb.callback"), "{names:?}");
        assert!(names.contains(&"cb.probe"), "the forged probe arrived");
        assert!(names.contains(&CbProc::InitState(Reset::One).name()));
        // The client's next call was held until the reset was answered.
        let held_at = trace.find(" world hold c").unwrap();
        let number: String = trace
            .get(held_at + 13..)
            .unwrap()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let accept = trace.find(" t2 op service cb.init_state\n").unwrap();
        let reply = trace.find(&format!(" world reply c{number} ")).unwrap();
        assert!(held_at < accept && accept < reply, "seed {seed}");
        // Every read after the reset is of the current version.
        let reads: Vec<u64> = ops(trace)
            .iter()
            .filter(|op| op.label == "read")
            .filter_map(|op| op.value)
            .collect();
        assert_eq!(reads.last(), Some(&2), "seed {seed}");
    }
}

#[test]
fn without_a14_a_stale_read_after_the_next_call_fails_the_continuity_check() {
    for seed in 1..=3 {
        let (outcome, _, seen) = a14_run("cb-a14-off", seed, false);
        assert_eq!(
            outcome.failure,
            Some(Failure::Invariant(CONTINUITY)),
            "seed {seed}: {}",
            outcome.trace
        );
        assert!(
            seen.iter()
                .all(|call| call.procedure.kind() != CbKind::Reset)
        );
    }
}

/// The daemon side of the front-end run: reads `small()` (version 1)
/// and sends the reply at epoch 0; records a break of version 1, in
/// process and as an advance to epoch 1; yields; sends a reply of
/// version 1 produced before the break, still at epoch 0; then, after a
/// peer's write, reads version 2 and sends it at epoch 1. `late_in_process`
/// plants an in-process submission of version 1 after its break.
fn daemon(
    cx: &TaskCx,
    link: &Mutex<Option<Arc<front::Shared>>>,
    order: &Mutex<BreakOrder>,
    late_in_process: bool,
) {
    let shared = loop {
        if let Some(shared) = lock(link).clone() {
            break shared;
        }
        sleep(cx, Duration::from_millis(1));
    };
    let read = |cx: &TaskCx| {
        version_of(&ask(cx, FS1, ME, Procedure::FsFetchStatus { fid: small() }).unwrap())
    };
    let first = read(cx);
    lock(order).submitted(Binding::InProcess, small(), first);
    assert!(shared.send(
        cx,
        Item::Reply {
            epoch: 0,
            fid: small(),
            version: first
        }
    ));
    lock(order).effective(Binding::InProcess, small(), first);
    cx.op("daemon-advance", "small", Some(1));
    assert!(shared.send(
        cx,
        Item::Advance {
            epoch: 1,
            fid: small(),
            version: first
        }
    ));
    sleep(cx, Duration::from_millis(1));
    if late_in_process {
        lock(order).submitted(Binding::InProcess, small(), first);
    }
    assert!(shared.send(
        cx,
        Item::Reply {
            epoch: 0,
            fid: small(),
            version: first
        }
    ));
    let write = Procedure::PeerWrite {
        fid: small(),
        offset: 0,
        bytes: b"new".to_vec(),
    };
    ask(cx, FS1, PEER, write).unwrap();
    let second = read(cx);
    lock(order).submitted(Binding::InProcess, small(), second);
    assert!(shared.send(
        cx,
        Item::Reply {
            epoch: 1,
            fid: small(),
            version: second
        }
    ));
    shared.close(cx);
}

fn front_run(seed: u64, compare: bool, late_in_process: bool) -> Outcome {
    let cell: Shared = Arc::new(Mutex::new(cell()));
    let order = Arc::new(Mutex::new(BreakOrder::default()));
    let link: Arc<Mutex<Option<Arc<front::Shared>>>> = Arc::default();
    runs("cb-front", seed, &cell, |setup| {
        let (daemon_link, daemon_order) = (Arc::clone(&link), Arc::clone(&order));
        setup.task("daemon", move |cx: &TaskCx| {
            daemon(cx, &daemon_link, &daemon_order, late_in_process);
        });
        let (front_link, front_order) = (Arc::clone(&link), Arc::clone(&order));
        setup.task("front-end", move |cx: &TaskCx| {
            let shared = front::Shared::new(cx.signal());
            *lock(&front_link) = Some(Arc::clone(&shared));
            let mut front = FrontEnd::new();
            if !compare {
                front.plant_no_compare();
            }
            front::run_front_end(
                cx,
                &shared,
                &mut front,
                &front_order,
                Duration::from_secs(30),
            );
        });
        let check = Arc::clone(&order);
        setup.check_each_step(move || lock(&check).check());
    })
}

#[test]
fn the_front_end_applies_an_advance_on_its_own_step() {
    for seed in 1..=3 {
        let outcome = front_run(seed, true, false);
        assert_eq!(outcome.failure, None, "seed {seed}: {}", outcome.trace);
        assert_eq!(outcome.trace, front_run(seed, true, false).trace);
        let steps: Vec<(String, String, u64, u64)> = ops(&outcome.trace)
            .into_iter()
            .filter(|op| op.label.starts_with("front-") || op.label == "daemon-advance")
            .map(|op| (op.label, op.object, op.value.unwrap(), op.decision))
            .collect();
        let shape: Vec<(&str, &str, u64)> = steps
            .iter()
            .map(|(label, object, epoch, _)| (label.as_str(), object.as_str(), *epoch))
            .filter(|(label, _, _)| *label != "daemon-advance")
            .collect();
        assert_eq!(
            shape,
            [
                ("front-write", "f100.2.1:v1", 0),
                ("front-apply", "f100.2.1:v1", 1),
                ("front-discard", "f100.2.1:v1", 0),
                ("front-write", "f100.2.1:v2", 1),
            ],
            "seed {seed}"
        );
        let sent = steps
            .iter()
            .find(|step| step.0 == "daemon-advance")
            .unwrap()
            .3;
        let applied = steps.iter().find(|step| step.0 == "front-apply").unwrap().3;
        assert!(sent < applied, "seed {seed}: applied on a later step");
        // Each item on its own step.
        let decisions: Vec<u64> = steps
            .iter()
            .filter(|step| step.0.starts_with("front-"))
            .map(|step| step.3)
            .collect();
        assert!(
            decisions.windows(2).all(|pair| pair[0] < pair[1]),
            "seed {seed}"
        );
    }
}

#[test]
fn a_front_end_that_skips_the_comparison_or_a_late_in_process_reply_breaks_the_order() {
    for seed in 1..=3 {
        for (compare, late) in [(false, false), (true, true)] {
            let outcome = front_run(seed, compare, late);
            assert_eq!(
                outcome.failure,
                Some(Failure::Invariant(BREAK_ORDER)),
                "seed {seed}, compare {compare}, late {late}: {}",
                outcome.trace
            );
        }
    }
}
