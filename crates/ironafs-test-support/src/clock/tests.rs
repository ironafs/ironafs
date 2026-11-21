// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seeded clock: reads take no time, advance and suspend
//! move the right clocks, overflow moves nothing, and reads fail on demand.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ironafs_time::{
    AwakeInstant, BootInstant, Clock, ClockError, ClockKind, Deadline, Doubt, Registration, Stamp,
    Waited, Waker, WallTime, Watch, WatchFull,
};

use super::{SeededClock, TimeOverflow};

fn at(boot: u64, awake: u64, wall_seconds: i64) -> SeededClock {
    SeededClock::starting_at(
        BootInstant::from_nanos(boot),
        AwakeInstant::from_nanos(awake),
        WallTime::from_unix(wall_seconds, 0).unwrap(),
    )
}

#[track_caller]
fn assert_reads(clock: &SeededClock, boot: u64, awake: u64, wall: WallTime) {
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(boot)));
    assert_eq!(clock.awake(), Ok(AwakeInstant::from_nanos(awake)));
    assert_eq!(clock.wall(), Ok(wall));
}

fn wall(seconds: i64, nanos: u32) -> WallTime {
    WallTime::from_unix(seconds, nanos).unwrap()
}

#[test]
fn reads_take_no_time() {
    let clock = at(10, 5, 100);
    assert_reads(&clock, 10, 5, wall(100, 0));
    assert_reads(&clock, 10, 5, wall(100, 0));
}

#[test]
fn advance_moves_all_three_clocks() {
    let clock = at(10, 5, 100);
    clock.advance(Duration::new(1, 7)).unwrap();
    assert_reads(&clock, 1_000_000_017, 1_000_000_012, wall(101, 7));
}

#[test]
fn suspend_moves_boot_and_wall_but_not_awake() {
    let clock = at(10, 5, 100);
    clock.suspend(Duration::from_hours(8)).unwrap();
    assert_reads(&clock, 28_800_000_000_010, 5, wall(28_900, 0));
}

#[test]
fn advance_past_the_last_nanosecond_moves_nothing() {
    let clock = at(u64::MAX - 1, 5, 100);
    assert_eq!(clock.advance(Duration::from_nanos(2)), Err(TimeOverflow));
    assert_reads(&clock, u64::MAX - 1, 5, wall(100, 0));
}

#[test]
fn suspend_past_the_last_wall_second_moves_nothing() {
    let clock = SeededClock::starting_at(
        BootInstant::from_nanos(0),
        AwakeInstant::from_nanos(0),
        wall(i64::MAX, 999_999_999),
    );
    assert_eq!(clock.suspend(Duration::from_nanos(1)), Err(TimeOverflow));
    assert_reads(&clock, 0, 0, wall(i64::MAX, 999_999_999));
}

#[test]
fn set_wall_jumps_wall_time_only() {
    let clock = at(10, 5, 100);
    clock.set_wall(wall(-50, 3));
    assert_reads(&clock, 10, 5, wall(-50, 3));
}

#[test]
fn failing_clock_reads_return_its_error_until_cleared() {
    let clock = at(10, 5, 100);
    clock.set_failing(ClockKind::Awake, true);
    assert_eq!(clock.awake(), Err(ClockError::new(ClockKind::Awake)));
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(10)));
    clock.set_failing(ClockKind::Boot, true);
    clock.set_failing(ClockKind::Wall, true);
    assert_eq!(clock.boot(), Err(ClockError::new(ClockKind::Boot)));
    assert_eq!(clock.wall(), Err(ClockError::new(ClockKind::Wall)));
    clock.set_failing(ClockKind::Awake, false);
    assert_eq!(clock.awake(), Ok(AwakeInstant::from_nanos(5)));
}

