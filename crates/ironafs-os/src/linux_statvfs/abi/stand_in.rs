// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `fstatvfs` (ADR 0005, point 6.5), built only
//! under Miri, which cannot call the C library. Given a non-negative
//! descriptor it writes one whole `struct statvfs` through `buf` (1000
//! free blocks of 4096 bytes, every other field non-zero, the opaque tail
//! included) and returns 0; given a negative one it writes nothing and
//! returns -1. So Miri checks the wrapper's pointer for the struct's full
//! size. A test can [`plan`] the next calls' answers instead: interrupted
//! by a signal some number of times (-1 with `EINTR`), or failed once with
//! an errno; [`last_error`] returns that errno, as the C library's would
//! be, and [`calls`] counts the calls made.
//!
//! A call of a shape a stand-in does not model is refused with -1 and
//! `EINVAL`, so no earlier errno is reported for it and a retry cannot
//! loop.

use core::ffi::c_int;
use std::cell::Cell;
use std::io;

use super::statvfs;

thread_local! {
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Answer) };
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

/// `EINTR`.
const EINTR: c_int = 4;

/// The next calls' answer.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_statvfs) enum Plan {
    /// As the kernel answers.
    Answer,
    /// -1 with `EINTR` for this many calls, then as the kernel answers.
    Interrupt(usize),
    /// -1 with this errno, once.
    Fail(c_int),
}

/// Plans the next calls' answer on this thread.
pub(in crate::linux_statvfs) fn plan(next: Plan) {
    PLAN.with(|plan| plan.set(next));
}

/// The calls made on this thread.
pub(in crate::linux_statvfs) fn calls() -> usize {
    CALLS.with(Cell::get)
}

/// The errno the last planned failure set.
pub(in crate::linux_statvfs) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

/// Counts a call and answers it as planned: `true` for a planned failure,
/// with its errno set.
fn planned_failure() -> bool {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    let errno = match PLAN.with(Cell::get) {
        Plan::Answer => return false,
        Plan::Interrupt(left) => {
            let next = if left > 1 {
                Plan::Interrupt(left - 1)
            } else {
                Plan::Answer
            };
            PLAN.with(|plan| plan.set(next));
            EINTR
        }
        Plan::Fail(errno) => {
            PLAN.with(|plan| plan.set(Plan::Answer));
            errno
        }
    };
    ERRNO.with(|cell| cell.set(errno));
    true
}

/// The free blocks and fragment size the stand-in reports.
pub(in crate::linux_statvfs) const BLOCKS: u64 = 1000;
pub(in crate::linux_statvfs) const FRAGMENT: u64 = 4096;

/// Writes a filesystem's figures through `buf` as `fstatvfs` would.
///
/// # Safety
///
/// As the foreign function's: `buf` is valid for a write of one
/// `struct statvfs`.
#[expect(unsafe_code, reason = "linux-statvfs: Miri stand-in")]
pub(in crate::linux_statvfs) unsafe fn fstatvfs(fd: c_int, buf: *mut statvfs) -> c_int {
    if fd < 0 {
        return refused();
    }
    if planned_failure() {
        return -1;
    }
    let figures = statvfs {
        f_bsize: 4096,
        f_frsize: FRAGMENT,
        f_blocks: 2000,
        f_bfree: 1100,
        f_bavail: BLOCKS,
        f_files: 500,
        f_ffree: 400,
        f_favail: 400,
        f_fsid: 1,
        f_flag: 1,
        f_namemax: 255,
        _opaque_tail: [-1; 6],
    };
    #[expect(unsafe_code, reason = "linux-statvfs: Miri stand-in")]
    // SAFETY: the caller promises `buf` is valid for one write of the
    // struct, aligned; Miri checks it.
    let () = unsafe { core::ptr::write(buf, figures) };
    0
}

/// `EINVAL`.
const EINVAL: c_int = 22;

/// A call of a shape the stand-in does not model: -1, with `EINVAL` as
/// its errno, so no earlier errno (such as a planned `EINTR`) is reported
/// for it.
fn refused<T: From<i8>>() -> T {
    ERRNO.with(|cell| cell.set(EINVAL));
    T::from(-1)
}
