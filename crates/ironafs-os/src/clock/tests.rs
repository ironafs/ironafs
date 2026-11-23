// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the checks every clock read passes, with the read itself
//! given as values (ADR 0005, sheet 1: the stand-in returns each invalid
//! `timespec` and a failure).

use std::io;

use super::{ClockError, OsClock, nanos};

#[test]
fn nanos_joins_seconds_and_nanoseconds() {
    assert_eq!(nanos(OsClock::Boot, Ok((0, 0))), Ok(0));
    assert_eq!(
        nanos(OsClock::Awake, Ok((3, 999_999_999))),
        Ok(3_999_999_999)
    );
}

#[test]
fn nanos_refuses_a_negative_second_count() {
    assert_eq!(
        nanos(OsClock::Boot, Ok((-1, 0))),
        Err(ClockError::Invalid {
            clock: OsClock::Boot,
            seconds: -1,
            nanos: 0
        })
    );
}

#[test]
fn nanos_refuses_a_nanosecond_field_outside_one_second() {
    for bad in [-1, 1_000_000_000, i64::MAX, i64::MIN] {
        assert_eq!(
            nanos(OsClock::Awake, Ok((1, bad))),
            Err(ClockError::Invalid {
                clock: OsClock::Awake,
                seconds: 1,
                nanos: bad
            }),
            "{bad}"
        );
    }
}

#[test]
fn nanos_refuses_a_time_that_overflows_64_bit_nanoseconds() {
    let seconds = i64::try_from(u64::MAX / 1_000_000_000).unwrap();
    assert_eq!(
        nanos(OsClock::Boot, Ok((seconds, 709_551_615))),
        Ok(u64::MAX)
    );
    assert_eq!(
        nanos(OsClock::Boot, Ok((seconds, 709_551_616))),
        Err(ClockError::Overflow {
            clock: OsClock::Boot
        })
    );
    assert_eq!(
        nanos(OsClock::Boot, Ok((seconds + 1, 0))),
        Err(ClockError::Overflow {
            clock: OsClock::Boot
        })
    );
}

#[test]
fn nanos_passes_a_failed_read_on_as_the_clock_s_error() {
    let failed = nanos(OsClock::Awake, Err(io::Error::from_raw_os_error(22)));
    match failed {
        Err(ClockError::Failed { clock, error }) => {
            assert_eq!(clock, OsClock::Awake);
            assert_eq!(error.raw_os_error(), Some(22));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn clock_errors_display_lowercase_with_the_clock_named() {
    assert_eq!(
        ClockError::Overflow {
            clock: OsClock::Boot
        }
        .to_string(),
        "the boot clock's reading does not fit 64-bit nanoseconds"
    );
    assert_eq!(
        ClockError::Invalid {
            clock: OsClock::Awake,
            seconds: 1,
            nanos: -1
        }
        .to_string(),
        "the awake clock returned an invalid time: 1 s and -1 ns"
    );
}
