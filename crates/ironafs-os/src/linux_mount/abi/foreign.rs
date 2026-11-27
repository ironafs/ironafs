// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C library's `mount` declaration for site `linux-mount`.

use core::ffi::{c_char, c_int, c_ulong, c_void};
use std::io;

/// The errno the failed call on this thread set.
pub(in crate::linux_mount) fn last_error() -> io::Error {
    io::Error::last_os_error()
}

// SAFETY: `mount(2)` declares this prototype in <sys/mount.h>. The
// standard library links the C library that provides it. For FUSE the
// four pointers must each name a C string readable through the call.
// The wrapper owns those strings and keeps the device descriptor open.
// The call can run on any thread and sets errno on the calling thread.
#[expect(unsafe_code, reason = "linux-mount")]
unsafe extern "C" {
    pub(in crate::linux_mount) fn mount(
        source: *const c_char,
        target: *const c_char,
        filesystem_type: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
}
