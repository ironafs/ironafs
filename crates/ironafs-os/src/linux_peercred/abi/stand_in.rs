// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `getsockopt` (ADR 0005, point 6.5), built only
//! under Miri, which cannot call the C library. It honours the call's
//! contract for what the wrapper passes: given `SOL_SOCKET` and
//! `SO_PEERCRED` and a length of at least `struct ucred`'s size, it writes
//! one `ucred` through `optval` (this process's id, and user and group 0)
//! and its size through `optlen`, and returns 0; anything else writes
//! nothing and returns -1. So Miri checks the wrapper's two pointers.
//!
//! A test can [`plan`] the next call's answer instead: a reported length
//! other than 12, a failure with an errno (which [`last_error`] then
//! returns, as the C library's errno would be), ids of -1 (no peer
//! credentials), or no connected peer. A plan is used once.

use core::ffi::{c_int, c_void};
use std::cell::Cell;
use std::io;
use std::os::unix::net::UnixStream;

use super::{SO_PEERCRED, SOL_SOCKET, socklen_t, ucred};

/// The next call's answer.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_peercred) enum Plan {
    /// As the kernel answers a connected Unix stream.
    Credentials,
    /// Credentials, reported as this many bytes.
    Length(socklen_t),
    /// -1, with this errno.
    Fail(c_int),
    /// Ids of -1, as for a socket with no peer credentials.
    NoCredentials,
    /// No connected peer: `connected` fails with `ENOTCONN`.
    NotConnected,
}

thread_local! {
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Credentials) };
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
}

/// `ENOTCONN`.
const ENOTCONN: c_int = 107;

/// Plans the next call's answer on this thread.
pub(in crate::linux_peercred) fn plan(next: Plan) {
    PLAN.with(|plan| plan.set(next));
}

/// The planned answer, which is used once.
fn take() -> Plan {
    PLAN.with(|plan| plan.replace(Plan::Credentials))
}

/// The planned peer check: fails for a planned `NotConnected`.
pub(in crate::linux_peercred) fn connected(_stream: &UnixStream) -> io::Result<()> {
    if matches!(PLAN.with(Cell::get), Plan::NotConnected) {
        take();
        return Err(io::Error::from_raw_os_error(ENOTCONN));
    }
    Ok(())
}

/// The errno the last planned failure set.
pub(in crate::linux_peercred) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

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
        return refused();
    }
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: the caller promises `optlen` is valid for one read; Miri
    // checks it.
    let room = unsafe { core::ptr::read(optlen) };
    if room < SIZE {
        return refused();
    }
    let pid = i32::try_from(std::process::id()).unwrap_or(-1);
    let (length, uid) = match take() {
        Plan::Fail(errno) => {
            ERRNO.with(|cell| cell.set(errno));
            return -1;
        }
        Plan::Length(length) => (length, 0),
        Plan::NoCredentials => (SIZE, u32::MAX),
        Plan::Credentials | Plan::NotConnected => (SIZE, 0),
    };
    let credentials = ucred { pid, uid, gid: uid };
    let value: *mut ucred = optval.cast();
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: `optval` is valid for `room` bytes of writes, at least one
    // `ucred`, and the wrapper passes a `ucred` it owns, so it is aligned;
    // Miri checks both.
    let () = unsafe { core::ptr::write(value, credentials) };
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: as the read above, for one write.
    let () = unsafe { core::ptr::write(optlen, length) };
    0
}

/// `EINVAL`.
const EINVAL: c_int = 22;

/// A call of a shape the stand-in does not model: -1, with `EINVAL` as
/// its errno, so no earlier errno (such as a planned `EINTR`) is reported
/// for it.
fn refused() -> c_int {
    ERRNO.with(|cell| cell.set(EINVAL));
    -1
}
