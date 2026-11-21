// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for wall time: its range past the 32-bit limits, times before
//! 1970, normalisation of the nanosecond part and checked arithmetic.

use core::time::Duration;

use super::WallTime;

fn wall(seconds: i64, nanos: u32) -> WallTime {
    WallTime::from_unix(seconds, nanos).unwrap()
}

#[test]
fn from_unix_keeps_seconds_and_nanos() {
    let time = wall(-5, 999_999_999);
    assert_eq!(time.seconds(), -5);
    assert_eq!(time.subsec_nanos(), 999_999_999);
}

#[test]
fn from_unix_rejects_a_whole_second_of_nanos() {
    assert_eq!(WallTime::from_unix(0, 1_000_000_000), None);
    assert_eq!(WallTime::from_unix(0, u32::MAX), None);
}

#[test]
fn from_unix_carries_the_32_bit_boundaries() {
    // 2038-01-19T03:14:08Z and 2106-02-07T06:28:16Z, the first seconds a
    // signed and an unsigned 32-bit time cannot hold.
    let signed_limit = i64::from(i32::MAX) + 1;
    let unsigned_limit = i64::from(u32::MAX) + 1;
    assert_eq!(wall(signed_limit, 0).seconds(), 2_147_483_648);
    assert_eq!(wall(unsigned_limit, 0).seconds(), 4_294_967_296);
    assert!(wall(signed_limit, 0) > wall(i64::from(i32::MAX), 999_999_999));
}

#[test]
fn ordering_is_chronological_across_1970() {
    let before = wall(-1, 999_999_999);
    assert!(before < WallTime::UNIX_EPOCH);
    assert!(wall(-2, 0) < wall(-1, 0));
    assert!(WallTime::UNIX_EPOCH < wall(0, 1));
}

#[test]
fn checked_add_carries_into_the_seconds() {
    let time = wall(-1, 600_000_000);
    assert_eq!(
        time.checked_add(Duration::from_millis(500)),
        Some(wall(0, 100_000_000))
    );
}

#[test]
fn checked_add_crosses_2038_without_wrapping() {
    let time = wall(i64::from(i32::MAX), 0);
    assert_eq!(
        time.checked_add(Duration::from_secs(1)),
        Some(wall(2_147_483_648, 0))
    );
}

#[test]
fn checked_add_past_the_last_second_is_none() {
    let time = wall(i64::MAX, 999_999_999);
    assert_eq!(time.checked_add(Duration::from_nanos(1)), None);
    assert_eq!(
        WallTime::UNIX_EPOCH.checked_add(Duration::from_secs(u64::MAX)),
        None
    );
}

#[test]
fn checked_sub_borrows_from_the_seconds() {
    let time = wall(0, 100_000_000);
    assert_eq!(
        time.checked_sub(Duration::from_millis(500)),
        Some(wall(-1, 600_000_000))
    );
}

#[test]
fn checked_sub_past_the_first_second_is_none() {
    let time = wall(i64::MIN, 0);
    assert_eq!(time.checked_sub(Duration::from_nanos(1)), None);
    assert_eq!(
        WallTime::UNIX_EPOCH.checked_sub(Duration::from_secs(u64::MAX)),
        None
    );
}

#[test]
fn checked_duration_since_spans_1970() {
    let earlier = wall(-1, 750_000_000);
    let later = wall(1, 250_000_000);
    assert_eq!(
        later.checked_duration_since(earlier),
        Some(Duration::from_millis(1_500))
    );
    assert_eq!(later.checked_duration_since(later), Some(Duration::ZERO));
}

#[test]
fn checked_duration_since_a_later_time_is_none() {
    assert_eq!(wall(0, 0).checked_duration_since(wall(0, 1)), None);
}

#[test]
fn checked_duration_since_spans_the_whole_range() {
    let span = wall(i64::MAX, 999_999_999).checked_duration_since(wall(i64::MIN, 0));
    assert_eq!(span, Some(Duration::new(u64::MAX, 999_999_999)));
}
