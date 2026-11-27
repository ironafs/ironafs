// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The vocabulary of the one blocking primitive, `Clock::wait` (seams and
//! scheduling specification, 5.3, points 1 to 4): samples, deadlines with
//! a latched doubt count, the wait's outcomes, and the watched flags a wait
//! reads.
//!
//! Everything here is a value or a trait. The concrete waiting primitives
//! (signals, cancel tokens, slots, queues) need a mutex, which `core`
//! lacks, so they live outside the pure crates and implement [`Watch`].

use alloc::sync::Arc;
use core::error::Error;
use core::fmt;
use core::time::Duration;

use crate::{
    AwakeDeadline, AwakeInstant, BootDeadline, BootInstant, ClockError, ClockKind, WallTime,
};

#[cfg(test)]
mod tests;

/// The doubt counts of the two monotonic clocks (5.3, point 2). Each only
/// grows: a failed read moves both, a regression moves the boot count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Doubt {
    /// The boot clock's doubt count.
    pub boot: u64,
    /// The awake clock's doubt count.
    pub awake: u64,
}

/// One sample as ADR 0004, C37, point 4, takes it (an awake read and a wall
/// read bracketed by two boot reads, all succeeding), with the doubt counts
/// and the episode count after this sample's own checks (5.3, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// The boot clock.
    pub boot: BootInstant,
    /// The awake clock.
    pub awake: AwakeInstant,
    /// Wall time.
    pub wall: WallTime,
    /// The doubt counts after this sample.
    pub doubt: Doubt,
    /// The episode count after this sample: a detected suspend, a
    /// regression, a failed read or a wall jump each counts one.
    pub episodes: u64,
}

/// Which clock a [`Deadline`] is on, with its instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum At {
    Boot(BootDeadline),
    Awake(AwakeDeadline),
}

/// A deadline on one monotonic clock, with a doubt count for that clock
/// latched when it was built (5.3, points 1 and 2).
///
/// Its fields are private to this crate, so outside it a deadline is built
/// only by [`Deadline::boot_after`] and [`Deadline::awake_after`], and
/// never by a literal, a destructured copy or an edited count. A wait, the
/// call owner's arbitration and a completion claim find it failed once its
/// clock's doubt count is above the latch, even if the sample taken then is
/// good.
///
/// ```compile_fail,E0451
/// // A literal is refused: the fields are private.
/// let _ = ironafs_time::Deadline { at: todo!(), latch: 0 };
/// ```
///
/// ```compile_fail,E0451
/// // So is a destructuring.
/// fn latch(deadline: ironafs_time::Deadline) -> u64 {
///     let ironafs_time::Deadline { latch, .. } = deadline;
///     latch
/// }
/// ```
///
/// ```compile_fail,E0616
/// // And a field write.
/// fn lower(deadline: &mut ironafs_time::Deadline) {
///     deadline.latch = 0;
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Deadline {
    at: At,
    latch: u64,
}

/// Why no deadline was built.
///
/// `Clock` is a failed sample, which the sampling function that builds
/// every deadline reports; the constructors themselves return only
/// `FromAfterNow` and `Overflow`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeadlineError {
    /// The fresh sample failed: no deadline is set until a valid sample
    /// returns (ADR 0004, C37, point 5).
    Clock(ClockError),
    /// `from` is newer than `now` on the deadline's clock. The caller
    /// treats it as a clock failure and never retries with another `from`.
    FromAfterNow,
    /// The deadline does not fit in 64-bit nanoseconds.
    Overflow,
}

impl fmt::Display for DeadlineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => {
                write!(formatter, "no deadline without a valid sample: {error}")
            }
            Self::FromAfterNow => {
                formatter.write_str("the deadline's start is newer than the sample it is built at")
            }
            Self::Overflow => {
                formatter.write_str("the deadline does not fit in 64-bit nanoseconds")
            }
        }
    }
}

impl Error for DeadlineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::FromAfterNow | Self::Overflow => None,
        }
    }
}

impl Deadline {
    /// The deadline `d` after `from` on the boot clock. Pure: calls no
    /// seam.
    ///
    /// `now` is the sample the caller has just taken, and `from` the
    /// request's recorded arrival or a reply's `first_sent`. Only the
    /// workspace's named sampling functions call this (`cargo xtask
    /// check-seams`).
    ///
    /// The latched count is the lower of `from`'s and `now`'s boot doubt
    /// counts, so a stale stamp gives a deadline that is already failed.
    ///
    /// # Errors
    ///
    /// [`DeadlineError::FromAfterNow`] if `from` is newer than `now` on the
    /// boot clock (a higher boot doubt count, or a later boot instant);
    /// [`DeadlineError::Overflow`] if the instant does not fit.
    pub fn boot_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> {
        if from.doubt.boot > now.doubt.boot || from.boot > now.boot {
            return Err(DeadlineError::FromAfterNow);
        }
        let at = BootDeadline::after(from.boot, d).ok_or(DeadlineError::Overflow)?;
        Ok(Self {
            at: At::Boot(at),
            latch: from.doubt.boot.min(now.doubt.boot),
        })
    }

