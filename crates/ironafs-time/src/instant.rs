// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Instants and deadlines on the boot and awake clocks (ADR 0004, C37,
//! point 1): unsigned 64-bit nanoseconds since an origin the clock chooses,
//! with checked arithmetic (C38, points 1 and 4).
//!
//! One generic type serves both clocks, and a marker type names the clock,
//! so the compiler refuses a deadline checked against the wrong clock.

use core::fmt::Debug;
use core::hash::Hash;
use core::marker::PhantomData;
use core::time::Duration;

#[cfg(test)]
mod tests;

/// A monotonic clock's marker type: [`Boot`] or [`Awake`].
pub trait MonotonicClock: Copy + Debug + Eq + Ord + Hash {}

/// Marks a value as being on the boot clock, which counts time the machine
/// was suspended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Boot {}

/// Marks a value as being on the awake clock, which excludes time the
/// machine was suspended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Awake {}

impl MonotonicClock for Boot {}
impl MonotonicClock for Awake {}

/// An instant on the boot clock (ADR 0004, C37, point 1).
///
/// ```
/// use core::time::Duration;
/// use ironafs_time::BootInstant;
///
/// let start = BootInstant::from_nanos(1_000);
/// let later = start.checked_add(Duration::from_nanos(500)).unwrap();
/// assert_eq!(later.as_nanos(), 1_500);
/// assert_eq!(later.checked_duration_since(start), Some(Duration::from_nanos(500)));
/// ```
pub type BootInstant = MonotonicInstant<Boot>;

/// An instant on the awake clock (ADR 0004, C37, point 1).
///
/// ```
/// use ironafs_time::AwakeInstant;
///
/// let instant = AwakeInstant::from_nanos(42);
/// assert_eq!(instant.as_nanos(), 42);
/// ```
pub type AwakeInstant = MonotonicInstant<Awake>;

/// A deadline on the boot clock: for every lifetime a server grants
/// (callbacks, tokens, location lifetimes, locks), so a night asleep counts
/// against it (ADR 0004, C37, point 1).
///
/// ```
/// use core::time::Duration;
/// use ironafs_time::{BootDeadline, BootInstant};
///
/// let now = BootInstant::from_nanos(0);
/// let deadline = BootDeadline::after(now, Duration::from_secs(1)).unwrap();
/// assert!(!deadline.has_passed(now));
/// assert!(deadline.has_passed(BootInstant::from_nanos(1_000_000_000)));
/// ```
pub type BootDeadline = MonotonicDeadline<Boot>;

/// A deadline on the awake clock: for round-trip and retransmission timers,
/// which must not fire a burst on waking (ADR 0004, C37, point 1).
///
/// ```
/// use core::time::Duration;
/// use ironafs_time::{AwakeDeadline, AwakeInstant};
///
/// let now = AwakeInstant::from_nanos(0);
/// let deadline = AwakeDeadline::after(now, Duration::from_millis(5)).unwrap();
/// assert_eq!(deadline.remaining(now), Duration::from_millis(5));
/// ```
pub type AwakeDeadline = MonotonicDeadline<Awake>;

/// An instant on the monotonic clock `C` ([`Boot`] or [`Awake`]):
/// nanoseconds since that clock's origin. Use it as [`BootInstant`] or
/// [`AwakeInstant`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicInstant<C: MonotonicClock> {
    nanos: u64,
    clock: PhantomData<C>,
}

impl<C: MonotonicClock> MonotonicInstant<C> {
    /// The instant `nanos` nanoseconds after the clock's origin.
    #[must_use]
    pub const fn from_nanos(nanos: u64) -> Self {
        Self {
            nanos,
            clock: PhantomData,
        }
    }

    /// Nanoseconds since the clock's origin.
    #[must_use]
    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// The instant `duration` later, or `None` if it does not fit in 64-bit
    /// nanoseconds.
    #[must_use]
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let delta = u64::try_from(duration.as_nanos()).ok()?;
        self.nanos.checked_add(delta).map(Self::from_nanos)
    }

    /// The time elapsed from `earlier` to this instant, or `None` if
    /// `earlier` is later than this instant.
    #[must_use]
    pub fn checked_duration_since(self, earlier: Self) -> Option<Duration> {
        self.nanos
            .checked_sub(earlier.nanos)
            .map(Duration::from_nanos)
    }
}

/// A deadline on the monotonic clock `C` ([`Boot`] or [`Awake`]): the
/// instant at which it passes. Use it as [`BootDeadline`] or
/// [`AwakeDeadline`]. No deadline counts on wall time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicDeadline<C: MonotonicClock> {
    at: MonotonicInstant<C>,
}

impl<C: MonotonicClock> MonotonicDeadline<C> {
    /// The deadline that passes at `instant`.
    #[must_use]
    pub const fn at(instant: MonotonicInstant<C>) -> Self {
        Self { at: instant }
    }

    /// The deadline `duration` after `now`, or `None` if that instant does
    /// not fit in 64-bit nanoseconds. A caller that gets `None` sets no
    /// deadline; it never clamps one.
    ///
    /// `now` must come from a valid sample: after any failed clock read, no
    /// new deadline is set until a valid sample returns (ADR 0004, C37,
    /// point 5; see [`Clock`](crate::Clock)). This function cannot tell,
    /// so the caller keeps that rule.
    #[must_use]
    pub fn after(now: MonotonicInstant<C>, duration: Duration) -> Option<Self> {
        now.checked_add(duration).map(Self::at)
    }

    /// The instant at which the deadline passes.
    #[must_use]
    pub const fn instant(self) -> MonotonicInstant<C> {
        self.at
    }

    /// Whether the deadline has passed at `now`: it passes at its instant,
    /// not after it.
    #[must_use]
    pub fn has_passed(self, now: MonotonicInstant<C>) -> bool {
        now.nanos >= self.at.nanos
    }

    /// The time left from `now` until the deadline passes; zero once it has
    /// passed.
    #[must_use]
    pub fn remaining(self, now: MonotonicInstant<C>) -> Duration {
        self.at
            .checked_duration_since(now)
            .unwrap_or(Duration::ZERO)
    }
}
