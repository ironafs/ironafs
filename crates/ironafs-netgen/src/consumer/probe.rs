// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A test consumer that counts what it is told, shared by the unit tests
//! of the counter and of the monitor. The ordering of hooks against reads
//! is asserted in the integration tests, where a hook can hold a reader.

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{Cause, Consumer, Consumers, Loss, Move};

/// What one probe saw.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    /// Moves to a generation.
    pub(crate) moves: AtomicU64,
    /// Stops.
    pub(crate) stops: AtomicU64,
    /// The last generation told, 0 if none.
    pub(crate) last: AtomicU64,
    /// Moves for a snapshot.
    pub(crate) snapshots: AtomicU64,
    /// Moves for a loss, and the last loss's code (`loss_code`).
    pub(crate) losses: AtomicU64,
    pub(crate) last_loss: AtomicU64,
}

impl Seen {
    pub(crate) fn moves(&self) -> u64 {
        self.moves.load(Ordering::SeqCst)
    }
    pub(crate) fn stops(&self) -> u64 {
        self.stops.load(Ordering::SeqCst)
    }
    pub(crate) fn last(&self) -> u64 {
        self.last.load(Ordering::SeqCst)
    }
    pub(crate) fn snapshots(&self) -> u64 {
        self.snapshots.load(Ordering::SeqCst)
    }
    pub(crate) fn losses(&self) -> u64 {
        self.losses.load(Ordering::SeqCst)
    }
    pub(crate) fn last_loss(&self) -> u64 {
        self.last_loss.load(Ordering::SeqCst)
    }
}

/// A number for each loss, for `Seen::last_loss`.
pub(crate) fn loss_code(loss: Loss) -> u64 {
    match loss {
        Loss::Overflow => 1,
        Loss::Truncated => 2,
        Loss::Malformed => 3,
        Loss::Interrupted => 4,
        Loss::Refused => 5,
        Loss::Failed => 6,
        Loss::Resumed => 7,
    }
}

struct Probe(Arc<Seen>);

impl Consumer for Probe {
    fn moved(&mut self, change: Move) {
        match change {
            Move::To(generation, cause) => {
                self.0.moves.fetch_add(1, Ordering::SeqCst);
                self.0.last.store(generation.get(), Ordering::SeqCst);
                match cause {
                    Cause::Changed => {}
                    Cause::Snapshot => {
                        self.0.snapshots.fetch_add(1, Ordering::SeqCst);
                    }
                    Cause::Lost(loss) => {
                        self.0.losses.fetch_add(1, Ordering::SeqCst);
                        self.0.last_loss.store(loss_code(loss), Ordering::SeqCst);
                    }
                }
            }
            Move::Stopped => {
                self.0.stops.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

/// Seven probes, one per consumer, and what each saw, in the fields'
/// order.
pub(crate) fn probes() -> (Consumers, [Arc<Seen>; 7]) {
    let seen: [Arc<Seen>; 7] = Default::default();
    let hook = |index: usize| -> Box<dyn Consumer> {
        Box::new(Probe(seen.get(index).cloned().unwrap_or_default()))
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
    (consumers, seen)
}
