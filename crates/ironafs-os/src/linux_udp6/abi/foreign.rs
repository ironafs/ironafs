// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declarations of site `linux-udp6`.

use core::ffi::{c_int, c_void};
use std::io;

use super::{sockaddr, socklen_t};

/// The errno the last failed call on this thread set.
pub(in crate::linux_udp6) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `int socket(int domain, int type, int protocol)`, `int
// setsockopt(int sockfd, int level, int optname, const void *optval,
// socklen_t optlen)` and `int bind(int sockfd, const struct sockaddr
// *addr, socklen_t addrlen)`, as `socket(2)`, `setsockopt(2)`, `bind(2)`
// and `<sys/socket.h>` declare them in glibc and musl (without
// `_GNU_SOURCE`, glibc's `bind` takes `const struct sockaddr *`, not its
// transparent union). All three are in the C library itself, so no
// `#[link]` is needed beyond the C library the standard library links.
// `socket` reads nothing of ours; `setsockopt` reads `optlen` bytes at
// `optval` and `bind` `addrlen` bytes at `addr`, so each pointer must be
// valid for that read; `crate::linux_udp6` passes only its own locals.
#[expect(unsafe_code, reason = "linux-udp6")]
unsafe extern "C" {
    pub(in crate::linux_udp6) fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    pub(in crate::linux_udp6) fn setsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *const c_void,
        optlen: socklen_t,
    ) -> c_int;
    pub(in crate::linux_udp6) fn bind(
        sockfd: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> c_int;
}
