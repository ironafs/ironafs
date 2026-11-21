// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-clock` against the running kernel (ADR 0005, sheet 1:
//! both clocks advance and never go back across reads). The checks on
//! invalid values and failures are `clock`'s, with a stand-in.

use super::abi::{CLOCK_BOOTTIME, CLOCK_MONOTONIC};
use super::{clock_id, read_clock};
use crate::clock::OsClock;

/// Reads `clock` until it changes, at most a million times; returns the
/// first and the changed reading.
fn until_it_moves(clock: OsClock) -> (u64, u64) {
    let first = read_clock(clock).unwrap();
    for _ in 0..1_000_000 {
        let next = read_clock(clock).unwrap();
        assert!(next >= first, "{clock:?} went back: {first} then {next}");
        if next > first {
            return (first, next);
        }
    }
    panic!("{clock:?} did not move in a million reads");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn read_clock_boot_advances_and_never_goes_back() {
    let (first, next) = until_it_moves(OsClock::Boot);
    assert!(next > first);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn read_clock_awake_advances_and_never_goes_back() {
    let (first, next) = until_it_moves(OsClock::Awake);
    assert!(next > first);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn read_clock_boot_is_never_behind_awake_read_before_it() {
    for _ in 0..1000 {
        let awake = read_clock(OsClock::Awake).unwrap();
        let boot = read_clock(OsClock::Boot).unwrap();
        assert!(boot >= awake, "boot {boot} behind awake {awake}");
    }
}

#[test]
fn clock_id_reads_the_boot_clock_from_clock_boottime_and_awake_from_clock_monotonic() {
    // No run without a suspend can tell the two apart by their readings, so
    // the mapping is asserted here and the constants by the ABI check.
    assert_eq!(clock_id(OsClock::Boot), CLOCK_BOOTTIME);
    assert_eq!(clock_id(OsClock::Awake), CLOCK_MONOTONIC);
}