    /// The deadline `d` after `from` on the awake clock, its instant and
    /// its doubt count; otherwise as [`Deadline::boot_after`].
    ///
    /// # Errors
    ///
    /// [`DeadlineError::FromAfterNow`] if `from` is newer than `now` on the
    /// awake clock; [`DeadlineError::Overflow`] if the instant does not fit.
    pub fn awake_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> {
        if from.doubt.awake > now.doubt.awake || from.awake > now.awake {
            return Err(DeadlineError::FromAfterNow);
        }
        let at = AwakeDeadline::after(from.awake, d).ok_or(DeadlineError::Overflow)?;
        Ok(Self {
            at: At::Awake(at),
            latch: from.doubt.awake.min(now.doubt.awake),
        })
    }

    /// The clock the deadline is on: [`ClockKind::Boot`] or
    /// [`ClockKind::Awake`], never [`ClockKind::Wall`].
    #[must_use]
    pub const fn clock(&self) -> ClockKind {
        match self.at {
            At::Boot(_) => ClockKind::Boot,
            At::Awake(_) => ClockKind::Awake,
        }
    }

    /// The instant, if the deadline is on the boot clock.
    #[must_use]
    pub const fn boot_at(&self) -> Option<BootDeadline> {
        match self.at {
            At::Boot(at) => Some(at),
            At::Awake(_) => None,
        }
    }

    /// The instant, if the deadline is on the awake clock.
    #[must_use]
    pub const fn awake_at(&self) -> Option<AwakeDeadline> {
        match self.at {
            At::Awake(at) => Some(at),
            At::Boot(_) => None,
        }
    }

    /// The doubt count of the deadline's clock latched when it was built.
    #[must_use]
    pub const fn latched_doubt(&self) -> u64 {
        self.latch
    }
}

/// How a wait ended (5.3, point 4). Each evaluation checks cancellation
/// first. If the token has fired, it returns without observing the signal
/// or clock. Otherwise it snapshots the signal and then reads the deadline's
/// clock and doubt count together. Clock failure or excess doubt takes
/// precedence over a passed deadline, which takes precedence over the
/// signal snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Waited {
    /// The cancel token fired.
    Cancelled,
    /// The deadline's clock could not be read, or its doubt count moved
    /// above the deadline's latch.
    ClockFailed,
    /// The deadline's clock reads at or past the deadline.
    DeadlinePassed,
    /// A raised signal snapshot was followed by a valid reading before the
    /// deadline, with doubt at or below the latch.
    Signalled,
}

/// A flag with a bounded list of registered wakers (5.3, point 3): a
/// signal its producer raises, or a cancel token its owner fires.
///
/// Raising sets the flag first, then wakes every registered waker, so no
/// wake is lost on either side of a waiter's check-to-sleep boundary.
pub trait Watch: Send + Sync {
    /// Whether the flag is set.
    fn is_set(&self) -> bool;

    /// Registers `waker`, to be woken when the flag is set.
    ///
    /// # Errors
    ///
    /// [`WatchFull`], never a panic, when the list is full.
    fn register(&self, waker: Arc<dyn Waker>) -> Result<Registration, WatchFull>;

    /// Removes the registration `register` returned.
    fn deregister(&self, registration: Registration);
}

/// What a watch wakes: a condition variable in production, a task record
/// under the simulator.
pub trait Waker: Send + Sync {
    /// Wakes the waiter.
    fn wake(&self);
}

/// A shared handle on a watch, for a token that outlives one borrow: what
/// a seam in a pure crate takes as its cancel token.
pub type WatchRef = Arc<dyn Watch>;

/// A registration of a waker with a watch: a number the watch assigns from
/// its own counter, so deregistration depends on no address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Registration(u64);

impl Registration {
    /// The registration numbered `number` by the watch that assigns it.
    #[must_use]
    pub const fn new(number: u64) -> Self {
        Self(number)
    }

    /// The number the watch assigned.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A watch's list of registered wakers is full: a defect, reported, never a
/// panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WatchFull;

impl fmt::Display for WatchFull {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the watch's waker list is full")
    }
}

impl Error for WatchFull {}
