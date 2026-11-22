// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the boot and awake instants and deadlines. Both types come
//! from one macro, so most behaviour is asserted on one clock and the
//! deadline rules on both.

use core::time::Duration;

use super::{AwakeDeadline, AwakeInstant, BootDeadline, BootInstant};

#[test]
fn checked_add_adds_nanoseconds() {
    let start = BootInstant::from_nanos(7);
    assert_eq!(
        start.checked_add(Duration::new(2, 5)),
        Some(BootInstant::from_nanos(2_000_000_012))
    );
}

#[test]
fn checked_add_reaches_the_last_nanosecond() {
    let start = BootInstant::from_nanos(u64::MAX - 1);
    assert_eq!(
        start.checked_add(Duration::from_nanos(1)),
        Some(BootInstant::from_nanos(u64::MAX))
    );
}

#[test]
fn checked_add_past_the_last_nanosecond_is_none() {
    let start = BootInstant::from_nanos(u64::MAX);
    assert_eq!(start.checked_add(Duration::from_nanos(1)), None);
}

#[test]
fn checked_add_of_a_duration_over_64_bits_is_none() {
    let start = AwakeInstant::from_nanos(0);
    assert_eq!(start.checked_add(Duration::MAX), None);
}

#[test]
fn checked_duration_since_gives_the_elapsed_time() {
    let earlier = AwakeInstant::from_nanos(1_000);
    let later = AwakeInstant::from_nanos(3_500_000_000);
    assert_eq!(
        later.checked_duration_since(earlier),
        Some(Duration::from_micros(3_499_999))
    );
    assert_eq!(
        earlier.checked_duration_since(earlier),
        Some(Duration::ZERO)
    );
}

#[test]
fn checked_duration_since_a_later_instant_is_none() {
    let earlier = BootInstant::from_nanos(1);
    let later = BootInstant::from_nanos(2);
    assert_eq!(earlier.checked_duration_since(later), None);
}

#[test]
fn boot_deadline_passes_at_its_instant() {
    let deadline = BootDeadline::at(BootInstant::from_nanos(100));
    assert!(!deadline.has_passed(BootInstant::from_nanos(99)));
    assert!(deadline.has_passed(BootInstant::from_nanos(100)));
    assert!(deadline.has_passed(BootInstant::from_nanos(101)));
}

#[test]
fn awake_deadline_passes_at_its_instant() {
    let deadline = AwakeDeadline::at(AwakeInstant::from_nanos(100));
    assert!(!deadline.has_passed(AwakeInstant::from_nanos(99)));
    assert!(deadline.has_passed(AwakeInstant::from_nanos(100)));
}

#[test]
fn deadline_after_counts_from_now() {
    let now = BootInstant::from_nanos(10);
    let deadline = BootDeadline::after(now, Duration::from_nanos(5));
    assert_eq!(
        deadline.map(BootDeadline::instant),
        Some(BootInstant::from_nanos(15))
    );
}

#[test]
fn deadline_after_past_the_last_nanosecond_is_none() {
    let now = AwakeInstant::from_nanos(u64::MAX);
    assert_eq!(AwakeDeadline::after(now, Duration::from_nanos(1)), None);
}

#[test]
fn remaining_counts_down_to_zero_and_stays_there() {
    let deadline = BootDeadline::at(BootInstant::from_nanos(100));
    assert_eq!(
        deadline.remaining(BootInstant::from_nanos(40)),
        Duration::from_nanos(60)
    );
    assert_eq!(
        deadline.remaining(BootInstant::from_nanos(100)),
        Duration::ZERO
    );
    assert_eq!(
        deadline.remaining(BootInstant::from_nanos(500)),
        Duration::ZERO
    );
}
