// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `madvise` (ADR 0005, point 6.5), built only under
//! Miri, which cannot call the C library. It honours the call's contract
//! for what the wrapper passes: given `MADV_DONTDUMP`, a 64 KiB-aligned
//! address and a non-zero length that is a multiple of 64 KiB, it reads
//! the first and the last byte of the range (so Miri checks that the
//! pointer is valid for the whole length) and returns 0; anything else
//! returns -1. It changes no byte, as the advice changes no content.

use core::ffi::{c_int, c_void};

use super::MADV_DONTDUMP;

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
