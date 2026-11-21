// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The network-change monitor on the running kernel
//! (docs/specs/network-generation.md, section 10, point 4). Unprivileged:
//! the first snapshot publishes a generation, and a resume moves it and is
//! followed by a snapshot; a hook that panics, on a move or on the stop,
//! and a hook that drops the handle, leave both threads to end. In a network namespace of their own, with
//! `CAP_NET_ADMIN` there (ignored tests, run so, one at a time): an
//! address, a route and a policy rule, each added and removed, move the
//! generation; an
//! overflow moves it and is followed by a snapshot.
//!
//! Each consumer's hook sends what it is told, and the generation it reads
//! during the call, to the test; a test waits on that channel, so a move
//! that never comes is a hang the runner terminates.

use std::fmt::Write as _;
use std::io::Write as _;
use std::process::Stdio;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use ironafs_net::{MonitorHandle, spawn};
use ironafs_netgen::{Cause, Change, Consumer, Consumers, Loss, Move, NetworkGeneration, Reader};

/// What a hook saw: its consumer's place in the order, the move, and the
/// generation it read during the call.
type Seen = (usize, Move, Option<NetworkGeneration>);

/// A planted panic in a hook: on a move of this cause, or on the stop.
#[derive(Clone, Copy)]
enum Planted {
    On(Cause),
    OnStop,
}

struct Hook {
    place: usize,
    reader: Reader,
    sender: Sender<Seen>,
    pause: Arc<Mutex<()>>,
    panic_on: Option<Planted>,
}

impl Consumer for Hook {
    fn moved(&mut self, change: Move) {
        // A test holding the pause stalls the monitor's thread here.
        drop(self.pause.lock().unwrap_or_else(PoisonError::into_inner));
        match (self.panic_on, change) {
            (Some(Planted::On(cause)), Move::To(_, seen)) => {
                assert_ne!(cause, seen, "a planted panic in a hook");
            }
            (Some(Planted::OnStop), _) => {
                assert_ne!(change, Move::Stopped, "a planted panic in a hook");
            }
            _ => {}
        }
        let read = self.reader.current().ok();
        let _ = self.sender.send((self.place, change, read));
    }
}

/// The subprocess builder: these tests change the namespace's addresses,
/// routes and rules with `ip`. The child's clock and randomness are not
/// inputs to any assertion.
#[expect(
    clippy::disallowed_types,
    reason = "rule 7: the kernel's state is changed by iproute2; the child's clock and \
              randomness are not inputs to any assertion"
)]
type Command = std::process::Command;

/// Seven hooks, one per consumer, all sending to one channel, all
/// stalled while `pause` is held.
fn hooks(
    sender: &Sender<Seen>,
    reader: &Reader,
    pause: &Arc<Mutex<()>>,
    panic_on: Option<Planted>,
) -> Consumers {
    let hook = |place: usize| -> Box<dyn Consumer> {
        Box::new(Hook {
            place,
            reader: reader.clone(),
            sender: sender.clone(),
            pause: Arc::clone(pause),
            panic_on,
        })
    };
    Consumers {
        negative_cache: hook(0),
        grants: hook(1),
        server_trust: hook(2),
        dns_records: hook(3),
        dns_rankings: hook(4),
        dns_late_replies: hook(5),
        dns_flat_list: hook(6),
    }
}

/// Receives one move's seven calls, checks that every consumer was told
/// in order and that none read the new generation during its call, and
/// returns the generation and cause.
#[track_caller]
fn next_move(moves: &Receiver<Seen>) -> (NetworkGeneration, Cause) {
    let mut told = None;
    for place in 0..7 {
        let (seen_place, change, read) = moves.recv().unwrap();
        assert_eq!(seen_place, place);
        let Move::To(generation, cause) = change else {
            panic!("stopped: {change:?}");
        };
        assert_ne!(
            read,
            Some(generation),
            "consumer {place} read the new generation"
        );
        assert!(told.is_none_or(|first| first == (generation, cause)));
        told = Some((generation, cause));
    }
    told.unwrap()
}

