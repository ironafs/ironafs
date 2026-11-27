// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmark measurements through the public timing helper.

use core::time::Duration;

use ironafs_test_support::{SeededClock, TimingError, measure_awake};
use ironafs_time::{Clock, ClockError, ClockKind};

fn record(seed: u64) -> [(u8, Duration); 2] {
    let clock = SeededClock::from_seed(seed);
    let reader: &dyn Clock = &clock;
    clock.set_failing(ClockKind::Boot, true);
    clock.set_failing(ClockKind::Wall, true);

    let mut calls = 0;
    let first = measure_awake(reader, || {
        clock.advance(Duration::from_nanos(125)).unwrap();
        clock.suspend(Duration::from_secs(3)).unwrap();
        calls += 1;
        calls
    })
    .unwrap();
    let second = measure_awake(reader, || {
        clock.advance(Duration::from_millis(7)).unwrap();
        calls += 1;
        calls
    })
    .unwrap();

    [first, second]
}

#[test]
fn seeded_measurements_record_each_operation_and_replay_from_one_seed() {
    let seed = 0x5eed;
    let first = record(seed);
    assert_eq!(
        first,
        [
            (1, Duration::from_nanos(125)),
            (2, Duration::from_millis(7))
        ],
        "seed {seed:#x}"
    );
    assert_eq!(record(seed), first, "seed {seed:#x}");
}

#[test]
fn measurement_rejects_a_backwards_awake_read() {
    let clock = SeededClock::from_seed(0x5eed);
    let result = measure_awake(&clock, || {
        clock
            .regress(ClockKind::Awake, Duration::from_nanos(1))
            .unwrap();
    });

    assert_eq!(result, Err(TimingError::ClockRegressed));
}

#[test]
fn failed_start_read_skips_the_operation() {
    let clock = SeededClock::from_seed(0x5eed);
    clock.set_failing(ClockKind::Awake, true);
    let mut called = false;

    let result = measure_awake(&clock, || called = true);

    assert_eq!(
        result,
        Err(TimingError::Clock(ClockError::new(ClockKind::Awake)))
    );
    assert!(!called);
}

#[test]
fn failed_end_read_rejects_the_measurement() {
    let clock = SeededClock::from_seed(0x5eed);
    let result = measure_awake(&clock, || clock.set_failing(ClockKind::Awake, true));

    assert_eq!(
        result,
        Err(TimingError::Clock(ClockError::new(ClockKind::Awake)))
    );
}
