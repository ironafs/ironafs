// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two monotonic clocks of ADR 0004, C37, point 2, and the checks
//! every read of them passes, whatever operating system made it.
//!
//! The **boot** clock counts time the machine was suspended; the **awake**
//! clock does not. On Linux they are `CLOCK_BOOTTIME` and
//! `CLOCK_MONOTONIC` (site `linux-clock`). Each read is a 64-bit
//! nanosecond count since an unspecified start, or an error; a read never
//! falls back to the other clock. What a failed read means (deadlines
//! expire, callbacks become suspect) is the caller's (ADR 0004, C37,
//! point 5), and so is wrapping the reads in `ironafs_time::Clock`.

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// Nanoseconds in a second.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux sites; the other systems' sites join with their parts"
    )
)]
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Which of the two monotonic clocks to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsClock {
    /// The boot clock, which counts time the machine was suspended:
    /// deadlines a server grants count on it.
    Boot,
    /// The awake clock, which does not count suspend: round-trip and
    /// retransmission timers count on it.
    Awake,
}

impl OsClock {
    fn name(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Awake => "awake",
        }
    }
}

/// Why a clock read failed.
#[derive(Debug)]
pub enum ClockError {
    /// The operating system refused the read, with its error.
    Failed { clock: OsClock, error: io::Error },
    /// The read succeeded but returned a time no clock can hold: a negative
    /// second count, or a nanosecond field outside 0 to 999 999 999.
    Invalid {
        clock: OsClock,
        seconds: i64,
        nanos: i64,
    },
    /// The time does not fit 64-bit nanoseconds.
    Overflow { clock: OsClock },
}

/// Two errors are equal when they say the same thing; operating-system
/// errors compare by their kind and number.
impl PartialEq for ClockError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Failed { clock, error },
                Self::Failed {
                    clock: other_clock,
                    error: other_error,
                },
            ) => {
                clock == other_clock
                    && error.kind() == other_error.kind()
                    && error.raw_os_error() == other_error.raw_os_error()
            }
            (
                Self::Invalid {
                    clock,
                    seconds,
                    nanos,
                },
                Self::Invalid {
                    clock: other_clock,
                    seconds: other_seconds,
                    nanos: other_nanos,
                },
            ) => clock == other_clock && seconds == other_seconds && nanos == other_nanos,
            (Self::Overflow { clock }, Self::Overflow { clock: other_clock }) => {
                clock == other_clock
            }
            _ => false,
        }
    }
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed { clock, error } => {
                write!(
                    formatter,
                    "the {} clock cannot be read: {error}",
                    clock.name()
                )
            }
            Self::Invalid {
                clock,
                seconds,
                nanos,
            } => write!(
                formatter,
                "the {} clock returned an invalid time: {seconds} s and {nanos} ns",
                clock.name()
            ),
            Self::Overflow { clock } => write!(
                formatter,
                "the {} clock's reading does not fit 64-bit nanoseconds",
                clock.name()
            ),
        }
    }
}

impl Error for ClockError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Checks one read of `clock`, given as the operating system's seconds and
/// nanoseconds or its error, and returns it as 64-bit nanoseconds, with
/// checked arithmetic (coding standards rule 2; ADR 0004, C38, point 1).
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux sites; the other systems' sites join with their parts"
    )
)]
pub(crate) fn nanos(clock: OsClock, read: io::Result<(i64, i64)>) -> Result<u64, ClockError> {
    let (seconds, nanos) = read.map_err(|error| ClockError::Failed { clock, error })?;
    let invalid = ClockError::Invalid {
        clock,
        seconds,
        nanos,
    };
    let Ok(whole) = u64::try_from(seconds) else {
        return Err(invalid);
    };
    let Ok(part) = u64::try_from(nanos) else {
        return Err(invalid);
    };
    if part >= NANOS_PER_SECOND {
        return Err(invalid);
    }
    whole
        .checked_mul(NANOS_PER_SECOND)
        .and_then(|total| total.checked_add(part))
        .ok_or(ClockError::Overflow { clock })
}
