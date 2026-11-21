// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A local socket peer's credentials, as the operating system attests
//! them (ADR 0005, sheet 13; ADR 0004, C43, point 4), and the checks every
//! read of them passes, whatever operating system made it.
//!
//! The credentials are those the kernel recorded when the connection was
//! made ("those that were in effect at the time of the call to connect(2),
//! listen(2), or socketpair(2)", unix(7)), not the peer's current ones: a
//! process that changes identity after connecting still shows its old one,
//! and a connection handed to another process still carries its opener's.
//! Which peer may do what is the caller's policy.

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// A socket peer's process id, user id and group id, as the kernel
/// recorded them when the connection was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerCredentials {
    pid: i32,
    uid: u32,
    gid: u32,
}

impl PeerCredentials {
    /// The peer's process id, as this process's pid namespace sees it.
    #[must_use]
    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// The peer's effective user id at connection time.
    #[must_use]
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// The peer's effective group id at connection time.
    #[must_use]
    pub fn gid(&self) -> u32 {
        self.gid
    }
}

/// Why a peer's credentials could not be read.
#[derive(Debug)]
pub enum PeerError {
    /// The call failed; the error carries its errno.
    Failed(io::Error),
    /// The call reported a credential length other than its structure's
    /// size, so the values cannot be trusted.
    Length {
        /// The length the call reported.
        returned: usize,
        /// The structure's size.
        expected: usize,
    },
}

impl fmt::Display for PeerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(error) => {
                write!(formatter, "reading the peer's credentials failed: {error}")
            }
            Self::Length { returned, expected } => write!(
                formatter,
                "the peer's credentials came back {returned} bytes long, not {expected}"
            ),
        }
    }
}

impl Error for PeerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Length { .. } => None,
        }
    }
}

/// What one credential read returned: the call's result, the length it
/// reported back, the structure's size, and the values it wrote.
#[derive(Debug)]
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux site; the macOS part joins with its own"
    )
)]
pub(crate) struct Read {
    pub(crate) result: io::Result<()>,
    pub(crate) returned: usize,
    pub(crate) expected: usize,
    pub(crate) pid: i32,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
}

/// The credentials of one read, only if the call succeeded and reported
/// exactly the structure's size.
///
/// # Errors
///
/// [`PeerError::Failed`] with the call's error, or [`PeerError::Length`].
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux site; the macOS part joins with its own"
    )
)]
pub(crate) fn checked(read: Read) -> Result<PeerCredentials, PeerError> {
    read.result.map_err(PeerError::Failed)?;
    if read.returned != read.expected {
        return Err(PeerError::Length {
            returned: read.returned,
            expected: read.expected,
        });
    }
    Ok(PeerCredentials {
        pid: read.pid,
        uid: read.uid,
        gid: read.gid,
    })
}
