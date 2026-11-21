// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `madvise` (ADR 0005, point 6.5), built only under
//! Miri, which cannot call the C library. It honours the call's contract
//! for what the wrapper passes: given `MADV_DONTDUMP`, a 64 KiB-aligned
//! address and a non-zero length that is a multiple of 64 KiB, it reads
//! the first and the last byte of the range (so Miri checks that the
//! pointer is valid for the whole length) and returns 0; anything else
//! returns -1. It changes no byte, as the advice changes no content.
//!
//! A test can [`plan`] the next call to fail with an errno, which
//! [`last_error`] then returns, as the C library's errno would be; and
//! [`last_release`] says whether the last buffer released was all zero.

use core::ffi::{c_int, c_void};
use std::cell::Cell;
use std::io;

use super::MADV_DONTDUMP;

/// The next call's answer.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_dontdump) enum Plan {
    /// As the kernel answers the wrapper's call.
    Advise,
    /// -1, with this errno.
    Fail(c_int),
}

thread_local! {
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Advise) };
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
    static RELEASE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Plans the next call's answer on this thread; it is used once.
pub(in crate::linux_dontdump) fn plan(next: Plan) {
    PLAN.with(|plan| plan.set(next));
}

/// The errno the last planned failure set.
pub(in crate::linux_dontdump) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

/// Records whether the memory released is all zero.
pub(in crate::linux_dontdump) fn released(memory: &[u8]) {
    RELEASE.with(|release| release.set(Some(memory.iter().all(|byte| *byte == 0))));
}

/// Whether the last memory released on this thread was all zero.
pub(in crate::linux_dontdump) fn last_release() -> Option<bool> {
    RELEASE.with(Cell::get)
}

/// The block the wrapper advises: 64 KiB in size and alignment.
const BLOCK: usize = 64 * 1024;

/// Checks the range at `addr` as `madvise` with `MADV_DONTDUMP` would.
///
/// # Safety
///
/// As the foreign function's for this advice: `addr` is valid for reads
/// of `length` bytes.
#[expect(unsafe_code, reason = "linux-dontdump: Miri stand-in")]
pub(in crate::linux_dontdump) unsafe fn madvise(
    addr: *mut c_void,
    length: usize,
    advice: c_int,
) -> c_int {
    let shape =
        advice == MADV_DONTDUMP && addr.addr() % BLOCK == 0 && length != 0 && length % BLOCK == 0;
    if !shape {
        return refused();
    }
    if let Plan::Fail(errno) = PLAN.with(|plan| plan.replace(Plan::Advise)) {
        ERRNO.with(|cell| cell.set(errno));
        return -1;
    }
    let bytes: *const u8 = addr.cast_const().cast();
    #[expect(unsafe_code, reason = "linux-dontdump: Miri stand-in")]
    // SAFETY: the caller promises `addr` is valid for reads of `length`
    // bytes; Miri checks that promise. The range is non-empty.
    let range = unsafe { core::slice::from_raw_parts(bytes, length) };
    let _ = (range.first(), range.last());
    0
}

/// `EINVAL`.
const EINVAL: c_int = 22;

/// A call of a shape the stand-in does not model: -1, with `EINVAL` as
/// its errno, so no earlier errno (such as a planned `EINTR`) is reported
/// for it.
fn refused() -> c_int {
    ERRNO.with(|cell| cell.set(EINVAL));
    -1
}