/// Waits for a move of `cause`, skipping others, then until the reader
/// shows that generation or a later one (the last hook returns before the
/// thread publishes; a loss's generation is published by the snapshot
/// after it), and returns it.
#[track_caller]
fn wait_for(moves: &Receiver<Seen>, reader: &Reader, cause: Cause) -> NetworkGeneration {
    loop {
        let (generation, seen) = next_move(moves);
        if seen == cause {
            while !reader.current().is_ok_and(|now| now >= generation) {
                std::thread::yield_now();
            }
            return generation;
        }
    }
}

/// A running monitor whose first snapshot has published, its reader, its
/// hooks' channel, and the lock that stalls its hooks.
fn started() -> (
    ironafs_net::MonitorHandle,
    Reader,
    Receiver<Seen>,
    Arc<Mutex<()>>,
) {
    let (sender, moves) = channel();
    let pause = Arc::new(Mutex::new(()));
    let (reader, handle) = spawn(|reader| hooks(&sender, reader, &pause, None)).unwrap();
    // On a shared host other changes may already have moved it past 1.
    wait_for(&moves, &reader, Cause::Snapshot);
    (handle, reader, moves, pause)
}

/// Runs `ip` with `arguments`, which must succeed.
fn ip(arguments: &str) {
    let status = Command::new("ip")
        .args(arguments.split_whitespace())
        .status()
        .unwrap();
    assert!(status.success(), "ip {arguments}");
}

#[test]
fn the_first_snapshot_publishes_a_generation() {
    let (_handle, reader, _moves, _pause) = started();
    assert!(reader.current().unwrap().get() >= 1);
}

#[test]
fn a_resume_moves_the_generation_then_a_snapshot_moves_it_again() {
    let (handle, reader, moves, _pause) = started();
    handle.resumed().unwrap();
    // Other changes on a shared host may come between; they are skipped.
    let lost = wait_for(&moves, &reader, Cause::Lost(Loss::Resumed));
    let snapshot = wait_for(&moves, &reader, Cause::Snapshot);
    assert!(snapshot > lost);
}

#[test]
fn a_hook_that_panics_leaves_nothing_readable_and_the_source_halted() {
    let (sender, moves) = channel();
    let pause = Arc::new(Mutex::new(()));
    let panic_on = Some(Planted::On(Cause::Lost(Loss::Resumed)));
    let (reader, handle) = spawn(|reader| hooks(&sender, reader, &pause, panic_on)).unwrap();
    wait_for(&moves, &reader, Cause::Snapshot);
    let resumed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.resumed()));
    assert!(resumed.is_err());
    // The loss withheld the generation before any hook ran, and the
    // poisoned lock halts the source on its next use.
    assert!(reader.current().is_err());
    assert_eq!(handle.resumed(), Err(ironafs_netgen::Stopped));
    assert!(reader.current().is_err());
}

#[test]
fn dropping_the_handle_stops_the_source_and_ends_both_threads() {
    let (handle, reader, moves, _pause) = started();
    drop(handle);
    // The drop returns once both threads have ended, which drops the
    // monitor and its hooks: the channel then closes. A quiet network
    // must not keep a thread blocked in its receive.
    let mut stopped = Vec::new();
    while let Ok((place, change, _)) = moves.recv() {
        if change == Move::Stopped {
            stopped.push(place);
        }
    }
    assert_eq!(stopped, (0..7).collect::<Vec<_>>());
    assert!(reader.current().is_err());
}

#[test]
fn a_hook_that_panics_on_the_stop_still_lets_both_threads_end() {
    let (sender, moves) = channel();
    let pause = Arc::new(Mutex::new(()));
    let panic_on = Some(Planted::OnStop);
    let (reader, handle) = spawn(|reader| hooks(&sender, reader, &pause, panic_on)).unwrap();
    drop(sender);
    wait_for(&moves, &reader, Cause::Snapshot);
    let dropped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(handle)));
    assert!(dropped.is_err());
    // Both threads end, which drops the monitor and its hooks: the channel
    // then closes. A thread left blocked in its receive keeps it open.
    while moves.recv().is_ok() {}
    assert!(reader.current().is_err());
}

/// A hook that, told of the second snapshot, waits for the handle and
/// drops it: on the notify thread, under the monitor's lock.
struct Dropper {
    handles: Receiver<MonitorHandle>,
    snapshots: usize,
    sender: Sender<Seen>,
}