#[test]
fn from_seed_gives_the_same_start_for_the_same_seed() {
    let first = SeededClock::from_seed(11);
    let second = SeededClock::from_seed(11);
    assert_eq!(first.boot(), second.boot());
    assert_eq!(first.awake(), second.awake());
    assert_eq!(first.wall(), second.wall());
}

#[test]
fn from_seed_starts_with_boot_at_or_past_awake() {
    for seed in 0..64 {
        let clock = SeededClock::from_seed(seed);
        assert!(clock.boot().unwrap().as_nanos() >= clock.awake().unwrap().as_nanos());
    }
}

#[test]
fn from_seed_starts_differ_between_seeds() {
    let first = SeededClock::from_seed(1);
    let second = SeededClock::from_seed(2);
    assert_ne!(first.awake(), second.awake());
    assert_ne!(first.wall(), second.wall());
}

#[test]
fn from_seed_leaves_room_to_run() {
    // Starts stay low enough that a test can advance by a century.
    for seed in 0..64 {
        let clock = SeededClock::from_seed(seed);
        clock.advance(Duration::from_hours(100 * 366 * 24)).unwrap();
    }
}

/// A flag a test sets by hand. The seeded clock's wait never blocks, so it
/// registers no waker.
#[derive(Default)]
struct Flag(AtomicBool);

impl Flag {
    fn set() -> Self {
        Self(AtomicBool::new(true))
    }
}

impl Watch for Flag {
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn register(&self, _waker: Arc<dyn Waker>) -> Result<Registration, WatchFull> {
        Ok(Registration::new(0))
    }
    fn deregister(&self, _registration: Registration) {}
}

#[track_caller]
fn sample(clock: &SeededClock) -> Stamp {
    clock.sample().unwrap()
}

#[track_caller]
fn counts(clock: &SeededClock) -> (Doubt, u64) {
    let stamp = sample(clock);
    (stamp.doubt, stamp.episodes)
}

const NO_DOUBT: Doubt = Doubt { boot: 0, awake: 0 };

#[test]
fn sample_reads_all_three_clocks_with_no_doubt() {
    let clock = at(10, 5, 100);
    assert_eq!(
        sample(&clock),
        Stamp {
            boot: BootInstant::from_nanos(10),
            awake: AwakeInstant::from_nanos(5),
            wall: wall(100, 0),
            doubt: NO_DOUBT,
            episodes: 0,
        }
    );
}

#[test]
fn failed_read_moves_both_counts_and_opens_one_episode() {
    let clock = at(10, 5, 100);
    clock.set_failing(ClockKind::Wall, true);
    assert_eq!(clock.sample(), Err(ClockError::new(ClockKind::Wall)));
    // A second failure in the same episode moves the counts again but
    // opens no episode.
    assert_eq!(clock.awake(), Ok(AwakeInstant::from_nanos(5)));
    assert_eq!(clock.wall(), Err(ClockError::new(ClockKind::Wall)));
    clock.set_failing(ClockKind::Wall, false);
    assert_eq!(counts(&clock), (Doubt { boot: 2, awake: 2 }, 1));
    // A valid sample closed the episode: the next failure opens another.
    clock.set_failing(ClockKind::Boot, true);
    assert_eq!(clock.boot(), Err(ClockError::new(ClockKind::Boot)));
    clock.set_failing(ClockKind::Boot, false);
    assert_eq!(counts(&clock), (Doubt { boot: 3, awake: 3 }, 2));
}

#[test]
fn boot_regression_moves_the_boot_count_only() {
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let _ = sample(&clock);
    clock
        .regress(ClockKind::Boot, Duration::from_secs(2))
        .unwrap();
    assert_eq!(counts(&clock), (Doubt { boot: 1, awake: 0 }, 1));
    // Counted once: the next sample finds no new regression.
    assert_eq!(counts(&clock), (Doubt { boot: 1, awake: 0 }, 1));
}

