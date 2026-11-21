// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declarations of site `linux-fdpass`.

use core::ffi::c_int;
use std::io;

use super::msghdr;

/// The errno the last failed call on this thread set.
pub(in crate::linux_fdpass) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `ssize_t sendmsg(int sockfd, const struct msghdr *msg, int
// flags)` and `ssize_t recvmsg(int sockfd, struct msghdr *msg, int
// flags)`, as `sendmsg(2)`, `recvmsg(2)` and `<sys/socket.h>` declare
// them in glibc and musl, with each library's own `struct msghdr`. Both
// are in the C library itself, so no `#[link]` is needed beyond the C
// library the standard library links. Both are declared unsafe to call:
// `sendmsg` reads the message and every buffer it points to, and
// `recvmsg` writes the message's lengths and flags and up to each
// buffer's stated size through its pointers, so each pointer must be
// valid for that; `crate::linux_fdpass` passes only messages over its own
// locals.
#[expect(unsafe_code, reason = "linux-fdpass")]
unsafe extern "C" {
    pub(in crate::linux_fdpass) fn sendmsg(
        sockfd: c_int,
        msg: *const msghdr,
        flags: c_int,
    ) -> isize;
    pub(in crate::linux_fdpass) fn recvmsg(sockfd: c_int, msg: *mut msghdr, flags: c_int) -> isize;
}
