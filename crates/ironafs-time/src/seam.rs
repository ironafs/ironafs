// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Clock` and `Rng` interfaces (ADR 0004, C3, point 3) and their
//! errors.
//!
//! Operation code holds them as injected values; it never reads a clock or
//! draws random bytes any other way (coding standards rule 7). Pure crates
//! take time and random bytes as inputs and call neither (C3, point 1).

use core::error::Error;
use core::fmt;

use crate::{AwakeInstant, BootInstant, Deadline, Stamp, Waited, WallTime, Watch};

#[cfg(test)]
mod tests;

/// The three clocks of ADR 0004, C37, point 1: boot, awake and wall.
///
/// Implementations read the operating system's clocks in production and
/// virtual clocks under the simulator. The interface has no way to set a
/// clock: the client never sets the system clock (C37, point 6).
///
/// # When a read fails
///
/// A read can fail, and the caller's duty then reaches past the clock that
/// failed. ADR 0004, C37, point 5: "Startup is refused without a valid
/// pair. If a later read fails, outstanding deadlines expire, callbacks
/// become suspect, one event is logged per episode, and no new deadline is
/// set until a valid sample returns." Point 4 adds: "A regression of a
/// monotonic clock, or a failed sample, also treats every boot-clock
/// deadline (callbacks, tokens, location lifetimes, locks) as possibly
/// expired: a lock counts as lost until it is established again with the
/// server, and a token is checked again before its next use."
///
/// So a failed read of any of the three clocks expires every outstanding
/// deadline on both monotonic clocks and marks callbacks suspect, not only
/// the deadlines on the clock that failed. One successful read afterwards
/// does not end the episode: a new deadline is set only from a valid
/// sample, which point 4 defines as an awake read bracketed by two boot
/// reads, all three succeeding.
///
/// The `Clock` implementation keeps the episode and the doubt (seams and
/// scheduling specification, 5.3, point 2): it remembers its last valid
/// sample, and [`Clock::sample`] and every read move two doubt counts, one
/// per monotonic clock, and an episode count. A failed read moves both
/// counts; a regression of either monotonic clock moves only the boot
/// count. A [`Deadline`] latches its own clock's count when it is built,
/// so whatever moves that count after it was built fails it at its next
/// evaluation, even if the sample taken then is good: a failure fails
/// every deadline, a regression only boot-clock ones.
/// [`MonotonicDeadline::after`](crate::MonotonicDeadline::after) builds a
/// bare instant with no latch; seams take a [`Deadline`].
///
/// ```
/// use ironafs_time::{
///     AwakeInstant, BootInstant, Clock, ClockError, Deadline, Doubt, Stamp, WallTime, Waited,
///     Watch,
/// };
///
/// /// A clock stopped at the origin of each timeline, which never fails.
/// struct Stopped;
///
/// impl Clock for Stopped {
///     fn boot(&self) -> Result<BootInstant, ClockError> {
///         Ok(BootInstant::from_nanos(0))
///     }
///     fn awake(&self) -> Result<AwakeInstant, ClockError> {
///         Ok(AwakeInstant::from_nanos(0))
///     }
///     fn wall(&self) -> Result<WallTime, ClockError> {
///         Ok(WallTime::UNIX_EPOCH)
///     }
///     fn sample(&self) -> Result<Stamp, ClockError> {
///         Ok(Stamp {
///             boot: self.boot()?,
///             awake: self.awake()?,
///             wall: self.wall()?,
///             doubt: Doubt { boot: 0, awake: 0 },
///             episodes: 0,
///         })
///     }
///     fn wait(&self, signal: &dyn Watch, deadline: &Deadline, cancel: &dyn Watch) -> Waited {
///         // The outcomes in their order. Undecided, a wait goes on
///         // waiting; time never moves here, so a deadline still ahead never
///         // passes, and this wait blocks for ever.
///         let origin_passed = deadline
///             .boot_at()
///             .is_some_and(|at| at.has_passed(BootInstant::from_nanos(0)))
///             || deadline
///                 .awake_at()
///                 .is_some_and(|at| at.has_passed(AwakeInstant::from_nanos(0)));
///         if cancel.is_set() {
///             Waited::Cancelled
///         } else if origin_passed {
///             Waited::DeadlinePassed
///         } else if signal.is_set() {
///             Waited::Signalled
///         } else {
///             loop {
///                 std::thread::park();
///             }
///         }
///     }
/// }
///
/// let clock: &dyn Clock = &Stopped;
/// assert_eq!(clock.wall(), Ok(WallTime::UNIX_EPOCH));
/// ```
pub trait Clock: Send + Sync {
    /// Reads the boot clock, which counts time the machine was suspended.
    ///
    /// # Errors
    ///
    /// Returns a [`ClockError`] for [`ClockKind::Boot`] if the clock cannot
    /// be read.
    fn boot(&self) -> Result<BootInstant, ClockError>;

