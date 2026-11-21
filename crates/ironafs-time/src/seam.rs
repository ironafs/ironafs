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

use crate::{AwakeInstant, BootInstant, WallTime};

#[cfg(test)]
mod tests;

/// The three clocks of ADR 0004, C37, point 1: boot, awake and wall.
///
/// Implementations read the operating system's clocks in production and
/// virtual clocks under the simulator. The interface has no way to set a
/// clock: the client never sets the system clock (C37, point 6).
///
/// A read can fail. The caller then treats every outstanding deadline on
/// that clock as expired and sets no new one until a read succeeds again
/// (C37, point 5).
///
/// ```
/// use ironafs_time::{AwakeInstant, BootInstant, Clock, ClockError, WallTime};
///
/// /// A clock stopped at the origin of each timeline.
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
