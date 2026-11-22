// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the deadline constructors and the waiting vocabulary (seams
//! and scheduling specification, 5.3, points 1 and 2).

use alloc::string::ToString;
use core::time::Duration;

use super::{Deadline, DeadlineError, Doubt, Registration, Stamp, WatchFull};
use crate::{
    AwakeDeadline, AwakeInstant, BootDeadline, BootInstant, ClockError, ClockKind, WallTime,
};

/// A stamp at `boot` and `awake` nanoseconds with the given doubt counts.
fn stamp(boot: u64, awake: u64, boot_doubt: u64, awake_doubt: u64) -> Stamp {
    Stamp {
        boot: BootInstant::from_nanos(boot),
        awake: AwakeInstant::from_nanos(awake),
        wall: WallTime::UNIX_EPOCH,
        doubt: Doubt {
            boot: boot_doubt,
            awake: awake_doubt,
        },
        episodes: 0,
    }
}

const SECOND: Duration = Duration::from_secs(1);

#[test]
fn boot_after_counts_from_from_on_the_boot_clock() {
    let from = stamp(1_000, 900, 2, 3);
    let now = stamp(5_000, 4_000, 2, 3);
    let deadline = Deadline::boot_after(&now, &from, SECOND).unwrap();
    assert_eq!(deadline.clock(), ClockKind::Boot);
    assert_eq!(
        deadline.boot_at(),
        Some(BootDeadline::at(BootInstant::from_nanos(1_000_001_000)))
    );
    assert_eq!(deadline.awake_at(), None);
    assert_eq!(deadline.latched_doubt(), 2);
}

#[test]
fn awake_after_counts_from_from_on_the_awake_clock() {
    let from = stamp(1_000, 900, 2, 3);
    let now = stamp(5_000, 4_000, 2, 3);
    let deadline = Deadline::awake_after(&now, &from, SECOND).unwrap();
    assert_eq!(deadline.clock(), ClockKind::Awake);
    assert_eq!(
        deadline.awake_at(),
        Some(AwakeDeadline::at(AwakeInstant::from_nanos(1_000_000_900)))
    );
    assert_eq!(deadline.boot_at(), None);
    assert_eq!(deadline.latched_doubt(), 3);
}

#[test]
fn stale_stamp_gives_an_already_failed_deadline() {
    // `now` kept from before a failure: a fresh sample after it carries a
    // higher count, so the deadline's latch is already below the clock's.
    let from = stamp(1_000, 1_000, 4, 4);
    let kept_now = stamp(2_000, 2_000, 4, 4);
    let fresh = stamp(3_000, 3_000, 5, 5);
    for deadline in [
        Deadline::boot_after(&kept_now, &from, SECOND).unwrap(),
        Deadline::awake_after(&kept_now, &from, SECOND).unwrap(),
    ] {
        assert_eq!(deadline.latched_doubt(), 4);
        assert!(deadline.latched_doubt() < fresh.doubt.boot);
        assert!(deadline.latched_doubt() < fresh.doubt.awake);
    }
}

#[test]
fn regression_between_from_and_now_fails_the_deadline() {
    // A boot-clock regression after the arrival moved the boot count; the
    // latch is the arrival's lower count, so the deadline is failed at
    // its first evaluation against `now`'s count.
    let from = stamp(9_000, 1_000, 7, 2);
    let now = stamp(9_500, 2_000, 8, 2);
    let deadline = Deadline::boot_after(&now, &from, SECOND).unwrap();
    assert_eq!(deadline.latched_doubt(), 7);
    assert!(deadline.latched_doubt() < now.doubt.boot);
}

#[test]
fn from_after_now_is_refused() {
    let now = stamp(5_000, 5_000, 3, 3);
    // A higher count on the deadline's clock.
    let higher = stamp(1_000, 1_000, 4, 4);
    assert_eq!(
        Deadline::boot_after(&now, &higher, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
    assert_eq!(
        Deadline::awake_after(&now, &higher, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
    // A later instant on the deadline's clock.
    let later = stamp(6_000, 6_000, 3, 3);
    assert_eq!(
        Deadline::boot_after(&now, &later, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
    assert_eq!(
        Deadline::awake_after(&now, &later, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
    // A later instant with a lower count is still refused: across a
    // regression a true arrival can read later, and refusal is the safe
    // outcome.
    let before_regression = stamp(6_000, 6_000, 2, 2);
    assert_eq!(
        Deadline::boot_after(&now, &before_regression, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
}

#[test]
fn from_after_now_is_judged_on_the_deadline_clock_only() {
    // Later on the awake clock only: a boot deadline is still built.
    let now = stamp(5_000, 5_000, 3, 3);
    let from = stamp(4_000, 6_000, 3, 9);
    assert!(Deadline::boot_after(&now, &from, SECOND).is_ok());
    assert_eq!(
        Deadline::awake_after(&now, &from, SECOND),
        Err(DeadlineError::FromAfterNow)
    );
}

#[test]
fn equal_stamps_build_a_deadline() {
    let now = stamp(5_000, 5_000, 3, 3);
    assert!(Deadline::boot_after(&now, &now, Duration::ZERO).is_ok());
    assert!(Deadline::awake_after(&now, &now, Duration::ZERO).is_ok());
}

#[test]
fn overflow_is_refused_never_clamped() {
    let at_end = stamp(u64::MAX - 1, u64::MAX - 1, 0, 0);
    assert_eq!(
        Deadline::boot_after(&at_end, &at_end, SECOND),
        Err(DeadlineError::Overflow)
    );
    assert_eq!(
        Deadline::awake_after(&at_end, &at_end, SECOND),
        Err(DeadlineError::Overflow)
    );
}

#[test]
fn deadline_error_messages() {
    assert_eq!(
        DeadlineError::Clock(ClockError::new(ClockKind::Boot)).to_string(),
        "no deadline without a valid sample: the boot clock could not be read"
    );
    assert_eq!(
        DeadlineError::FromAfterNow.to_string(),
        "the deadline's start is newer than the sample it is built at"
    );
    assert_eq!(
        DeadlineError::Overflow.to_string(),
        "the deadline does not fit in 64-bit nanoseconds"
    );
}

#[test]
fn registration_keeps_its_number() {
    assert_eq!(Registration::new(42).get(), 42);
}

#[test]
fn watch_full_message() {
    assert_eq!(WatchFull.to_string(), "the watch's waker list is full");
}
