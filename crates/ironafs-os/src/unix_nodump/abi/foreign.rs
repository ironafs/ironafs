// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declarations of site `unix-nodump` on Linux.

use core::ffi::c_int;
use std::io;

use super::rlimit;

/// The errno the last failed call on this thread set.
pub(in crate::unix_nodump) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `int setrlimit(int resource, const struct rlimit *rlim)`, as
// `setrlimit(2)` and POSIX declare it (glibc's GNU mode spells the
// resource as an enum of the same size; musl writes `int`), and `int
// prctl(int op, ...)`, as `prctl(2)` and `<sys/prctl.h>` declare it in
// glibc and musl. Both are in the C library itself, so no `#[link]` is
// needed beyond the C library the standard library links. Both are
// declared unsafe to call: `setrlimit` reads one `struct rlimit` through
// its pointer, and `prctl` reads its variadic arguments as `unsigned long`
// values, which `crate::unix_nodump` passes as `c_ulong`.
#[expect(unsafe_code, reason = "unix-nodump")]
unsafe extern "C" {
    pub(in crate::unix_nodump) fn setrlimit(resource: c_int, rlim: *const rlimit) -> c_int;
    pub(in crate::unix_nodump) fn prctl(option: c_int, ...) -> c_int;
}
