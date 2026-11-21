// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seeded clock: reads take no time, advance and suspend
//! move the right clocks, overflow moves nothing, and reads fail on demand.

use std::time::Duration;

use ironafs_time::{AwakeInstant, BootInstant, Clock, ClockError, ClockKind, WallTime};

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
