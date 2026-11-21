// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The model front end: a model of a binding in its own process (ADR
//! 0004, C16, point 6), the FSKit extension or the SMB front end. The
//! daemon side sends it, on one ordered and bounded stream, epoch
//! advances and replies stamped with the epoch they were produced under.
//! The front end takes one item per step of its own task, so an advance
//! is applied on a step scheduled apart from the daemon's, and it
//! compares a reply's epoch with the latest it has applied immediately
//! before it writes it: an older reply is discarded.
//!
//! What it applies and writes is fed to [`BreakOrder`], so the
//! simulator's break-ordering invariant (O45, point 3) checks the
//! effective-break point out of process, as the core's record checks it
//! in process.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ironafs_time::BootDeadline;

use super::check::{Binding, BreakOrder};
use super::types::Fid;
use crate::sched::{SignalId, SimDeadline, TaskCx, Wait, Waited};

#[cfg(test)]
mod tests;

/// The most items the stream holds (C39, point 3: every queue is
/// bounded). The model's own value.
pub const FRONT_CAP: usize = 64;

/// One item on the stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// The binding's fence epoch moves to `epoch`, for a break of `fid`'s
    /// version `version`.
    Advance {
        /// The new epoch.
        epoch: u64,
        /// The file broken.
        fid: Fid,
        /// The version the break made stale.
        version: u64,
    },
    /// A read reply produced under `epoch`, carrying `fid`'s version
    /// `version`.
    Reply {
        /// The epoch it was produced under.
        epoch: u64,
        /// The file.
        fid: Fid,
        /// The version its bytes are.
        version: u64,
    },
}

/// What the front end did with one item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// An advance applied.
    Applied,
    /// A reply written.
    Written,
    /// A reply discarded: older than the applied epoch.
    Discarded,
}

/// The ordered, bounded stream from the daemon to the front end.
#[derive(Clone, Debug, Default)]
pub struct FrontStream {
    items: VecDeque<Item>,
    closed: bool,
}

impl FrontStream {
    /// Appends `item`; `false` if the stream is full.
    pub fn push(&mut self, item: Item) -> bool {
        if self.items.len() >= FRONT_CAP {
            return false;
        }
        self.items.push_back(item);
        true
    }

    /// Takes the first item.
    pub fn pop(&mut self) -> Option<Item> {
        self.items.pop_front()
    }

    /// The items waiting.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether no item waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// No item will follow: the front end ends once it has taken the
    /// last.
    pub const fn close(&mut self) {
        self.closed = true;
    }
}

/// The front end's state: the latest epoch it has applied.
#[derive(Clone, Debug)]
pub struct FrontEnd {
    applied: u64,
    compare: bool,
}

impl Default for FrontEnd {
    fn default() -> Self {
        Self::new()
    }
}

impl FrontEnd {
    /// A front end at epoch 0.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            applied: 0,
            compare: true,
        }
    }

    /// A plant: write every reply without comparing its epoch, as a
    /// front end that broke C16, point 6, would. Off in every evidence
    /// run.
    pub const fn plant_no_compare(&mut self) {
        self.compare = false;
    }

    /// The latest epoch applied.
    #[must_use]
    pub const fn applied(&self) -> u64 {
        self.applied
    }

    /// Applies or writes one item, in one step, feeding `order`.
    pub fn step(&mut self, item: &Item, order: &mut BreakOrder) -> Step {
        match *item {
            Item::Advance {
                epoch,
                fid,
                version,
            } => {
                self.applied = self.applied.max(epoch);
                order.effective(Binding::FrontEnd, fid, version);
                Step::Applied
            }
            Item::Reply {
                epoch,
                fid,
                version,
            } => {
                if self.compare && epoch < self.applied {
                    return Step::Discarded;
                }
                order.submitted(Binding::FrontEnd, fid, version);
                Step::Written
            }
        }
    }
}

/// The stream as two tasks share it: the items, and the signal raised
/// while any waits or once it is closed.
#[derive(Debug)]
pub struct Shared {
    stream: Mutex<FrontStream>,
    signal: SignalId,
}

impl Shared {
    /// An empty stream whose waiter wakes on `signal`.
    #[must_use]
    pub fn new(signal: SignalId) -> Arc<Self> {
        Arc::new(Self {
            stream: Mutex::new(FrontStream::default()),
            signal,
        })
    }

    fn stream(&self) -> std::sync::MutexGuard<'_, FrontStream> {
        self.stream.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The daemon side sends `item`; `false` if the stream is full.
    pub fn send(&self, cx: &TaskCx, item: Item) -> bool {
        let sent = self.stream().push(item);
        if sent {
            cx.raise(self.signal);
        }
        sent
    }

    /// The daemon side closes the stream.
    pub fn close(&self, cx: &TaskCx) {
        self.stream().close();
        cx.raise(self.signal);
    }
}

/// The front end's task: until the stream is closed and empty, or
/// `idle` passes with nothing to do, it waits for an item, takes one,
/// applies or writes it, records what it did as an operation event, and
/// yields again.
pub fn run_front_end(
    cx: &TaskCx,
    shared: &Shared,
    front: &mut FrontEnd,
    order: &Mutex<BreakOrder>,
    idle: Duration,
) {
    loop {
        let Ok(now) = cx.clock().boot() else {
            return;
        };
        let Some(deadline) = BootDeadline::after(now, idle) else {
            return;
        };
        let waited = cx.wait(Wait {
            signal: Some(shared.signal),
            cancel: None,
            deadline: SimDeadline::Boot(deadline),
        });
        if waited != Waited::Signalled {
            return;
        }
        let item = {
            let mut stream = shared.stream();
            let item = stream.pop();
            if item.is_none() && stream.closed {
                return;
            }
            // A closed stream keeps its signal raised, so the close is
            // seen once the last item is taken.
            if stream.is_empty() && !stream.closed {
                cx.lower(shared.signal);
            }
            item
        };
        let Some(item) = item else {
            continue;
        };
        let step = front.step(
            &item,
            &mut order.lock().unwrap_or_else(PoisonError::into_inner),
        );
        let (epoch, fid, version) = match item {
            Item::Advance {
                epoch,
                fid,
                version,
            }
            | Item::Reply {
                epoch,
                fid,
                version,
            } => (epoch, fid, version),
        };
        let label = match step {
            Step::Applied => "front-apply",
            Step::Written => "front-write",
            Step::Discarded => "front-discard",
        };
        let object = format!(
            "f{}.{}.{}:v{version}",
            fid.volume.0, fid.vnode.0, fid.unique.0
        );
        cx.op(label, &object, Some(epoch));
    }
}
