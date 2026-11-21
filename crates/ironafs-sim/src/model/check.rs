// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Two of the simulator's invariants (ADR 0004, O45, point 3), as checks
//! a scenario feeds as its tasks act and runs after every step
//! ([`crate::sched::Setup::check_each_step`]). Tasks run one at a time,
//! so the order of the calls is the order of the run.

use std::collections::BTreeMap;

use super::types::{Addr, Fid};

#[cfg(test)]
mod tests;

/// The name of a broken continuity invariant, in the trace's terminal
/// line.
pub const CONTINUITY: &str = "a14-stale-read";

/// The name of a broken break-ordering invariant.
pub const BREAK_ORDER: &str = "break-order";

/// Where a server stands since it last failed to deliver a break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Since {
    /// The break failed; the client has not completed a call since.
    Failed,
    /// The client has completed a call since the failure.
    Called,
}

/// After the simulated server failed to deliver a break, a stale read
/// under trust is allowed only until the client's next completed call to
/// that server (O45, point 3, the third invariant; assumption A14).
#[derive(Clone, Debug, Default)]
pub struct Continuity {
    servers: BTreeMap<Addr, Since>,
    broken: bool,
}

impl Continuity {
    /// `server` failed to deliver a break: every attempt was lost.
    pub fn break_failed(&mut self, server: Addr) {
        self.servers.insert(server, Since::Failed);
    }

    /// A call of the client to `server` completed with a reply.
    pub fn call_completed(&mut self, server: Addr) {
        if let Some(since) = self.servers.get_mut(&server) {
            *since = Since::Called;
        }
    }

    /// The client served version `version_read` of a file of `server`,
    /// whose current version is `version_now`, under callback trust or
    /// not.
    pub fn read(&mut self, server: Addr, version_read: u64, version_now: u64, trusted: bool) {
        let stale = version_read < version_now;
        if stale && trusted && self.servers.get(&server) == Some(&Since::Called) {
            self.broken = true;
        }
    }

    /// # Errors
    ///
    /// [`CONTINUITY`] once a forbidden read has been fed.
    pub const fn check(&self) -> Result<(), &'static str> {
        if self.broken { Err(CONTINUITY) } else { Ok(()) }
    }
}

/// Where a break becomes effective (C16, points 3 and 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Binding {
    /// A binding in the daemon's process: effective when the core
    /// records the break.
    InProcess,
    /// A binding in its own process: effective when its front end has
    /// applied the epoch advance.
    FrontEnd,
}

/// No read reply starts to be submitted after a break of the version it
/// carries is effective at its binding (O45, point 3, the first
/// invariant).
#[derive(Clone, Debug, Default)]
pub struct BreakOrder {
    /// Per binding and file, the newest version a break has made stale.
    broken: BTreeMap<(Binding, Fid), u64>,
    violated: bool,
}

impl BreakOrder {
    /// A break of `fid`'s version `version` became effective at
    /// `binding`.
    pub fn effective(&mut self, binding: Binding, fid: Fid, version: u64) {
        let newest = self.broken.entry((binding, fid)).or_insert(version);
        *newest = (*newest).max(version);
    }

    /// A reply carrying `fid`'s version `version` started to be submitted
    /// at `binding`.
    pub fn submitted(&mut self, binding: Binding, fid: Fid, version: u64) {
        if self
            .broken
            .get(&(binding, fid))
            .is_some_and(|newest| version <= *newest)
        {
            self.violated = true;
        }
    }

    /// # Errors
    ///
    /// [`BREAK_ORDER`] once a forbidden submission has been fed.
    pub const fn check(&self) -> Result<(), &'static str> {
        if self.violated {
            Err(BREAK_ORDER)
        } else {
            Ok(())
        }
    }
}
