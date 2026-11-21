// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux network-change monitor's state machine
//! (docs/specs/network-generation.md, 5.7 and section 7). It does no I/O:
//! a driver subscribes the notify socket first, then hands every datagram
//! of that socket to [`Monitor::notification`], sends each
//! [`Monitor::request`] on its request socket and hands the replies to
//! [`Monitor::reply`], and reports what went wrong on either socket.
//!
//! The monitor keeps a table of links and of addresses only to tell a
//! lifetime refresh, or a link message that leaves usability unchanged,
//! from a change. Every other message of its groups is a change. Anything
//! that can hide a change moves the generation at once and starts a fresh
//! snapshot, whose completion moves it again.

use alloc::collections::BTreeMap;

use crate::Reader;
use crate::consumer::Change;
use crate::consumer::{Cause, Consumers, Loss};
use crate::generation::{Publisher, Stopped};
use crate::netlink::{
    AddressKey, Dump, Event, LinkState, NLMSG_NOOP, acknowledged_noop, dump_request, event,
    messages,
};

#[cfg(test)]
mod tests;

/// Snapshots that may fail in a row before the source stops (5.5,
/// point 4). A dirty snapshot taken again is not a failure.
pub(crate) const SNAPSHOT_ATTEMPTS: u32 = 8;

/// The tables' caps (5.5, point 3).
pub(crate) const LINKS_MAX: usize = 1024;
pub(crate) const ADDRESSES_MAX: usize = 4096;

/// A dump the driver must send on its request socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub(crate) dump: Dump,
    pub(crate) sequence: u32,
}

impl Request {
    /// Whether this is a snapshot's first dump, of links; else it is its
    /// second, of addresses.
    #[must_use]
    pub fn is_links(&self) -> bool {
        self.dump == Dump::Links
    }

    /// The request's bytes, ready to send to the kernel.
    #[must_use]
    pub fn bytes(&self) -> [u8; 20] {
        dump_request(self.dump, self.sequence)
    }
}

/// The request the driver sends on its **notify** socket once a
/// snapshot's dumps are complete. The kernel acknowledges it on that
/// socket, after every notification queued before it; the snapshot is
/// published only when the monitor reads that acknowledgement, so nothing
/// the notify socket received before it (an overflow among them) can
/// follow the publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barrier {
    pub(crate) sequence: u32,
}

impl Barrier {
    /// The request's bytes: `NLMSG_NOOP` with `NLM_F_REQUEST | NLM_F_ACK`.
    #[must_use]
    pub fn bytes(&self) -> [u8; 16] {
        acknowledged_noop(self.sequence)
    }
}

/// A request that wakes a thread blocked receiving on a route socket: the
/// kernel answers it with an acknowledgement of sequence 0, which the
/// monitor never uses for anything else, and which moves nothing.
#[must_use]
pub fn wake() -> [u8; 16] {
    acknowledged_noop(0)
}

/// A table of what decides usability, capped. Over its cap it cannot
/// recognise a refresh, so every message of its kind is a change until a
/// snapshot fits again.
#[derive(Debug)]
struct Table<K, V> {
    entries: BTreeMap<K, V>,
    cap: usize,
    over: bool,
}

impl<K: Ord, V: PartialEq> Table<K, V> {
    fn new(cap: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            cap,
            over: false,
        }
    }

    /// Records `key` as `value`; whether that is a change.
    fn put(&mut self, key: K, value: V) -> bool {
        if self.over {
            return true;
        }
        let full = self.entries.len() >= self.cap;
        match self.entries.get_mut(&key) {
            Some(held) if *held == value => false,
            Some(held) => {
                *held = value;
                true
            }
            None if full => {
                self.over = true;
                self.entries.clear();
                true
            }
            None => {
                self.entries.insert(key, value);
                true
            }
        }
    }
}

/// An address's usability: its flags and scope.
type AddressState = (u32, u8);

/// A snapshot in progress: the dump outstanding and the fresh tables.
#[derive(Debug)]
struct Snapshot {
    request: Request,
    /// Once both dumps are complete, the barrier sent on the notify
    /// socket, whose acknowledgement publishes the snapshot.
    barrier: Option<u32>,
    /// A link or address notification arrived while it ran: when its
    /// barrier comes back it is taken again instead of published.
    dirty: bool,
    links: Table<i32, LinkState>,
    addresses: Table<AddressKey, AddressState>,
}