    /// Reads the awake clock, which excludes time the machine was
    /// suspended.
    ///
    /// # Errors
    ///
    /// Returns a [`ClockError`] for [`ClockKind::Awake`] if the clock
    /// cannot be read.
    fn awake(&self) -> Result<AwakeInstant, ClockError>;

    /// Reads wall time.
    ///
    /// # Errors
    ///
    /// Returns a [`ClockError`] for [`ClockKind::Wall`] if the clock cannot
    /// be read.
    fn wall(&self) -> Result<WallTime, ClockError>;

    /// Takes one sample as ADR 0004, C37, point 4, does (an awake read and a
    /// wall read bracketed by two boot reads), and applies the checks of the
    /// seams and scheduling specification, 5.3, point 2: a failed read moves
    /// both doubt counts and opens an episode if none is open; a regression
    /// of either monotonic clock moves the boot count and counts an episode;
    /// a detected suspend or a wall jump past the threshold counts an
    /// episode.
    ///
    /// # Errors
    ///
    /// Returns the [`ClockError`] of the first read that failed. No
    /// [`Stamp`] exists then, so no deadline can be built from it.
    fn sample(&self) -> Result<Stamp, ClockError>;

    /// Waits until `signal` is raised, `cancel` fires or `deadline` passes:
    /// the one blocking primitive of the core (5.3, point 3). Every wake,
    /// spurious or not, evaluates the outcome in the order of [`Waited`],
    /// the deadline's latched doubt included.
    fn wait(&self, signal: &dyn Watch, deadline: &Deadline, cancel: &dyn Watch) -> Waited;
}

/// A source of random bytes (ADR 0004, C3, point 3).
///
/// In production it is the cryptographic random source behind the
/// cryptography interface (C5, point 1), never a seeded generator; under
/// test and in the simulator it is seeded, so a run replays from its seed.
///
/// A source that fails does not fall back to another: the operation that
/// asked fails, or the process refuses to start.
///
/// ```
/// use ironafs_time::{Rng, RngError};
///
/// /// A source that has failed for good.
/// struct Exhausted;
///
/// impl Rng for Exhausted {
///     fn fill(&self, _bytes: &mut [u8]) -> Result<(), RngError> {
///         Err(RngError::new())
///     }
/// }
///
/// let mut key = [0_u8; 16];
/// assert_eq!(Exhausted.fill(&mut key), Err(RngError::new()));
/// ```
pub trait Rng: Send + Sync {
    /// Fills all of `bytes` with random bytes.
    ///
    /// # Errors
    ///
    /// Returns an [`RngError`] if the source cannot supply them. The
    /// contents of `bytes` are then unspecified and must not be used.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError>;
}

/// Which of the three clocks of [`Clock`] a value or an error is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClockKind {
    /// The boot clock, which counts suspend.
    Boot,
    /// The awake clock, which excludes suspend.
    Awake,
    /// Wall time.
    Wall,
}

/// A clock could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockError {
    clock: ClockKind,
}

impl ClockError {
    /// The error for a failed read of `clock`.
    #[must_use]
    pub const fn new(clock: ClockKind) -> Self {
        Self { clock }
    }

    /// The clock that could not be read.
    #[must_use]
    pub const fn clock(self) -> ClockKind {
        self.clock
    }
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.clock {
            ClockKind::Boot => "the boot clock could not be read",
            ClockKind::Awake => "the awake clock could not be read",
            ClockKind::Wall => "wall time could not be read",
        })
    }
}

impl Error for ClockError {}

/// The random source could not supply bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RngError {
    // Private, so more detail can be added without breaking callers.
    _private: (),
}

impl RngError {
    /// The error for a source that could not supply bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl fmt::Display for RngError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the random source could not supply bytes")
    }
}

impl Error for RngError {}
