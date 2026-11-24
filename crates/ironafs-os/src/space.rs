// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A filesystem's free space for an unprivileged user (ADR 0005, sheet
//! 22): the result of the query and its errors, whatever made the call.
//! This is safe code over owned values, so the tests reach every branch
//! without a filesystem.

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// Why the free space was not measured.
#[derive(Debug)]
pub enum SpaceError {
    /// The descriptor could not be inspected before the query; the error
    /// carries its errno.
    Inspect(io::Error),
    /// The descriptor is not a directory's: the chunk store's own
    /// directory is the one measured.
    NotDirectory,
    /// The query failed; the error carries its errno.
    Failed(io::Error),
    /// The free blocks times the fragment size does not fit 64 bits.
    Overflow {
        /// `f_bavail`.
        blocks: u64,
        /// `f_frsize`.
        fragment: u64,
    },
}

impl fmt::Display for SpaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspect(error) => write!(formatter, "inspecting the directory failed: {error}"),
            Self::NotDirectory => formatter.write_str("the descriptor is not a directory's"),
            Self::Failed(error) => write!(formatter, "querying the free space failed: {error}"),
            Self::Overflow { blocks, fragment } => write!(
                formatter,
                "{blocks} free blocks of {fragment} bytes do not fit 64 bits"
            ),
        }
    }
}

impl Error for SpaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspect(error) | Self::Failed(error) => Some(error),
            Self::NotDirectory | Self::Overflow { .. } => None,
        }
    }
}

/// The bytes `blocks` free blocks of `fragment` bytes hold
/// (`f_bavail × f_frsize`).
///
/// # Errors
///
/// [`SpaceError::Overflow`] if the product does not fit 64 bits.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn available(blocks: u64, fragment: u64) -> Result<u64, SpaceError> {
    blocks
        .checked_mul(fragment)
        .ok_or(SpaceError::Overflow { blocks, fragment })
}