/// The monitor: the counter's only writer, the live tables, and the
/// snapshot in progress.
#[derive(Debug)]
pub struct Monitor {
    publisher: Publisher,
    links: Table<i32, LinkState>,
    addresses: Table<AddressKey, AddressState>,
    snapshot: Option<Snapshot>,
    /// The last sequence number used; wraps.
    sequence: u32,
    /// Snapshots failed since the last one completed.
    failures: u32,
    /// Whether a snapshot has completed: until then nothing is published.
    started: bool,
    /// Whether the driver's notify socket was made after the last loss on
    /// it (or at the start).
    subscribed: bool,
    stopped: bool,
}

impl Monitor {
    /// A monitor whose first snapshot is outstanding, and the reader of
    /// its generation, unavailable until that snapshot completes. `make`
    /// builds the consumers' hooks, and may give each a reader. The driver
    /// must have subscribed its notify socket before it sends the first
    /// request.
    pub fn new(make: impl FnOnce(&Reader) -> Consumers) -> (Self, Reader) {
        let (mut publisher, reader) = Publisher::new(make);
        publisher.withhold();
        let mut monitor = Self {
            publisher,
            links: Table::new(LINKS_MAX),
            addresses: Table::new(ADDRESSES_MAX),
            snapshot: None,
            sequence: 0,
            failures: 0,
            started: false,
            subscribed: false,
            stopped: false,
        };
        monitor.begin_snapshot();
        (monitor, reader)
    }

    /// The driver's notify socket has joined its groups, fresh: every
    /// notification from now on reaches it, so a snapshot may start. Until
    /// this is called after [`Monitor::new`] or after a loss on the notify
    /// socket, [`Monitor::request`] asks for nothing.
    pub fn subscribed(&mut self) {
        self.subscribed = true;
    }

    /// Whether the driver must make its notify socket again and call
    /// [`Monitor::subscribed`]: at the start, and after a loss on that
    /// socket, whose queue may hold records older than the loss.
    #[must_use]
    pub fn needs_subscription(&self) -> bool {
        !self.subscribed && !self.stopped
    }

    /// Stops the source without calling any hook: for a driver unwinding
    /// from a hook's panic, where calling the hooks again could panic
    /// twice. The generation becomes unavailable, and every later call
    /// fails.
    pub fn halt(&mut self) {
        self.stopped = true;
        self.snapshot = None;
        self.publisher.halt();
    }

    /// The dump to send next, if a snapshot is in progress. The same
    /// request is returned until its answer is complete or it is
    /// abandoned.
    #[must_use]
    pub fn request(&self) -> Option<Request> {
        if !self.subscribed {
            return None;
        }
        self.snapshot
            .as_ref()
            .filter(|snapshot| snapshot.barrier.is_none())
            .map(|snapshot| snapshot.request)
    }

