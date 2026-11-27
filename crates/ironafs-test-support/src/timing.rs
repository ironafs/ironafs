// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Timing a benchmark operation through the awake clock seam.

use core::error::Error;
use core::fmt;
use core::time::Duration;

use ironafs_time::{Clock, ClockError};

/// Why a benchmark measurement failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingError {
    /// A read of the awake clock failed.
    Clock(ClockError),
    /// The ending reading was earlier than the starting reading.
    ClockRegressed,
}

impl fmt::Display for TimingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => fmt::Display::fmt(error, formatter),
            Self::ClockRegressed => formatter.write_str("the awake clock moved backwards"),
        }
    }
}

impl Error for TimingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::ClockRegressed => None,
        }
    }
}

/// Runs `operation` once and returns its result with the elapsed awake time.
///
/// # Errors
///
/// Returns [`TimingError::Clock`] if either read fails. The operation runs
/// only after a successful starting read. Returns
/// [`TimingError::ClockRegressed`] if the ending reading is earlier.
pub fn measure_awake<T>(
    clock: &dyn Clock,
    operation: impl FnOnce() -> T,
) -> Result<(T, Duration), TimingError> {
    let start = clock.awake().map_err(TimingError::Clock)?;
    let result = operation();
    let end = clock.awake().map_err(TimingError::Clock)?;
    let elapsed = end
        .checked_duration_since(start)
        .ok_or(TimingError::ClockRegressed)?;
    Ok((result, elapsed))
}