#[test]
fn awake_regression_moves_the_boot_count_only() {
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let _ = sample(&clock);
    clock
        .regress(ClockKind::Awake, Duration::from_secs(2))
        .unwrap();
    assert_eq!(counts(&clock), (Doubt { boot: 1, awake: 0 }, 1));
}

#[test]
fn regression_seen_by_a_single_read_is_counted_once() {
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let _ = sample(&clock);
    clock
        .regress(ClockKind::Boot, Duration::from_nanos(1))
        .unwrap();
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(4_999_999_999)));
    assert_eq!(counts(&clock), (Doubt { boot: 1, awake: 0 }, 1));
}

#[test]
fn suspend_past_the_threshold_is_one_episode_and_moves_no_count() {
    let clock = at(10, 5, 100);
    let _ = sample(&clock);
    clock.suspend(Duration::from_secs(1)).unwrap();
    assert_eq!(counts(&clock), (NO_DOUBT, 0));
    clock.suspend(Duration::from_millis(1_001)).unwrap();
    assert_eq!(counts(&clock), (NO_DOUBT, 1));
}

#[test]
fn wall_jump_past_the_threshold_is_one_episode_and_moves_no_count() {
    let clock = at(10, 5, 100);
    let _ = sample(&clock);
    clock.set_wall(wall(101, 0));
    assert_eq!(counts(&clock), (NO_DOUBT, 0));
    clock.set_wall(wall(99, 999_999_999));
    assert_eq!(counts(&clock), (NO_DOUBT, 1));
    clock.set_wall(wall(101, 0));
    assert_eq!(counts(&clock), (NO_DOUBT, 2));
}

#[test]
fn regress_below_the_origin_moves_nothing() {
    let clock = at(10, 5, 100);
    assert_eq!(
        clock.regress(ClockKind::Awake, Duration::from_nanos(6)),
        Err(TimeOverflow)
    );
    assert_reads(&clock, 10, 5, wall(100, 0));
}

/// A boot deadline `after` from a fresh sample: these tests stand in for a
/// sampling function, so the constructor's ban is expected here.
#[expect(
    clippy::disallowed_methods,
    reason = "the seeded clock's tests build deadlines from a fresh sample, as a sampling function does"
)]
fn boot_deadline(clock: &SeededClock, after: Duration) -> Deadline {
    let now = sample(clock);
    Deadline::boot_after(&now, &now, after).unwrap()
}

/// An awake deadline `after` from a fresh sample, as `boot_deadline`.
#[expect(
    clippy::disallowed_methods,
    reason = "the seeded clock's tests build deadlines from a fresh sample, as a sampling function does"
)]
fn awake_deadline(clock: &SeededClock, after: Duration) -> Deadline {
    let now = sample(clock);
    Deadline::awake_after(&now, &now, after).unwrap()
}

#[test]
fn wait_advances_to_the_deadline_and_reports_it_passed() {
    let clock = at(10, 5, 100);
    let deadline = boot_deadline(&clock, Duration::from_secs(3));
    let outcome = clock.wait(&Flag::default(), &deadline, &Flag::default());
    assert_eq!(outcome, Waited::DeadlinePassed);
    assert_reads(&clock, 3_000_000_010, 3_000_000_005, wall(103, 0));
}

#[test]
fn wait_on_an_awake_deadline_advances_by_the_awake_time_left() {
    let clock = at(10, 5, 100);
    let deadline = awake_deadline(&clock, Duration::from_nanos(7));
    let outcome = clock.wait(&Flag::default(), &deadline, &Flag::default());
    assert_eq!(outcome, Waited::DeadlinePassed);
    assert_reads(&clock, 17, 12, wall(100, 7));
}

#[test]
fn wait_on_a_raised_signal_takes_no_time() {
    let clock = at(10, 5, 100);
    let deadline = boot_deadline(&clock, Duration::from_secs(3));
    assert_eq!(
        clock.wait(&Flag::set(), &deadline, &Flag::default()),
        Waited::Signalled
    );
    assert_reads(&clock, 10, 5, wall(100, 0));
}

