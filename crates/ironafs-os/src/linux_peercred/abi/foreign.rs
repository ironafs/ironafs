// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declaration of site `linux-peercred`.

use core::ffi::{c_int, c_void};

use super::socklen_t;

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
