// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declarations of site `linux-rtnetlink`.

use core::ffi::{c_int, c_void};

use super::{msghdr, sockaddr, socklen_t};

// SAFETY: `int socket(int domain, int type, int protocol)`, `int bind(int
// sockfd, const struct sockaddr *addr, socklen_t addrlen)`, `ssize_t
// sendto(int sockfd, const void *buf, size_t len, int flags, const struct
// sockaddr *dest_addr, socklen_t addrlen)` and `ssize_t recvmsg(int
// sockfd, struct msghdr *msg, int flags)`, as `socket(2)`, `bind(2)`,
// `send(2)`, `recvmsg(2)` and `<sys/socket.h>` declare them in glibc and
// musl (without `_GNU_SOURCE`, glibc's address parameters are `const
// struct sockaddr *`, not its transparent union), with each library's own
// `struct msghdr`. All four are in the C library itself, so no `#[link]`
// is needed beyond the C library the standard library links. `socket`
// reads nothing of ours; `bind` reads `addrlen` bytes at `addr`, `sendto`
// `len` bytes at `buf` and `addrlen` at `dest_addr`, and `recvmsg` writes
// the message's address, data, lengths and flags through its pointers, so
// each pointer must be valid for that; `crate::linux_rtnetlink` passes
// only its own locals and its caller's borrowed slices.
#[expect(unsafe_code, reason = "linux-rtnetlink")]
unsafe extern "C" {
    pub(in crate::linux_rtnetlink) fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    pub(in crate::linux_rtnetlink) fn bind(
        sockfd: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> c_int;
    pub(in crate::linux_rtnetlink) fn sendto(
        sockfd: c_int,
        buf: *const c_void,
        len: usize,
        flags: c_int,
        dest_addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> isize;
    pub(in crate::linux_rtnetlink) fn recvmsg(
        sockfd: c_int,
        msg: *mut msghdr,
        flags: c_int,
    ) -> isize;
}
