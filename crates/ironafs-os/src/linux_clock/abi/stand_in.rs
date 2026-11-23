// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `clock_gettime` (ADR 0005, point 6.5), built only
//! under Miri, which cannot call the C library. It has the foreign
//! function's signature and honours its contract: for the two clock ids
//! the wrapper passes it writes one `timespec` through `tp` and returns 0,
//! each read one second later than the last, the boot clock a fixed
//! amount ahead of the awake clock; for any other id it writes nothing and
//! returns -1. So Miri checks the wrapper's own pointer, its exclusive
//! local and the validation after the call.

use core::ffi::{c_int, c_long};
use core::sync::atomic::{AtomicI64, Ordering};

use super::{CLOCK_BOOTTIME, CLOCK_MONOTONIC, timespec};

/// The stand-in's seconds, advanced by each read.
static SECONDS: AtomicI64 = AtomicI64::new(1);

/// How far the stand-in's boot clock is ahead of its awake clock: the time
/// it pretends the machine was suspended.
const SUSPENDED: c_long = 1_000;

/// Writes the next reading of `clockid` through `tp`.
///
/// # Safety
///
/// As the foreign function's: `tp` is valid for one `timespec` write.
#[expect(unsafe_code, reason = "linux-clock: Miri stand-in")]
pub(in crate::linux_clock) unsafe fn clock_gettime(clockid: c_int, tp: *mut timespec) -> c_int {
    let seconds = SECONDS.fetch_add(1, Ordering::Relaxed);
    let tv_sec = match clockid {
        CLOCK_MONOTONIC => seconds,
        CLOCK_BOOTTIME => seconds + SUSPENDED,
        _ => return -1,
    };
    let reading = timespec { tv_sec, tv_nsec: 0 };
    #[expect(unsafe_code, reason = "linux-clock: Miri stand-in")]
    // SAFETY: the caller promises `tp` is valid for one `timespec` write,
    // as the foreign function requires; Miri checks that promise.
    let () = unsafe { core::ptr::write(tp, reading) };
    0
}
