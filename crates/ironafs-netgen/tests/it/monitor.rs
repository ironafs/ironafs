// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The monitor's hooks that unwind or ask for a stop
//! (docs/specs/network-generation.md, 5.2, point 4, and 5.3, point 3), on
//! datagrams written here from 5.4's layout in the host's byte order.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ironafs_netgen::{Cause, Consumer, Consumers, Monitor, Move, Reader, StopRequest};

/// One message: a header of `kind`, `flags` and `sequence`, then `body`.
fn message(kind: u16, flags: u16, sequence: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(u32::try_from(16 + body.len()).unwrap().to_ne_bytes());
    bytes.extend(kind.to_ne_bytes());
    bytes.extend(flags.to_ne_bytes());
    bytes.extend(sequence.to_ne_bytes());
    bytes.extend(0_u32.to_ne_bytes());
    bytes.extend(body);
    bytes
}

/// The sequence number in a request's bytes.
fn sequence(request: &[u8]) -> u32 {
    u32::from_ne_bytes(request[8..12].try_into().unwrap())
}

/// A route notification: a change.
fn route() -> Vec<u8> {
    message(24, 0, 0, &[0; 12])
}

/// Answers the first snapshot: both dumps empty, then the barrier's
/// acknowledgement.
fn first_snapshot(monitor: &mut Monitor) {
    while let Some(request) = monitor.request() {
        let done = message(3, 2, sequence(&request.bytes()), &0_i32.to_ne_bytes());
        monitor.reply(&done).unwrap();
    }
    let barrier = monitor.barrier().unwrap().bytes();
    let mut body = 0_i32.to_ne_bytes().to_vec();
    body.extend(barrier);
    monitor
        .notification(&message(2, 0x100, sequence(&barrier), &body))
        .unwrap();
}

/// What the hooks saw: moves to a generation, and stops.
#[derive(Default)]
struct Counts {
    moves: AtomicU64,
    stops: AtomicU64,
}

/// A hook that counts what it is told, and on a change does `act`.
struct Hook {
    counts: Arc<Counts>,
    act: Arc<dyn Fn() + Send + Sync>,
}

impl Consumer for Hook {
    fn moved(&mut self, change: Move) {
        match change {
            Move::To(_, Cause::Changed(_)) => {
                self.counts.moves.fetch_add(1, Ordering::SeqCst);
                (self.act)();
            }
            Move::To(..) => {
                self.counts.moves.fetch_add(1, Ordering::SeqCst);
            }
            Move::Stopped => {
                self.counts.stops.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

/// A started monitor whose seven hooks count into one `Counts` and do
/// `act` on a change, and its reader.
fn started(act: &Arc<dyn Fn() + Send + Sync>) -> (Monitor, Reader, Arc<Counts>) {
    let counts = Arc::new(Counts::default());
    let hook = |_: usize| -> Box<dyn Consumer> {
        Box::new(Hook {
            counts: Arc::clone(&counts),
            act: Arc::clone(act),
        })
    };
    let consumers = Consumers {
        negative_cache: hook(0),
        grants: hook(1),
        server_trust: hook(2),
        dns_records: hook(3),
        dns_rankings: hook(4),
        dns_late_replies: hook(5),
        dns_flat_list: hook(6),
    };
    let (mut monitor, reader) = Monitor::new(|_| consumers);
    monitor.subscribed();
    first_snapshot(&mut monitor);
    assert_eq!(reader.current().unwrap().get(), 1);
    (monitor, reader, counts)
}

#[test]
fn a_hook_unwinding_from_a_move_halts_the_source_and_the_drop_tells_no_one() {
    let act: Arc<dyn Fn() + Send + Sync> = Arc::new(|| panic!("a planted panic in a hook"));
    let (mut monitor, reader, counts) = started(&act);
    let unwound = catch_unwind(AssertUnwindSafe(|| monitor.notification(&route())));
    assert!(unwound.is_err());
    // Nothing stays readable, and neither a later call nor the drop calls
    // a hook again.
    assert!(reader.current().is_err());
    drop(monitor);
    assert_eq!(counts.moves.load(Ordering::SeqCst), 7 + 1);
    assert_eq!(counts.stops.load(Ordering::SeqCst), 0);
}

#[test]
fn a_stop_asked_by_a_hook_happens_after_its_move_is_told_and_before_it_is_published() {
    // A lone change, whose move would be published at the datagram's end,
    // and a change followed by another, which must not be dispatched.
    for changes in [1, 2] {
        let request: Arc<Mutex<Option<StopRequest>>> = Arc::default();
        let shared = Arc::clone(&request);
        let ask = move || {
            if let Some(request) = shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
            {
                request.ask();
            }
        };
        let act: Arc<dyn Fn() + Send + Sync> = Arc::new(ask);
        let (mut monitor, reader, counts) = started(&act);
        *request.lock().unwrap() = Some(monitor.stop_request());
        let datagram = route().repeat(changes);
        assert!(monitor.notification(&datagram).is_err(), "{changes}");
        // The first change reached all seven consumers, a second none; the
        // stop was told, and nothing more was published.
        assert_eq!(counts.moves.load(Ordering::SeqCst), 7 + 7);
        assert_eq!(counts.stops.load(Ordering::SeqCst), 7);
        assert!(reader.current().is_err());
        assert!(monitor.is_stopped());
    }
}
