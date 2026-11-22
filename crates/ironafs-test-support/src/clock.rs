// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A seeded [`Clock`] for tests and the simulator (coding standards rule
//! 7): three virtual clocks that move only when the test moves them.

use std::error::Error;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ironafs_time::{AwakeInstant, BootInstant, Clock, ClockError, ClockKind, WallTime};

use crate::SplitMix64;

#[cfg(test)]
mod tests;

/// Three virtual clocks (ADR 0004, C37, point 1) that move only when the
/// test moves them: a read takes no time.
///
/// [`SeededClock::advance`] moves all three; [`SeededClock::suspend`]
/// moves the boot clock and wall time but not the awake clock, as a
/// machine asleep does. Wall time can be set to any value, to model a jump.
/// Each clock can be made to fail its reads (C37, point 5).
///
/// A clock built [from a seed](SeededClock::from_seed) starts at values
/// drawn from it, so code under test cannot rely on clocks starting at
/// zero, and the same seed always gives the same start.
///
/// ```
/// use std::time::Duration;
///
/// use ironafs_test_support::SeededClock;
/// use ironafs_time::{AwakeInstant, BootInstant, Clock, WallTime};
///
/// let clock = SeededClock::starting_at(
///     BootInstant::from_nanos(0),
///     AwakeInstant::from_nanos(0),
///     WallTime::UNIX_EPOCH,
/// );
/// clock.suspend(Duration::from_secs(3_600)).unwrap();
/// assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(3_600_000_000_000)));
/// assert_eq!(clock.awake(), Ok(AwakeInstant::from_nanos(0)));
/// ```
#[derive(Debug)]
pub struct SeededClock {
    state: Mutex<ClockState>,
}

#[derive(Clone, Copy, Debug)]
struct ClockState {
    boot: BootInstant,
    awake: AwakeInstant,
    wall: WallTime,
    boot_failing: bool,
    awake_failing: bool,
    wall_failing: bool,
}

/// A clock was moved past the last value its type can hold. The clock is
/// left as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeOverflow;

impl fmt::Display for TimeOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the clock would move past the last time it can hold")
    }
}

impl Error for TimeOverflow {}

impl SeededClock {
    /// A clock whose three clocks start at the given values.
    #[must_use]
    pub fn starting_at(boot: BootInstant, awake: AwakeInstant, wall: WallTime) -> Self {
        Self {
            state: Mutex::new(ClockState {
                boot,
                awake,
                wall,
                boot_failing: false,
                awake_failing: false,
                wall_failing: false,
            }),
        }
    }

    /// A clock whose start is drawn from `seed`.
    ///
    /// The awake clock starts below 2^62 nanoseconds (about 146 years), the
    /// boot clock up to 2^40 nanoseconds (about 18 minutes) of suspend
    /// past it, and wall time below 2^33 seconds after 1970 (about the
    /// year 2242), so a test can still run for centuries of virtual time.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        let mut generator = SplitMix64::new(seed);
        let awake = generator.next_u64() >> 2;
        let suspended = generator.next_u64() >> 24;
        let wall_seconds = generator.next_u64() >> 31;
        let wall_nanos = generator.next_u64() % 1_000_000_000;
        // Each value is shifted or reduced into range above, so none of
        // these can fail; falling back keeps the constructor total.
        let boot = awake.checked_add(suspended).unwrap_or(awake);
        let wall = i64::try_from(wall_seconds)
            .ok()
            .zip(u32::try_from(wall_nanos).ok())
            .and_then(|(seconds, nanos)| WallTime::from_unix(seconds, nanos))
            .unwrap_or(WallTime::UNIX_EPOCH);
        Self::starting_at(
            BootInstant::from_nanos(boot),
            AwakeInstant::from_nanos(awake),
            wall,
        )
    }

    /// Moves all three clocks forward by `duration`.
    ///
    /// # Errors
    ///
    /// Returns [`TimeOverflow`], and moves nothing, if any clock would pass
    /// its last value.
    pub fn advance(&self, duration: Duration) -> Result<(), TimeOverflow> {
        self.move_by(duration, true)
    }

    /// Moves the boot clock and wall time forward by `duration`, and not
    /// the awake clock: a suspend.
    ///
    /// # Errors
    ///
    /// Returns [`TimeOverflow`], and moves nothing, if any clock would pass
    /// its last value.
    pub fn suspend(&self, duration: Duration) -> Result<(), TimeOverflow> {
        self.move_by(duration, false)
    }

    /// Sets wall time to `wall`, forwards or backwards: a jump.
    pub fn set_wall(&self, wall: WallTime) {
        self.state().wall = wall;
    }

    /// Makes every later read of `clock` fail while `failing` is true.
    pub fn set_failing(&self, clock: ClockKind, failing: bool) {
        let mut state = self.state();
        match clock {
            ClockKind::Boot => state.boot_failing = failing,
            ClockKind::Awake => state.awake_failing = failing,
            ClockKind::Wall => state.wall_failing = failing,
        }
    }

    /// Moves the boot clock and wall time by `duration`, and the awake
    /// clock too if `awake` is true; all or nothing.
    fn move_by(&self, duration: Duration, awake: bool) -> Result<(), TimeOverflow> {
        let mut state = self.state();
        let boot = state.boot.checked_add(duration).ok_or(TimeOverflow)?;
        let wall = state.wall.checked_add(duration).ok_or(TimeOverflow)?;
        let awake = if awake {
            state.awake.checked_add(duration).ok_or(TimeOverflow)?
        } else {
            state.awake
        };
        state.boot = boot;
        state.awake = awake;
        state.wall = wall;
        Ok(())
    }

    fn state(&self) -> MutexGuard<'_, ClockState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Clock for SeededClock {
    fn boot(&self) -> Result<BootInstant, ClockError> {
        let state = self.state();
        read(state.boot, state.boot_failing, ClockKind::Boot)
    }

    fn awake(&self) -> Result<AwakeInstant, ClockError> {
        let state = self.state();
        read(state.awake, state.awake_failing, ClockKind::Awake)
    }

    fn wall(&self) -> Result<WallTime, ClockError> {
        let state = self.state();
        read(state.wall, state.wall_failing, ClockKind::Wall)
    }
}

/// A read of `clock`: its `value`, or its error while it is failing.
fn read<T>(value: T, failing: bool, clock: ClockKind) -> Result<T, ClockError> {
    if failing {
        Err(ClockError::new(clock))
    } else {
        Ok(value)
    }
}
