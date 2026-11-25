// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `getsockopt` (ADR 0005, point 6.5), built only
//! under Miri, which cannot call the C library. It honours the call's
//! contract for what the wrapper passes: given `SOL_SOCKET` and `SO_TYPE`
//! and a length of at least an `int`'s size, it writes `SOCK_STREAM`
//! through `optval` and 4 through `optlen`; given `SOL_SOCKET` and
//! `SO_PEERCRED` and a length of at least `struct ucred`'s size, it writes
//! one `ucred` through `optval` (this process's id, and user and group 0)
//! and its size through `optlen`; and returns 0. Anything else writes
//! nothing and returns -1. So Miri checks the wrapper's pointers.
//!
//! A test can [`plan`] an answer instead: for the type read, another type,
//! a reported length other than 4, or a failure; for the credential read,
//! a reported length other than 12, a failure, or ids of -1 (no peer
//! credentials); or no connected peer. A failure's errno is what
//! [`last_error`] then returns, as the C library's errno would be. A plan
//! is used once, by the first call it applies to.

use core::ffi::{c_int, c_void};
use std::cell::Cell;
use std::io;
use std::os::unix::net::UnixStream;

use super::{SO_PEERCRED, SO_TYPE, SOCK_STREAM, SOL_SOCKET, socklen_t, ucred};

/// The next call's answer.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_peercred) enum Plan {
    /// As the kernel answers a connected Unix stream.
    Credentials,
    /// The type read answers this type.
    Type(c_int),
    /// The type read answers `SOCK_STREAM`, reported as this many bytes.
    TypeLength(socklen_t),
    /// The type read returns -1, with this errno.
    TypeFail(c_int),
    /// Credentials, reported as this many bytes.
    Length(socklen_t),
    /// The credential read returns -1, with this errno.
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

/// The planned answer if `applies` accepts it, which then uses it; the
/// default answer otherwise.
fn take(applies: impl Fn(Plan) -> bool) -> Plan {
    PLAN.with(|plan| {
        if applies(plan.get()) {
            plan.replace(Plan::Credentials)
        } else {
            Plan::Credentials
        }
    })
}

/// The planned peer check: fails for a planned `NotConnected`.
pub(in crate::linux_peercred) fn connected(_stream: &UnixStream) -> io::Result<()> {
    if matches!(
        take(|plan| matches!(plan, Plan::NotConnected)),
        Plan::NotConnected
    ) {
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

/// An `int`'s size, as the call reports it.
const INT: socklen_t = 4;

/// Writes the socket's type, or this process's credentials, through
/// `optval` as `getsockopt` with `SO_TYPE` or `SO_PEERCRED` would.
///
/// # Safety
///
/// As the foreign function's: `optlen` is valid for a read and a write of
/// one `socklen_t`, and `optval` for writes of `*optlen` bytes, aligned
/// for the option's value.
#[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
pub(in crate::linux_peercred) unsafe fn getsockopt(
    sockfd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut c_void,
    optlen: *mut socklen_t,
) -> c_int {
    let needed = match optname {
        SO_TYPE => INT,
        SO_PEERCRED => SIZE,
        _ => return refused(),
    };
    if sockfd < 0 || level != SOL_SOCKET {
        return refused();
    }
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: the caller promises `optlen` is valid for one read; Miri
    // checks it.
    let room = unsafe { core::ptr::read(optlen) };
    if room < needed {
        return refused();
    }
    let length = if optname == SO_TYPE {
        let (length, kind) = match take(|plan| {
            matches!(
                plan,
                Plan::Type(_) | Plan::TypeLength(_) | Plan::TypeFail(_)
            )
        }) {
            Plan::TypeFail(errno) => return failed(errno),
            Plan::Type(kind) => (INT, kind),
            Plan::TypeLength(length) => (length, SOCK_STREAM),
            _ => (INT, SOCK_STREAM),
        };
        let value: *mut c_int = optval.cast();
        #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
        // SAFETY: `optval` is valid for `room` bytes of writes, at least
        // one `int`, and the wrapper passes an `int` it owns, so it is
        // aligned; Miri checks both.
        let () = unsafe { core::ptr::write(value, kind) };
        length
    } else {
        let pid = i32::try_from(std::process::id()).unwrap_or(-1);
        let (length, uid) = match take(|plan| {
            matches!(plan, Plan::Length(_) | Plan::Fail(_) | Plan::NoCredentials)
        }) {
            Plan::Fail(errno) => return failed(errno),
            Plan::Length(length) => (length, 0),
            Plan::NoCredentials => (SIZE, u32::MAX),
            _ => (SIZE, 0),
        };
        let credentials = ucred { pid, uid, gid: uid };
        let value: *mut ucred = optval.cast();
        #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
        // SAFETY: `optval` is valid for `room` bytes of writes, at least
        // one `ucred`, and the wrapper passes a `ucred` it owns, so it is
        // aligned; Miri checks both.
        let () = unsafe { core::ptr::write(value, credentials) };
        length
    };
    #[expect(unsafe_code, reason = "linux-peercred: Miri stand-in")]
    // SAFETY: as the read above, for one write.
    let () = unsafe { core::ptr::write(optlen, length) };
    0
}

/// A planned failure: -1, with `errno`.
fn failed(errno: c_int) -> c_int {
    ERRNO.with(|cell| cell.set(errno));
    -1
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
