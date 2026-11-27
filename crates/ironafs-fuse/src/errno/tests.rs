// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the Linux error numbers (fuse-codec.md 5.7).

use ironafs_vfs::{NameError, VfsError};

use super::{Errno, for_error, for_name_error, for_rename_with_flags};

/// Every contract value with the Linux name vfs-contract 5.12, point 2,
/// gives it.
const MAPPING: &[(VfsError, Errno)] = &[
    (VfsError::NotFound, Errno::ENOENT),
    (VfsError::Exists, Errno::EEXIST),
    (VfsError::Access, Errno::EACCES),
    (VfsError::TokenRequired, Errno::EACCES),
    (VfsError::TokenExpired, Errno::EACCES),
    (VfsError::TokenRevoked, Errno::EACCES),
    (VfsError::SecurityFloor, Errno::EACCES),
    (VfsError::ReadOnly, Errno::EROFS),
    (VfsError::NoSpace, Errno::ENOSPC),
    (VfsError::QuotaExceeded, Errno::EDQUOT),
    (VfsError::CrossDevice, Errno::EXDEV),
    (VfsError::Stale, Errno::ESTALE),
    (VfsError::Busy, Errno::EAGAIN),
    (VfsError::Offline, Errno::ENETDOWN),
    (VfsError::TimedOut, Errno::ETIMEDOUT),
    (VfsError::NotYetStored, Errno::EIO),
    (VfsError::Interrupted, Errno::EINTR),
    (VfsError::NotSupported, Errno::EOPNOTSUPP),
    (VfsError::WouldBlock, Errno::EAGAIN),
    (VfsError::Unreachable, Errno::EHOSTUNREACH),
    (VfsError::Conflict, Errno::EBUSY),
    (VfsError::OutOfRange, Errno::EINVAL),
    (VfsError::NotADirectory, Errno::ENOTDIR),
    (VfsError::IsADirectory, Errno::EISDIR),
    (VfsError::NotEmpty, Errno::ENOTEMPTY),
    (VfsError::NoAttribute, Errno::ENODATA),
    (VfsError::TooSmall, Errno::ERANGE),
    (VfsError::PastEnd, Errno::ENXIO),
    (VfsError::Protocol, Errno::EIO),
];

#[test]
fn every_value_maps_to_an_error() {
    for &(error, expected) in MAPPING {
        let errno = for_error(error);
        assert_eq!(errno, expected, "{error:?}");
        assert!(errno.get() > 0, "{error:?} maps to {}", errno.get());
        assert!(Errno::ALL.contains(&errno), "{error:?}");
    }
}

#[test]
fn rename_flags_is_einval() {
    assert_eq!(for_rename_with_flags(VfsError::NotSupported), Errno::EINVAL);
    for &(error, expected) in MAPPING {
        if error != VfsError::NotSupported {
            assert_eq!(for_rename_with_flags(error), expected, "{error:?}");
        }
    }
}

#[test]
fn no_errno_is_enosys() {
    // ENOSYS is 38 in asm-generic/errno.h (E74).
    assert!(Errno::ALL.iter().all(|errno| errno.get() != 38));
}

#[test]
fn numbers_match_the_uapi_headers() {
    let expected = [
        (Errno::ENOENT, 2),
        (Errno::EINTR, 4),
        (Errno::EIO, 5),
        (Errno::ENXIO, 6),
        (Errno::EAGAIN, 11),
        (Errno::EACCES, 13),
        (Errno::EBUSY, 16),
        (Errno::EEXIST, 17),
        (Errno::EXDEV, 18),
        (Errno::ENOTDIR, 20),
        (Errno::EISDIR, 21),
        (Errno::EINVAL, 22),
        (Errno::ENOSPC, 28),
        (Errno::EROFS, 30),
        (Errno::ERANGE, 34),
        (Errno::ENAMETOOLONG, 36),
        (Errno::ENOTEMPTY, 39),
        (Errno::ENODATA, 61),
        (Errno::EPROTO, 71),
        (Errno::EOPNOTSUPP, 95),
        (Errno::ENETDOWN, 100),
        (Errno::ETIMEDOUT, 110),
        (Errno::EHOSTUNREACH, 113),
        (Errno::ESTALE, 116),
        (Errno::EDQUOT, 122),
    ];
    for (errno, number) in expected {
        assert_eq!(errno.get(), number);
    }
    assert_eq!(Errno::ALL.len(), expected.len());
}

#[test]
fn name_errors_map_as_the_contract_says() {
    assert_eq!(for_name_error(NameError::TooLong), Errno::ENAMETOOLONG);
    for error in [
        NameError::Empty,
        NameError::Dot,
        NameError::DotDot,
        NameError::Slash,
        NameError::Nul,
    ] {
        assert_eq!(for_name_error(error), Errno::EINVAL, "{error:?}");
    }
}
