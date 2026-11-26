// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A seeded [`Clock`] for tests and the simulator (coding standards rule
//! 7): three virtual clocks that move only when the test moves them.

use std::error::Error;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use ironafs_time::{
    AwakeInstant, BootInstant, Clock, ClockError, ClockKind, Deadline, Doubt, Stamp, Waited,
    WallTime, Watch,
};

use crate::SplitMix64;

#[cfg(test)]
mod tests;

/// Three virtual clocks (ADR 0004, C37, point 1) that move only when the
/// test moves them: a read takes no time.
///
/// [`SeededClock::advance`] moves all three; [`SeededClock::suspend`]
/// moves the boot clock and wall time but not the awake clock, as a
/// machine asleep does. Wall time can be set to any value, to model a jump.
/// Each clock can be made to fail its reads (C37, point 5), and moved back
/// ([`SeededClock::regress`]).
///
/// Like every `Clock` (seams and scheduling specification, 5.2, point 3,
/// and 5.3, point 2), it keeps the doubt counts and the episode count from
/// its scripted reads: a failed read moves both counts and opens an episode
/// if none is open, until a valid sample closes it; a regression of a
/// monotonic clock moves the boot count and counts an episode; a suspend,
/// or a wall-time jump, past one second (C37, point 4's proposed threshold;
/// a read here takes no time, so there is no sampling skew) counts an
/// episode. Its [`Clock::wait`] never blocks: it advances all three clocks
/// by the time left on the deadline's clock, with checked arithmetic, then
/// evaluates the outcome again; an overflow is `ClockFailed`.
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
    failing: Failing,
    doubt: Doubt,
    episodes: u64,
    /// An episode of failed reads is open until a valid sample returns.
    failing_episode: bool,
    /// What a read is judged against for a regression: the last valid
    /// sample's boot and awake values, lowered to a read's value once that
    /// read's regression is counted, so one regression counts once.
    boot_reference: Option<BootInstant>,
    awake_reference: Option<AwakeInstant>,
    /// The last valid sample, for suspends and wall jumps.
    last_sample: Option<(BootInstant, AwakeInstant, WallTime)>,
    /// A regression was counted since the last valid sample, so the next
    /// one is not compared with it.
    regressed_since_sample: bool,
}

/// Which clocks fail their reads.
#[derive(Clone, Copy, Debug, Default)]
struct Failing {
    boot: bool,
    awake: bool,
    wall: bool,
}

