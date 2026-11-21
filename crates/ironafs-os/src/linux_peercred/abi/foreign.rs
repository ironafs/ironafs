// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declaration of site `linux-peercred`.

use core::ffi::{c_int, c_void};
use std::io;
use std::os::unix::net::UnixStream;

use super::socklen_t;

/// Whether `stream` has a connected Unix peer: the standard library's
/// `getpeername`, which a listener, an unconnected socket and a socket of
/// another family fail.
pub(in crate::linux_peercred) fn connected(stream: &UnixStream) -> io::Result<()> {
    stream.peer_addr().map(drop)
}

/// The errno the last failed call on this thread set.
pub(in crate::linux_peercred) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `int getsockopt(int sockfd, int level, int optname, void
// *optval, socklen_t *optlen)`, as `getsockopt(2)` and `<sys/socket.h>`
// declare it in glibc and musl (musl adds `restrict` to the two pointers,
// which does not change the function's type). It is in the C library
// itself, so no `#[link]` is needed beyond the C library the standard
// library links. It is declared unsafe to call: it writes up to `*optlen`
// bytes through `optval` and writes `*optlen`, so both pointers must be
// valid for those writes; `crate::linux_peercred` passes only its own
// locals, and only `SOL_SOCKET` with `SO_PEERCRED`.
#[expect(unsafe_code, reason = "linux-peercred")]
unsafe extern "C" {
    pub(in crate::linux_peercred) fn getsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut socklen_t,
    ) -> c_int;
}
