// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract's one semantic error type (ADR 0004, C46).
//!
//! Each layer maps it to its system's errors (C46, point 2). The enum is
//! deliberately exhaustive: a layer's mapping names every value, with no
//! wildcard arm that a new value could fall into and come out as success
//! (C46, point 4). A new value is a change of the contract, which layers
//! and core are built against at one exact version (N41, point 7).
//!
//! ```compile_fail,E0004
//! // A mapping that leaves a value out does not compile.
//! fn errno(error: ironafs_vfs::VfsError) -> i32 {
//!     match error {
//!         ironafs_vfs::VfsError::NotFound => 2,
//!         ironafs_vfs::VfsError::Access => 13,
//!     }
//! }
//! ```

use core::error::Error;
use core::fmt;

#[cfg(test)]
mod tests;

/// Why an operation of the contract failed (C46, point 1). No value means
/// success, and no value is a partial result: a failed listing or read is
/// an error, never an empty or short answer (C46, point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VfsError {
    /// No such entry.
    NotFound,
    /// The entry already exists.
    Exists,
    /// The caller's rights do not allow the operation.
    Access,
    /// The caller holds no token for the cell (N4; N27, point 6).
    TokenRequired,
    /// The caller's token has expired.
    TokenExpired,
    /// The caller's token was revoked, which is distinct from expiry (N47,
    /// point 5).
    TokenRevoked,
    /// The connection would be below the cell's security floor (N1).
    SecurityFloor,
    /// The volume or the file is read-only.
    ReadOnly,
    /// The server or the cache has no space.
    NoSpace,
    /// The volume's quota is exceeded.
    QuotaExceeded,
    /// The operation would cross volumes or cells.
    CrossDevice,
    /// The handle or node no longer names what it named (C13; C14, point 3;
    /// N27, point 6).
    Stale,
    /// The executor or the server is busy; the request may be retried
    /// (C39, point 2).
    Busy,
    /// The cell is offline.
    Offline,
    /// The request's deadline passed (C39, point 3).
    TimedOut,
    /// A commit's deadline passed while its store was still making
    /// progress; the data is not yet on the server (C9, points 3 and 5).
    NotYetStored,
    /// The request was cancelled or interrupted.
    Interrupted,
    /// The operation is not supported, including every reserved operation
    /// a volume does not offer (N46, point 1).
    NotSupported,
    /// The operation would block and the caller asked not to.
    WouldBlock,
    /// No server for the volume could be reached.
    Unreachable,
    /// A conflicting lock, share access or change.
    Conflict,
    /// A value outside what the protocol can carry (C38, point 3).
    OutOfRange,
}

/// How a contract error reaches applications (C46, point 1). The cause of
/// an indirect error goes to the side channel (N4, N6, O35).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    /// As a permission error (`EACCES`, an `NSError` with POSIX `EACCES`,
    /// `STATUS_ACCESS_DENIED`).
    Permission,
    /// As an invalid-argument error (`EINVAL`, an `NSError` with POSIX
    /// `EINVAL`, `STATUS_INVALID_PARAMETER`).
    InvalidArgument,
    /// As the system's own error for this value.
    Itself,
}

impl VfsError {
    /// How this error reaches applications (C46, point 1): the token
    /// errors and `SecurityFloor` as a permission error, `OutOfRange` as
    /// an invalid argument, every other value as itself.
    #[must_use]
    pub const fn surface(self) -> Surface {
        match self {
            Self::TokenRequired | Self::TokenExpired | Self::TokenRevoked | Self::SecurityFloor => {
                Surface::Permission
            }
            Self::OutOfRange => Surface::InvalidArgument,
            Self::NotFound
            | Self::Exists
            | Self::Access
            | Self::ReadOnly
            | Self::NoSpace
            | Self::QuotaExceeded
            | Self::CrossDevice
            | Self::Stale
            | Self::Busy
            | Self::Offline
            | Self::TimedOut
            | Self::NotYetStored
            | Self::Interrupted
            | Self::NotSupported
            | Self::WouldBlock
            | Self::Unreachable
            | Self::Conflict => Surface::Itself,
        }
    }
}

impl fmt::Display for VfsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "no such entry",
            Self::Exists => "the entry already exists",
            Self::Access => "permission denied by the entry's rights",
            Self::TokenRequired => "no token for the cell",
            Self::TokenExpired => "the token has expired",
            Self::TokenRevoked => "the token was revoked",
            Self::SecurityFloor => "below the cell's security floor",
            Self::ReadOnly => "read-only",
            Self::NoSpace => "no space left",
            Self::QuotaExceeded => "the volume's quota is exceeded",
            Self::CrossDevice => "the operation would cross volumes",
            Self::Stale => "stale handle or node",
            Self::Busy => "busy, try again",
            Self::Offline => "the cell is offline",
            Self::TimedOut => "the request's deadline passed",
            Self::NotYetStored => "not yet stored on the server",
            Self::Interrupted => "interrupted",
            Self::NotSupported => "not supported",
            Self::WouldBlock => "the operation would block",
            Self::Unreachable => "no server for the volume could be reached",
            Self::Conflict => "conflicting lock, share access or change",
            Self::OutOfRange => "a value is out of the protocol's range",
        })
    }
}

impl Error for VfsError {}