impl Consumer for Dropper {
    fn moved(&mut self, change: Move) {
        if let Move::To(_, Cause::Snapshot) = change {
            self.snapshots += 1;
            if self.snapshots == 2 {
                drop(self.handles.recv());
            }
        }
        let _ = self.sender.send((0, change, None));
    }
}

#[test]
fn a_hook_that_drops_the_handle_stops_the_source_and_both_threads_end() {
    let (sender, moves) = channel();
    let (give, handles) = channel();
    let pause = Arc::new(Mutex::new(()));
    let (reader, handle) = spawn(|reader| Consumers {
        negative_cache: Box::new(Dropper {
            handles,
            snapshots: 0,
            sender: sender.clone(),
        }),
        ..hooks(&sender, reader, &pause, None)
    })
    .unwrap();
    drop(sender);
    wait_for(&moves, &reader, Cause::Snapshot);
    // The resume's snapshot, on the notify thread, drops the handle.
    handle.resumed().unwrap();
    give.send(handle).unwrap();
    let mut stopped = Vec::new();
    while let Ok((place, change, _)) = moves.recv() {
        if change == Move::Stopped {
            stopped.push(place);
        }
    }
    assert_eq!(stopped, (0..7).collect::<Vec<_>>());
    assert!(reader.current().is_err());
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and iproute2: run there, one test at a time"]
fn an_address_a_route_and_a_policy_rule_each_move_the_generation() {
    ip("link set lo up");
    for (change, add, delete) in [
        (
            Change::Address,
            "address add 192.0.2.1/24 dev lo",
            "address del 192.0.2.1/24 dev lo",
        ),
        (
            Change::Route,
            "route add 198.51.100.0/24 dev lo",
            "route del 198.51.100.0/24 dev lo",
        ),
        (
            Change::Rule,
            "rule add from 203.0.113.0/24 table 100",
            "rule del from 203.0.113.0/24 table 100",
        ),
        (
            Change::Address,
            "-6 address add 2001:db8:1::1/64 dev lo nodad",
            "-6 address del 2001:db8:1::1/64 dev lo",
        ),
        (
            Change::Route,
            "-6 route add 2001:db8:2::/64 dev lo",
            "-6 route del 2001:db8:2::/64 dev lo",
        ),
        (
            Change::Rule,
            "-6 rule add from 2001:db8:3::/64 table 100",
            "-6 rule del from 2001:db8:3::/64 table 100",
        ),
    ] {
        // Added, then removed: each moves it, and the namespace is left
        // as it was. Each change gets a monitor of its own, subscribed after
        // the last change's notifications were sent (`ip` returns after the
        // kernel's answer), so only this change can move it.
        for command in [add, delete] {
            let (_handle, reader, moves, _pause) = started();
            let before = reader.current().unwrap();
            ip(command);
            // The move of this kind of change: an address command's own
            // prefix route does not satisfy it.
            let after = wait_for(&moves, &reader, Cause::Changed(change));
            assert!(after > before, "{command}");
        }
    }
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and iproute2: run there, one test at a time"]
fn an_overflow_moves_the_generation_then_a_snapshot_moves_it_again() {
    ip("link set lo up");
    let (_handle, reader, moves, pause) = started();
    // The thread stalls in a hook on the first change, so the burst fills
    // the notify socket, which nobody reads, until the kernel drops.
    let held = pause.lock().unwrap();
    let mut lines = String::new();
    for index in 0..4000 {
        writeln!(lines, "address add 2001:db8:9::{index:x}/128 dev lo nodad").unwrap();
    }
    for index in 0..4000 {
        writeln!(lines, "address del 2001:db8:9::{index:x}/128 dev lo").unwrap();
    }
    // The batch goes through standard input: no file a root run writes.
    let mut batch = Command::new("ip")
        .args(["-batch", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    batch
        .stdin
        .take()
        .unwrap()
        .write_all(lines.as_bytes())
        .unwrap();
    assert!(batch.wait().unwrap().success());
    drop(held);
    let lost = wait_for(&moves, &reader, Cause::Lost(Loss::Overflow));
    let snapshot = wait_for(&moves, &reader, Cause::Snapshot);
    assert!(snapshot > lost);
}
