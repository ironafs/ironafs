// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `socket`, `setsockopt` and `bind` (ADR 0005,
//! point 6.5), built only under Miri, which cannot make a network socket.
//! `socket` returns one end of a fresh local socket pair (a descriptor
//! Miri knows, so adopting and closing it is checked); `setsockopt` reads
//! its option value through the wrapper's pointer for the stated length;
//! `bind` reads the whole `struct sockaddr_in6` through the wrapper's
//! pointer when the length is its size. Each records the call, in order,
//! on this thread, and a test can make the next of one of them fail once
//! with an errno, which [`last_error`] then returns, as the C library's
//! would be.
//!
//! A call of a shape a stand-in does not model is refused with -1 and
//! `EINVAL`, so no earlier errno is reported for it.

use core::ffi::{c_int, c_void};
use std::cell::{Cell, RefCell};
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;

use super::{sockaddr, sockaddr_in6, socklen_t};
pub(in crate::linux_udp6) use crate::stand_in_errno::last_error;
use crate::stand_in_errno::{refused, set_errno};

/// One call a stand-in received, with what it read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::linux_udp6) enum Call {
    Socket(c_int, c_int, c_int),
    /// The level, the option, the value read and the length.
    Option(c_int, c_int, c_int, socklen_t),
    /// The address read and the length.
    Bind(sockaddr_in6, socklen_t),
}

/// Which call fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::linux_udp6) enum Fail {
    Socket,
    Option,
    Bind,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static FAIL: Cell<Option<(Fail, c_int)>> = const { Cell::new(None) };
}

/// The calls made on this thread so far, which it then forgets.
pub(in crate::linux_udp6) fn calls() -> Vec<Call> {
    CALLS.with(|calls| calls.take())
}

/// Makes the next `call` on this thread fail once, with -1 and `errno`.
pub(in crate::linux_udp6) fn fail(call: Fail, errno: c_int) {
    FAIL.with(|fail| fail.set(Some((call, errno))));
}

fn record(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}

/// Whether `call` is planned to fail; if so, the plan is used up and its
/// errno set.
fn failing(call: Fail) -> bool {
    match FAIL.with(Cell::get) {
        Some((planned, errno)) if planned == call => {
            FAIL.with(|fail| fail.set(None));
            set_errno(errno);
            true
        }
        _ => false,
    }
}

/// Makes a descriptor as `socket` would.
///
/// # Safety
///
/// None beyond the foreign function's: it takes integers.
#[expect(unsafe_code, reason = "linux-udp6: Miri stand-in")]
pub(in crate::linux_udp6) unsafe fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int {
    record(Call::Socket(domain, kind, protocol));
    if failing(Fail::Socket) {
        return -1;
    }
    match UnixStream::pair() {
        Ok((ours, _peer)) => ours.into_raw_fd(),
        Err(_) => refused(),
    }
}

/// Reads the option value as `setsockopt` would.
///
/// # Safety
///
/// As the foreign function's: `optval` is valid for reads of `optlen`
/// bytes. And, since the stand-in reads it as one `int` when `optlen` is
/// `sizeof(int)`, `optval` is then aligned for `c_int`: the wrapper passes
/// a pointer to its own `c_int` local.
#[expect(unsafe_code, reason = "linux-udp6: Miri stand-in")]
pub(in crate::linux_udp6) unsafe fn setsockopt(
    sockfd: c_int,
    level: c_int,
    optname: c_int,
    optval: *const c_void,
    optlen: socklen_t,
) -> c_int {
    if sockfd < 0 || usize::try_from(optlen) != Ok(size_of::<c_int>()) {
        return refused();
    }
    let pointer: *const c_int = optval.cast();
    #[expect(unsafe_code, reason = "linux-udp6: Miri stand-in")]
    // SAFETY: the caller promises `optval` is valid for `optlen` bytes,
    // one `int`, and aligned for it (`# Safety`); Miri checks both.
    let value = unsafe { core::ptr::read(pointer) };
    record(Call::Option(level, optname, value, optlen));
    if failing(Fail::Option) { -1 } else { 0 }
}

/// Reads the address as `bind` would.
///
/// # Safety
///
/// As the foreign function's: `addr` is valid for reads of `addrlen`
/// bytes. And, since the stand-in reads it as one `struct sockaddr_in6`
/// when `addrlen` is that struct's size, `addr` is then aligned for it:
/// the wrapper passes a pointer to a `sockaddr_in6` it borrows.
#[expect(unsafe_code, reason = "linux-udp6: Miri stand-in")]
pub(in crate::linux_udp6) unsafe fn bind(
    sockfd: c_int,
    addr: *const sockaddr,
    addrlen: socklen_t,
) -> c_int {
    if sockfd < 0 || usize::try_from(addrlen) != Ok(size_of::<sockaddr_in6>()) {
        return refused();
    }
    let pointer: *const sockaddr_in6 = addr.cast();
    #[expect(unsafe_code, reason = "linux-udp6: Miri stand-in")]
    // SAFETY: the caller promises `addr` is valid for `addrlen` bytes, one
    // `struct sockaddr_in6`, and aligned for it (`# Safety`); Miri checks
    // both.
    let name = unsafe { core::ptr::read(pointer) };
    record(Call::Bind(name, addrlen));
    if failing(Fail::Bind) { -1 } else { 0 }
}