    /// The barrier to send on the notify socket, once a snapshot's dumps
    /// are complete; the same one until it comes back or the snapshot is
    /// abandoned.
    #[must_use]
    pub fn barrier(&self) -> Option<Barrier> {
        if !self.subscribed {
            return None;
        }
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.barrier)
            .map(|sequence| Barrier { sequence })
    }

    /// Applies one datagram of the notify socket.
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped.
    pub fn notification(&mut self, datagram: &[u8]) -> Result<(), Stopped> {
        self.live()?;
        // The barrier's acknowledgement publishes only from a whole
        // datagram: a bad message after it is a loss, which abandons the
        // snapshot anyway.
        let whole = messages(datagram).all(|message| {
            message
                .and_then(|message| event(&message))
                .is_ok_and(|found| found != Event::Overrun)
        });
        let mut refused = false;
        for message in messages(datagram) {
            let Ok((sequence, found)) =
                message.and_then(|message| Ok((message.sequence, event(&message)?)))
            else {
                return self.lost(Loss::Malformed);
            };
            // A link or address message while a snapshot runs may describe
            // a state the dump read before or after it: the snapshot is
            // dirty, and is taken again once its barrier comes back, never
            // merged.
            if matches!(
                found,
                Event::Link(_) | Event::LinkGone(_) | Event::Address(_) | Event::AddressGone(_)
            ) && let Some(snapshot) = self.snapshot.as_mut()
            {
                snapshot.dirty = true;
            }
            let change = match found {
                Event::Route => Some(Change::Route),
                Event::Rule => Some(Change::Rule),
                Event::Nexthop => Some(Change::Nexthop),
                Event::Link(link) => self
                    .links
                    .put(link.index, link.state)
                    .then_some(Change::Link),
                Event::LinkGone(index) => {
                    self.links.entries.remove(&index);
                    if let Ok(index) = u32::try_from(index) {
                        self.addresses.entries.retain(|key, _| key.index != index);
                    }
                    Some(Change::Link)
                }
                Event::Address(address) => self
                    .addresses
                    .put(address.key, (address.flags, address.scope))
                    .then_some(Change::Address),
                Event::AddressGone(address) => {
                    self.addresses.entries.remove(&address.key);
                    Some(Change::Address)
                }
                Event::Overrun => return self.lost(Loss::Malformed),
                Event::Error(_) | Event::Acknowledged(_) => {
                    if self
                        .barrier()
                        .is_some_and(|barrier| barrier.sequence == sequence)
                    {
                        // Only an acknowledgement of the barrier's own
                        // request, error 0, is its answer.
                        if found != Event::Acknowledged(NLMSG_NOOP) {
                            refused = true;
                        } else if whole {
                            self.barrier_back()?;
                        }
                    }
                    None
                }
                Event::Done | Event::Skipped => None,
            };
            if let Some(change) = change {
                self.advance(Cause::Changed(change))?;
            }
        }
        if refused {
            return self.dump_failed(Loss::Refused);
        }
        Ok(())
    }

    /// Applies one datagram of the request socket: part of the answer to
    /// [`Monitor::request`]. Messages of any other sequence are stale and
    /// skipped.
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped.
    pub fn reply(&mut self, datagram: &[u8]) -> Result<(), Stopped> {
        self.live()?;
        // The whole datagram is checked before any of it is applied, so a
        // `NLMSG_DONE` never completes a snapshot ahead of bad bytes, and
        // an overrun of any sequence fails the dump.
        let Some(current) = self.request() else {
            return Ok(());
        };
        // After the dump's terminator, nothing more of its sequence may
        // follow, and an error of its sequence refuses it.
        let mut ended = false;
        for message in messages(datagram) {
            let Ok((sequence, found)) =
                message.and_then(|message| Ok((message.sequence, event(&message)?)))
            else {
                return self.dump_failed(Loss::Malformed);
            };
            if found == Event::Overrun {
                return self.dump_failed(Loss::Malformed);
            }
            if sequence == current.sequence {
                match found {
                    Event::Error(_) | Event::Acknowledged(_) => {
                        return self.dump_failed(Loss::Refused);
                    }
                    _ if ended => return self.dump_failed(Loss::Malformed),
                    Event::Done => ended = true,
                    _ => {}
                }
            }
        }
        for message in messages(datagram) {
            let Some(snapshot) = self.snapshot.as_mut() else {
                return Ok(());
            };
            let Ok(message) = message else {
                return self.dump_failed(Loss::Malformed);
            };
            if message.sequence != snapshot.request.sequence {
                continue;
            }
            if message.interrupted() {
                return self.dump_failed(Loss::Interrupted);
            }
            match event(&message) {
                Err(_) => return self.dump_failed(Loss::Malformed),
                Ok(Event::Link(link)) => {
                    snapshot.links.put(link.index, link.state);
                }
                Ok(Event::Address(address)) => {
                    snapshot
                        .addresses
                        .put(address.key, (address.flags, address.scope));
                }
                Ok(Event::Error(_) | Event::Acknowledged(_)) => {
                    return self.dump_failed(Loss::Refused);
                }
                // Nothing after the terminator is applied: the next dump's
                // sequence was checked by nobody, and its request is not
                // yet sent.
                Ok(Event::Done) => match snapshot.request.dump {
                    Dump::Links => {
                        self.sequence = next_after(self.sequence);
                        snapshot.request = Request {
                            dump: Dump::Addresses,
                            sequence: self.sequence,
                        };
                        return Ok(());
                    }
                    Dump::Addresses => {
                        self.sequence = next_after(self.sequence);
                        snapshot.barrier = Some(self.sequence);
                        return Ok(());
                    }
                },
                Ok(_) => {}
            }
        }
        Ok(())
    }

    /// Something on the notify socket, or outside both sockets (a
    /// resume), may have hidden a change: the generation becomes
    /// unavailable, every consumer is told of the move at once, and a
    /// fresh snapshot starts, abandoning one in progress; its completion
    /// publishes again. After any loss but a resume, the driver makes its
    /// notify socket again first ([`Monitor::needs_subscription`]).
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped.
    pub fn lost(&mut self, loss: Loss) -> Result<(), Stopped> {
        self.recover(loss, loss != Loss::Resumed)
    }

    /// The snapshot in progress failed (a send or receive failure on the
    /// request socket, or a bad answer): as [`Monitor::lost`], and counted.
    /// The source stops after [`SNAPSHOT_ATTEMPTS`] failures with no
    /// snapshot completed between them.
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped, including by this call.
    pub fn dump_failed(&mut self, loss: Loss) -> Result<(), Stopped> {
        self.live()?;
        self.failures = self.failures.saturating_add(1);
        if self.failures >= SNAPSHOT_ATTEMPTS {
            self.stop();
            return Err(Stopped);
        }
        self.recover(loss, false)
    }

    /// Withholds the generation, tells every consumer of the loss, and
    /// starts a fresh snapshot; with `resubscribe`, only once the driver
    /// has made its notify socket again.
    fn recover(&mut self, loss: Loss, resubscribe: bool) -> Result<(), Stopped> {
        self.live()?;
        if self.started {
            self.publisher.withhold();
        }
        self.advance(Cause::Lost(loss))?;
        if resubscribe {
            self.subscribed = false;
        }
        self.begin_snapshot();
        Ok(())
    }

    /// Fails once the source has stopped.
    fn live(&self) -> Result<(), Stopped> {
        if self.stopped { Err(Stopped) } else { Ok(()) }
    }

    /// Moves the generation, once a snapshot has completed; before that
    /// nothing is published and no consumer has been given a generation,
    /// so none is told, and the first completion's move covers it.
    fn advance(&mut self, cause: Cause) -> Result<(), Stopped> {
        if !self.started {
            return Ok(());
        }
        if self.publisher.advance(cause).is_err() {
            self.stop();
            return Err(Stopped);
        }
        Ok(())
    }

    fn begin_snapshot(&mut self) {
        let sequence = self.next_sequence();
        self.snapshot = Some(Snapshot {
            request: Request {
                dump: Dump::Links,
                sequence,
            },
            barrier: None,
            dirty: false,
            links: Table::new(LINKS_MAX),
            addresses: Table::new(ADDRESSES_MAX),
        });
    }

    /// The barrier came back: the snapshot is published or, if dirty,
    /// taken again. A retake is not a failed snapshot, and moves nothing:
    /// nothing is readable while a snapshot runs, and each change that made
    /// it dirty moved the generation itself.
    fn barrier_back(&mut self) -> Result<(), Stopped> {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.dirty)
        {
            self.begin_snapshot();
            return Ok(());
        }
        self.complete()
    }

    /// The fresh tables replace the live ones, and the generation moves.
    fn complete(&mut self) -> Result<(), Stopped> {
        if let Some(snapshot) = self.snapshot.take() {
            self.links = snapshot.links;
            self.addresses = snapshot.addresses;
        }
        self.failures = 0;
        self.started = true;
        if self.publisher.release(Cause::Snapshot).is_err() {
            self.stop();
            return Err(Stopped);
        }
        Ok(())
    }

    fn next_sequence(&mut self) -> u32 {
        self.sequence = next_after(self.sequence);
        self.sequence
    }

    /// Whether the source has stopped.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Stops the source: every consumer is told, the generation becomes
    /// unavailable, and every later call fails. A driver whose thread ends
    /// calls it, so readers never keep a generation nobody moves.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.snapshot = None;
        self.publisher.stop();
    }
}

/// The sequence number after `sequence`, wrapping, never 0: 0 is the
/// wake's ([`wake`]).
fn next_after(sequence: u32) -> u32 {
    match sequence.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}
