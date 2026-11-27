// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux error numbers the layer answers with (fuse-codec.md 5.7).
//!
//! The numbers are those of the kernel's `asm-generic` error headers,
//! which x86-64 and 64-bit Arm both use, whatever system this crate is built
//! on. [`Errno`] is a closed set: its only values are the constants here,
//! and none is `ENOSYS`, which the kernel treats as a permanent refusal
//! for some requests (ADR 0004, L2, point 4, and N43).

use ironafs_vfs::{NameError, VfsError};

#[cfg(test)]
mod tests;

/// A Linux error number a reply may carry. A reply sends its negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Errno(i32);

impl Errno {
    /// No such file or directory.
    pub const ENOENT: Self = Self(2);
    /// Interrupted system call.
    pub const EINTR: Self = Self(4);
    /// Input/output error.
    pub const EIO: Self = Self(5);
    /// No such device or address.
    pub const ENXIO: Self = Self(6);
    /// Try again; the same value as `EWOULDBLOCK`.
    pub const EAGAIN: Self = Self(11);
    /// Permission denied.
    pub const EACCES: Self = Self(13);
    /// Device or resource busy.
    pub const EBUSY: Self = Self(16);
    /// File exists.
    pub const EEXIST: Self = Self(17);
    /// Cross-device link.
    pub const EXDEV: Self = Self(18);
    /// Not a directory.
    pub const ENOTDIR: Self = Self(20);
    /// Is a directory.
    pub const EISDIR: Self = Self(21);
    /// Invalid argument.
    pub const EINVAL: Self = Self(22);
    /// No space left on device.
    pub const ENOSPC: Self = Self(28);
    /// Read-only file system.
    pub const EROFS: Self = Self(30);
    /// Math result not representable.
    pub const ERANGE: Self = Self(34);
    /// File name too long.
    pub const ENAMETOOLONG: Self = Self(36);
    /// Directory not empty.
    pub const ENOTEMPTY: Self = Self(39);
    /// No data available.
    pub const ENODATA: Self = Self(61);
    /// Protocol error: the answer to an INIT the codec refuses.
    pub const EPROTO: Self = Self(71);
    /// Operation not supported; the same value as `ENOTSUP`. The answer
    /// to every request the layer does not serve.
    pub const EOPNOTSUPP: Self = Self(95);
    /// Network is down.
    pub const ENETDOWN: Self = Self(100);
    /// Connection timed out.
    pub const ETIMEDOUT: Self = Self(110);
    /// No route to host.
    pub const EHOSTUNREACH: Self = Self(113);
    /// Stale file handle.
    pub const ESTALE: Self = Self(116);
    /// Quota exceeded.
    pub const EDQUOT: Self = Self(122);

    /// Every value the type can hold.
    pub const ALL: &[Self] = &[
        Self::ENOENT,
        Self::EINTR,
        Self::EIO,
        Self::ENXIO,
        Self::EAGAIN,
        Self::EACCES,
        Self::EBUSY,
        Self::EEXIST,
        Self::EXDEV,
        Self::ENOTDIR,
        Self::EISDIR,
        Self::EINVAL,
        Self::ENOSPC,
        Self::EROFS,
        Self::ERANGE,
        Self::ENAMETOOLONG,
        Self::ENOTEMPTY,
        Self::ENODATA,
        Self::EPROTO,
        Self::EOPNOTSUPP,
        Self::ENETDOWN,
        Self::ETIMEDOUT,
        Self::EHOSTUNREACH,
        Self::ESTALE,
        Self::EDQUOT,
    ];

    /// The error's number, always positive.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

/// The Linux error for a contract error (vfs-contract 5.12, point 2).
/// The match has no wildcard arm, so a new contract value does not compile
/// until it is mapped (ADR 0004, C46, point 4).
#[must_use]
pub const fn for_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotFound => Errno::ENOENT,
        VfsError::Exists => Errno::EEXIST,
        VfsError::Access
        | VfsError::TokenRequired
        | VfsError::TokenExpired
        | VfsError::TokenRevoked
        | VfsError::SecurityFloor => Errno::EACCES,
        VfsError::ReadOnly => Errno::EROFS,
        VfsError::NoSpace => Errno::ENOSPC,
        VfsError::QuotaExceeded => Errno::EDQUOT,
        VfsError::CrossDevice => Errno::EXDEV,
        VfsError::Stale => Errno::ESTALE,
        VfsError::Busy | VfsError::WouldBlock => Errno::EAGAIN,
        VfsError::Offline => Errno::ENETDOWN,
        VfsError::TimedOut => Errno::ETIMEDOUT,
        VfsError::NotYetStored | VfsError::Protocol => Errno::EIO,
        VfsError::Interrupted => Errno::EINTR,
        VfsError::NotSupported => Errno::EOPNOTSUPP,
        VfsError::Unreachable => Errno::EHOSTUNREACH,
        VfsError::Conflict => Errno::EBUSY,
        VfsError::OutOfRange => Errno::EINVAL,
        VfsError::NotADirectory => Errno::ENOTDIR,
        VfsError::IsADirectory => Errno::EISDIR,
        VfsError::NotEmpty => Errno::ENOTEMPTY,
        VfsError::NoAttribute => Errno::ENODATA,
        VfsError::TooSmall => Errno::ERANGE,
        VfsError::PastEnd => Errno::ENXIO,
    }
}

/// The Linux error for a contract error from a rename with flags: a
/// refused flag is an invalid argument (vfs-contract 5.6, point 9).
#[must_use]
pub const fn for_rename_with_flags(error: VfsError) -> Errno {
    match error {
        VfsError::NotSupported => Errno::EINVAL,
        other => for_error(other),
    }
}

/// The Linux error for bytes that are not a name (vfs-contract 5.12,
/// point 4).
#[must_use]
pub const fn for_name_error(error: NameError) -> Errno {
    match error {
        NameError::TooLong => Errno::ENAMETOOLONG,
        NameError::Empty
        | NameError::Dot
        | NameError::DotDot
        | NameError::Slash
        | NameError::Nul => Errno::EINVAL,
    }
}
