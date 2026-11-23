// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-clock` (ADR 0005, sheet 1: both clocks advance and
//! never go back across reads), against the running kernel, and under Miri
//! against the foreign function's stand-in. The checks on invalid values
//! and failures are `clock`'s, with scripted values.
//!
//! The two clocks are not compared with each other: a time namespace can
//! give each its own offset (`time_namespaces(7)`), so the boot clock's
//! reading need not be at or above the awake clock's.

use super::abi::{CLOCK_BOOTTIME, CLOCK_MONOTONIC};
use super::{clock_id, read_clock};
use crate::clock::OsClock;

/// Reads `clock` until it changes, at most a million times; returns the
/// first and the changed reading.
#[track_caller]
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
fn read_clock_boot_advances_and_never_goes_back() {
    let (first, next) = until_it_moves(OsClock::Boot);
    assert!(next > first);
}

#[test]
fn read_clock_awake_advances_and_never_goes_back() {
    let (first, next) = until_it_moves(OsClock::Awake);
    assert!(next > first);
}

#[test]
fn clock_id_reads_the_boot_clock_from_clock_boottime_and_awake_from_clock_monotonic() {
    // No run without a suspend can tell the two apart by their readings, so
    // the mapping is asserted here and the constants by the ABI check.
    assert_eq!(clock_id(OsClock::Boot), CLOCK_BOOTTIME);
    assert_eq!(clock_id(OsClock::Awake), CLOCK_MONOTONIC);
}