/// C37, point 4's proposed threshold for a suspend or a wall jump.
const THRESHOLD_NANOS: i128 = 1_000_000_000;

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
                failing: Failing::default(),
                doubt: Doubt { boot: 0, awake: 0 },
                episodes: 0,
                failing_episode: false,
                boot_reference: None,
                awake_reference: None,
                last_sample: None,
                regressed_since_sample: false,
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

    /// Moves `clock` back by `duration`: a regression.
    ///
    /// # Errors
    ///
    /// Returns [`TimeOverflow`], and moves nothing, if the clock would
    /// pass its origin (or, for wall time, its first value).
    pub fn regress(&self, clock: ClockKind, duration: Duration) -> Result<(), TimeOverflow> {
        let mut state = self.state();
        let back = u64::try_from(duration.as_nanos()).map_err(|_| TimeOverflow)?;
        match clock {
            ClockKind::Boot => {
                let nanos = state
                    .boot
                    .as_nanos()
                    .checked_sub(back)
                    .ok_or(TimeOverflow)?;
                state.boot = BootInstant::from_nanos(nanos);
            }
            ClockKind::Awake => {
                let nanos = state
                    .awake
                    .as_nanos()
                    .checked_sub(back)
                    .ok_or(TimeOverflow)?;
                state.awake = AwakeInstant::from_nanos(nanos);
            }
            ClockKind::Wall => state.wall = state.wall.checked_sub(duration).ok_or(TimeOverflow)?,
        }
        Ok(())
    }

    /// Makes every later read of `clock` fail while `failing` is true.
    pub fn set_failing(&self, clock: ClockKind, failing: bool) {
        let mut state = self.state();
        match clock {
            ClockKind::Boot => state.failing.boot = failing,
            ClockKind::Awake => state.failing.awake = failing,
            ClockKind::Wall => state.failing.wall = failing,
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

impl ClockState {
    /// A failed read: both counts move, and an episode opens if none is.
    fn fail(&mut self, clock: ClockKind) -> ClockError {
        // The counts cannot reach the end of 64 bits in any test; saturating
        // keeps a failure a failure.
        self.doubt.boot = self.doubt.boot.saturating_add(1);
        self.doubt.awake = self.doubt.awake.saturating_add(1);
        if !self.failing_episode {
            self.failing_episode = true;
            self.episodes = self.episodes.saturating_add(1);
        }
        ClockError::new(clock)
    }

    /// A regression of either monotonic clock: the boot count moves and one
    /// episode is counted.
    fn regressed(&mut self) {
        self.doubt.boot = self.doubt.boot.saturating_add(1);
        self.episodes = self.episodes.saturating_add(1);
        self.regressed_since_sample = true;
    }

    fn read_boot(&mut self) -> Result<BootInstant, ClockError> {
        if self.failing.boot {
            return Err(self.fail(ClockKind::Boot));
        }
        if self
            .boot_reference
            .is_some_and(|reference| self.boot < reference)
        {
            self.regressed();
            self.boot_reference = Some(self.boot);
        }
        Ok(self.boot)
    }

    fn read_awake(&mut self) -> Result<AwakeInstant, ClockError> {
        if self.failing.awake {
            return Err(self.fail(ClockKind::Awake));
        }
        if self
            .awake_reference
            .is_some_and(|reference| self.awake < reference)
        {
            self.regressed();
            self.awake_reference = Some(self.awake);
        }
        Ok(self.awake)
    }

    fn read_wall(&mut self) -> Result<WallTime, ClockError> {
        if self.failing.wall {
            return Err(self.fail(ClockKind::Wall));
        }
        Ok(self.wall)
    }

    /// One valid sample's own checks against the last valid sample: a
    /// suspend or a wall jump past the threshold counts an episode. A
    /// regression was already counted by the reads.
    fn check_sample(&mut self, boot: BootInstant, awake: AwakeInstant, wall: WallTime) {
        if let Some((last_boot, last_awake, last_wall)) = self.last_sample {
            let boot_grew = i128::from(boot.as_nanos()) - i128::from(last_boot.as_nanos());
            let awake_grew = i128::from(awake.as_nanos()) - i128::from(last_awake.as_nanos());
            let wall_moved = wall_nanos(wall) - wall_nanos(last_wall);
            // A regression is its own episode, counted by the reads; the
            // clocks are then not compared across it, even if they have
            // since moved past the last sample again.
            if !self.regressed_since_sample && boot_grew >= 0 && awake_grew >= 0 {
                if boot_grew - awake_grew > THRESHOLD_NANOS {
                    self.episodes = self.episodes.saturating_add(1);
                }
                if (wall_moved - boot_grew).abs() > THRESHOLD_NANOS {
                    self.episodes = self.episodes.saturating_add(1);
                }
            }
        }
        self.last_sample = Some((boot, awake, wall));
        self.regressed_since_sample = false;
        self.boot_reference = Some(boot);
        self.awake_reference = Some(awake);
        self.failing_episode = false;
    }
}

/// Wall time as signed nanoseconds since 1970, wide enough never to
/// overflow.
fn wall_nanos(wall: WallTime) -> i128 {
    i128::from(wall.seconds()) * 1_000_000_000 + i128::from(wall.subsec_nanos())
}

impl Clock for SeededClock {
    fn boot(&self) -> Result<BootInstant, ClockError> {
        self.state().read_boot()
    }

    fn awake(&self) -> Result<AwakeInstant, ClockError> {
        self.state().read_awake()
    }

    fn wall(&self) -> Result<WallTime, ClockError> {
        self.state().read_wall()
    }

    fn sample(&self) -> Result<Stamp, ClockError> {
        let mut state = self.state();
        let first = state.read_boot()?;
        let awake = state.read_awake()?;
        let wall = state.read_wall()?;
        let boot = state.read_boot()?;
        // Reads take no time here, so the bracket's two boot reads agree.
        let _ = first;
        state.check_sample(boot, awake, wall);
        Ok(Stamp {
            boot,
            awake,
            wall,
            doubt: state.doubt,
            episodes: state.episodes,
        })
    }

    fn wait(&self, signal: &dyn Watch, deadline: &Deadline, cancel: &dyn Watch) -> Waited {
        if let Some(outcome) = self.evaluate(signal, deadline, cancel) {
            return outcome;
        }
        let left = {
            let mut state = self.state();
            match (deadline.boot_at(), deadline.awake_at()) {
                (Some(at), _) => state.read_boot().map(|now| at.remaining(now)),
                (None, Some(at)) => state.read_awake().map(|now| at.remaining(now)),
                (None, None) => return Waited::ClockFailed,
            }
        };
        let Ok(left) = left else {
            return Waited::ClockFailed;
        };
        if self.advance(left).is_err() {
            return Waited::ClockFailed;
        }
        self.evaluate(signal, deadline, cancel)
            .unwrap_or(Waited::DeadlinePassed)
    }
}

impl SeededClock {
    /// One evaluation of a wait's outcome, in the order of the seams and
    /// scheduling specification, 5.3, point 4; `None` while undecided.
    fn evaluate(
        &self,
        signal: &dyn Watch,
        deadline: &Deadline,
        cancel: &dyn Watch,
    ) -> Option<Waited> {
        if cancel.is_set() {
            return Some(Waited::Cancelled);
        }
        let raised = signal.is_set();
        let mut state = self.state();
        let passed = match (deadline.boot_at(), deadline.awake_at()) {
            (Some(at), _) => state
                .read_boot()
                .map(|now| (at.has_passed(now), state.doubt.boot)),
            (None, Some(at)) => state
                .read_awake()
                .map(|now| (at.has_passed(now), state.doubt.awake)),
            (None, None) => return Some(Waited::ClockFailed),
        };
        drop(state);
        match passed {
            Err(_) => Some(Waited::ClockFailed),
            Ok((_, doubt)) if doubt > deadline.latched_doubt() => Some(Waited::ClockFailed),
            Ok((true, _)) => Some(Waited::DeadlinePassed),
            Ok((false, _)) if raised => Some(Waited::Signalled),
            Ok((false, _)) => None,
        }
    }
}
