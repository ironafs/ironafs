// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `setrlimit` and `prctl` (ADR 0005, point 6.5),
//! built only under Miri, which cannot call the C library. They take what
//! the wrappers pass and honour the calls' contracts for it: `setrlimit`
//! with `RLIMIT_CORE` reads the `struct rlimit` through its pointer (so
//! Miri checks the pointer) and returns 0, and -1 for any other resource;
//! `prctl` keeps one dumpable attribute, set by `PR_SET_DUMPABLE` with 0
//! or 1 and the other arguments 0, and returned by `PR_GET_DUMPABLE` with
//! every argument 0; anything else returns -1.

use core::ffi::{c_int, c_ulong};
use core::sync::atomic::{AtomicI32, Ordering};

use super::{PR_GET_DUMPABLE, PR_SET_DUMPABLE, RLIMIT_CORE, rlimit};

/// The stand-in's dumpable attribute: 1, as a process starts.
static DUMPABLE: AtomicI32 = AtomicI32::new(1);

/// Reads the limit at `rlim` as `setrlimit` would.
///
/// # Safety
///
/// As the foreign function's: `rlim` is valid for one `rlimit` read.
#[expect(unsafe_code, reason = "unix-nodump: Miri stand-in")]
pub(in crate::unix_nodump) unsafe fn setrlimit(resource: c_int, rlim: *const rlimit) -> c_int {
    if resource != RLIMIT_CORE {
        return -1;
    }
    #[expect(unsafe_code, reason = "unix-nodump: Miri stand-in")]
    // SAFETY: the caller promises `rlim` is valid for one `rlimit` read, as
    // the foreign function requires; Miri checks that promise.
    let limit = unsafe { core::ptr::read(rlim) };
    if limit.rlim_cur > limit.rlim_max {
        return -1;
    }
    0
}

/// Sets or reads the stand-in's dumpable attribute as `prctl` would.
///
/// # Safety
///
/// None beyond the foreign function's, which reads no pointer for these
/// operations; it is `unsafe` only to match it.
#[expect(unsafe_code, reason = "unix-nodump: Miri stand-in")]
pub(in crate::unix_nodump) unsafe fn prctl(
    option: c_int,
    arg2: c_ulong,
    arg3: c_ulong,
    arg4: c_ulong,
    arg5: c_ulong,
) -> c_int {
    let rest = [arg3, arg4, arg5] == [0, 0, 0];
    match (option, arg2) {
        (PR_SET_DUMPABLE, 0 | 1) if rest => {
            DUMPABLE.store(if arg2 == 0 { 0 } else { 1 }, Ordering::Relaxed);
            0
        }
        (PR_GET_DUMPABLE, 0) if rest => DUMPABLE.load(Ordering::Relaxed),
        _ => -1,
    }
}
