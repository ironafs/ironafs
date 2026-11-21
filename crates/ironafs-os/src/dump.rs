// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dump controls (ADR 0004, O25 and O26; ADR 0005, sheets 11 and 12):
//! the steps that keep a process's memory out of core dumps, and their
//! error, whatever operating system takes them.
//!
//! On Linux, "disable dumps for this process" is three steps: the
//! core-file limit set to zero, the process made non-dumpable, and the
//! dumpable attribute read back, which must then be exactly 0. The limit
//! alone is not enough, because a dump piped to a program ignores it
//! (core(5)). Every step is taken even when an earlier one failed, so a
//! process that is about to refuse to start is still as protected as its
//! system allows; the first failure is reported. What a failure means
//! (startup refused, O25 being Firm) is the caller's.

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
pub(crate) mod harness;
#[cfg(test)]
mod tests;

/// Why the dump controls are not in place.
#[derive(Debug)]
pub enum DumpError {
    /// Setting the core-file limit to zero failed.
    Limit(io::Error),
    /// Making the process non-dumpable failed.
    Set(io::Error),
    /// Reading the dumpable attribute back failed.
    ReadBack(io::Error),
    /// The dumpable attribute read back as this value, not 0, after it was
    /// set to 0 (1 is dumpable; 2 is the kernel's root-only dump mode).
    StillDumpable(i32),
    /// Excluding a secret buffer's memory from dumps failed, so the buffer
    /// was not handed out.
    Exclude(io::Error),
}

impl fmt::Display for DumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(error) => write!(
                formatter,
                "setting the core-file limit to zero failed: {error}"
            ),
            Self::Set(error) => {
                write!(formatter, "making the process non-dumpable failed: {error}")
            }
            Self::ReadBack(error) => {
                write!(
                    formatter,
                    "reading back the dumpable attribute failed: {error}"
                )
            }
            Self::StillDumpable(value) => write!(
                formatter,
                "the dumpable attribute reads {value} after it was set to 0"
            ),
            Self::Exclude(error) => {
                write!(
                    formatter,
                    "excluding a secret buffer from dumps failed: {error}"
                )
            }
        }
    }
}

impl Error for DumpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Limit(error)
            | Self::Set(error)
            | Self::ReadBack(error)
            | Self::Exclude(error) => Some(error),
            Self::StillDumpable(_) => None,
        }
    }
}

/// Takes the three Linux steps through `limit`, `set` and `read`, each
/// once and in that order, whatever the earlier ones returned; succeeds
/// only when the first two succeed and `read` returns exactly 0.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux site; the macOS part joins with its own steps"
    )
)]
pub(crate) fn disable_with(
    limit: impl FnOnce() -> io::Result<()>,
    set: impl FnOnce() -> io::Result<()>,
    read: impl FnOnce() -> io::Result<i32>,
) -> Result<(), DumpError> {
    let limit = limit().map_err(DumpError::Limit);
    let set = set().map_err(DumpError::Set);
    let read = match read() {
        Ok(0) => Ok(()),
        Ok(value) => Err(DumpError::StillDumpable(value)),
        Err(error) => Err(DumpError::ReadBack(error)),
    };
    limit.and(set).and(read)
}
