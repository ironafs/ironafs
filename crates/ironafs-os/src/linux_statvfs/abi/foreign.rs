// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declaration of site `linux-statvfs`.

use core::ffi::c_int;
use std::io;

use super::statvfs;

/// The errno the last failed call on this thread set.
pub(in crate::linux_statvfs) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `int fstatvfs(int fd, struct statvfs *buf)`, as `statvfs(3)`
// and `<sys/statvfs.h>` declare it in glibc and musl, with the struct of
// `abi`. It is in the C library itself (and in the x86-64 glibc ABI
// before 2.17, ADR 0005, Context item 8), so no `#[link]` is needed
// beyond the C library the standard library links. It is declared unsafe
// to call: it writes one `struct statvfs` through `buf`, which must be
// valid for that write; `crate::linux_statvfs` passes only its own local.
#[expect(unsafe_code, reason = "linux-statvfs")]
unsafe extern "C" {
    pub(in crate::linux_statvfs) fn fstatvfs(fd: c_int, buf: *mut statvfs) -> c_int;
}