#[test]
fn wait_outcomes_keep_their_order() {
    let clock = at(10, 5, 100);
    let passed = boot_deadline(&clock, Duration::ZERO);
    // Cancelled before everything, passed before signalled.
    assert_eq!(
        clock.wait(&Flag::set(), &passed, &Flag::set()),
        Waited::Cancelled
    );
    assert_eq!(
        clock.wait(&Flag::set(), &passed, &Flag::default()),
        Waited::DeadlinePassed
    );
    // A failing clock before a passed deadline.
    clock.set_failing(ClockKind::Boot, true);
    assert_eq!(
        clock.wait(&Flag::set(), &passed, &Flag::default()),
        Waited::ClockFailed
    );
}

#[test]
fn wait_on_a_latched_deadline_fails_after_the_fault_clears() {
    let clock = at(10, 5, 100);
    let deadline = boot_deadline(&clock, Duration::from_secs(3));
    clock.set_failing(ClockKind::Awake, true);
    assert_eq!(clock.awake(), Err(ClockError::new(ClockKind::Awake)));
    clock.set_failing(ClockKind::Awake, false);
    assert_eq!(
        clock.wait(&Flag::set(), &deadline, &Flag::default()),
        Waited::ClockFailed
    );
}

#[test]
fn wait_after_a_regression_fails_boot_deadlines_and_not_awake_ones() {
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let boot = boot_deadline(&clock, Duration::from_secs(3));
    let awake = awake_deadline(&clock, Duration::from_secs(3));
    clock
        .regress(ClockKind::Boot, Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        clock.wait(&Flag::default(), &boot, &Flag::default()),
        Waited::ClockFailed
    );
    assert_eq!(
        clock.wait(&Flag::default(), &awake, &Flag::default()),
        Waited::DeadlinePassed
    );
}

#[test]
fn wait_whose_advance_overflows_fails_and_moves_nothing() {
    // The awake deadline is 5 ns away, but the boot clock has room for 2.
    let clock = at(u64::MAX - 2, 0, 100);
    let deadline = awake_deadline(&clock, Duration::from_nanos(5));
    assert_eq!(
        clock.wait(&Flag::default(), &deadline, &Flag::default()),
        Waited::ClockFailed
    );
    assert_reads(&clock, u64::MAX - 2, 0, wall(100, 0));
}

#[test]
fn regression_is_judged_against_the_last_valid_sample_not_a_later_read() {
    // A read after the sample moves no reference: going back to a value
    // still past the last valid sample is no regression (5.3, point 2).
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let _ = sample(&clock);
    clock.advance(Duration::from_secs(2)).unwrap();
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(7_000_000_000)));
    assert_eq!(clock.awake(), Ok(AwakeInstant::from_nanos(7_000_000_000)));
    clock
        .regress(ClockKind::Boot, Duration::from_secs(1))
        .unwrap();
    clock
        .regress(ClockKind::Awake, Duration::from_secs(1))
        .unwrap();
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(6_000_000_000)));
    assert_eq!(counts(&clock), (NO_DOUBT, 0));
}

#[test]
fn a_regression_below_the_last_valid_sample_counts_once_until_the_next_sample() {
    let clock = at(5_000_000_000, 5_000_000_000, 100);
    let _ = sample(&clock);
    clock
        .regress(ClockKind::Boot, Duration::from_secs(2))
        .unwrap();
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(3_000_000_000)));
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(3_000_000_000)));
    assert_eq!(counts(&clock), (Doubt { boot: 1, awake: 0 }, 1));
    // A further regression below the regressed value counts again.
    clock
        .regress(ClockKind::Boot, Duration::from_secs(1))
        .unwrap();
    assert_eq!(counts(&clock), (Doubt { boot: 2, awake: 0 }, 2));
}
