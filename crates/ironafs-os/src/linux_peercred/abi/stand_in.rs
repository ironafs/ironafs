// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `getsockopt` (ADR 0005, point 6.5), built only
//! under Miri, which cannot call the C library. It honours the call's
//! contract for what the wrapper passes: given `SOL_SOCKET` and
//! `SO_PEERCRED` and a length of at least `struct ucred`'s size, it writes
//! one `ucred` through `optval` (this process's id, and user and group 0)
//! and its size through `optlen`, and returns 0; anything else writes
//! nothing and returns -1. So Miri checks the wrapper's two pointers.

use core::ffi::{c_int, c_void};

use super::{SO_PEERCRED, SOL_SOCKET, socklen_t, ucred};

/// `struct ucred`'s size, as the call reports it.
const SIZE: socklen_t = 12;

/// Writes this process's credentials through `optval` as `getsockopt`
/// with `SO_PEERCRED` would.
///
/// # Safety
///
/// As the foreign function's: `optlen` is valid for a read and a write of
/// one `socklen_t`, and `optval` for writes of `*optlen` bytes.
#[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
pub(in crate::linux_peercred) unsafe fn getsockopt(
    sockfd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut c_void,
    optlen: *mut socklen_t,
) -> c_int {
    if sockfd < 0 || level != SOL_SOCKET || optname != SO_PEERCRED {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: the caller promises `optlen` is valid for one read; Miri
    // checks it.
    let room = unsafe { core::ptr::read(optlen) };
    if room < SIZE {
        return -1;
    }
    let pid = i32::try_from(std::process::id()).unwrap_or(-1);
    let credentials = ucred {
        pid,
        uid: 0,
        gid: 0,
    };
    let value: *mut ucred = optval.cast();
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: `optval` is valid for `room` bytes of writes, at least one
    // `ucred`, and the wrapper passes a `ucred` it owns, so it is aligned;
    // Miri checks both.
    let () = unsafe { core::ptr::write(value, credentials) };
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: as the read above, for one write.
    let () = unsafe { core::ptr::write(optlen, SIZE) };
    0
}
