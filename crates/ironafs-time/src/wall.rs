// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wall time: signed 64-bit seconds since 1970 with a nanosecond part
//! (ADR 0004, C38, point 1). It is the one wall-time type inside IronAFS;
//! the wire's narrower times are converted to and from it in one place
//! (C38, point 2), and a value the wire cannot carry is refused there.

use core::time::Duration;

#[cfg(test)]
mod tests;

/// Nanoseconds in one second.
const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// [`NANOS_PER_SECOND`] for arithmetic on nanoseconds since 1970.
const NANOS_I128: i128 = 1_000_000_000;

/// A wall-clock time: seconds since 1970-01-01T00:00:00Z, negative before
/// it, plus nanoseconds within the second.
///
/// Only for AFS timestamps and for token expiry as the protocol states it
/// (ADR 0004, C37, point 1): no deadline counts on wall time, which can
/// jump. The signed 64-bit seconds carry times before 1970 and long past
/// the 32-bit limits of 2038 and 2106. Ordering is chronological.
///
/// ```
/// use ironafs_time::WallTime;
///
/// let time = WallTime::from_unix(-1, 500_000_000).unwrap();
/// assert!(time < WallTime::UNIX_EPOCH);
/// assert_eq!(WallTime::from_unix(0, 1_000_000_000), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WallTime {
    // Field order makes the derived ordering chronological: `nanos` is
    // always below one second and counts forward from `seconds`.
    seconds: i64,
    nanos: u32,
}

impl WallTime {
    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Self = Self {
        seconds: 0,
        nanos: 0,
    };

    /// The time `seconds` after 1970 plus `nanos` nanoseconds, or `None` if
    /// `nanos` is a second or more. A time before 1970 has negative
    /// `seconds` and counts `nanos` forward from them: half a second before
    /// 1970 is `(-1, 500_000_000)`.
    #[must_use]
    pub const fn from_unix(seconds: i64, nanos: u32) -> Option<Self> {
        if nanos < NANOS_PER_SECOND {
            Some(Self { seconds, nanos })
        } else {
            None
        }
    }

    /// Whole seconds since 1970, rounded towards the past.
    #[must_use]
    pub const fn seconds(self) -> i64 {
        self.seconds
    }

    /// Nanoseconds after [`WallTime::seconds`], below one second.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// The time `duration` later, or `None` if the seconds overflow.
    #[must_use]
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let delta = i128::try_from(duration.as_nanos()).ok()?;
        Self::from_total_nanos(self.total_nanos().checked_add(delta)?)
    }

    /// The time `duration` earlier, or `None` if the seconds overflow.
    #[must_use]
    pub fn checked_sub(self, duration: Duration) -> Option<Self> {
        let delta = i128::try_from(duration.as_nanos()).ok()?;
        Self::from_total_nanos(self.total_nanos().checked_sub(delta)?)
    }

    /// The time from `earlier` to this time, or `None` if `earlier` is
    /// later. Wall time can jump backwards, so a caller handles `None`.
    #[must_use]
    pub fn checked_duration_since(self, earlier: Self) -> Option<Duration> {
        let span = self.total_nanos().checked_sub(earlier.total_nanos())?;
        let seconds = u64::try_from(span.checked_div(NANOS_I128)?).ok()?;
        let nanos = u32::try_from(span.checked_rem(NANOS_I128)?).ok()?;
        Some(Duration::new(seconds, nanos))
    }

    /// Nanoseconds since 1970. Every wall time fits: the seconds times one
    /// billion is below 2^93.
    fn total_nanos(self) -> i128 {
        i128::from(self.seconds) * NANOS_I128 + i128::from(self.nanos)
    }

    /// The wall time `total` nanoseconds after 1970, or `None` if its
    /// seconds do not fit in 64 bits.
    fn from_total_nanos(total: i128) -> Option<Self> {
        let seconds = i64::try_from(total.checked_div_euclid(NANOS_I128)?).ok()?;
        let nanos = u32::try_from(total.checked_rem_euclid(NANOS_I128)?).ok()?;
        Self::from_unix(seconds, nanos)
    }
}
