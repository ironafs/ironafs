// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The errno of the Miri stand-ins of `linux-udp6`, `linux-statvfs` and
//! `linux-rtnetlink` (ADR 0005, point 6.5): set by a planned failure or a
//! refused call, on this thread, and read back as the C library's errno
//! would be. It holds no site.

use core::ffi::c_int;
use std::cell::Cell;
use std::io;

thread_local! {
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
}

/// Sets this thread's errno.
pub(crate) fn set_errno(errno: c_int) {
    ERRNO.with(|cell| cell.set(errno));
}

/// The errno the last planned failure or refusal set.
pub(crate) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

/// `EINVAL`.
const EINVAL: c_int = 22;

/// A call of a shape a stand-in does not model: -1, with `EINVAL` as its
/// errno, so no earlier errno (such as a planned failure's) is reported
/// for it, and a caller that retries on `EINTR` cannot loop.
pub(crate) fn refused<T: From<i8>>() -> T {
    set_errno(EINVAL);
    T::from(-1)
}
