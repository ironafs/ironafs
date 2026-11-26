// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `setrlimit` and `prctl` (ADR 0005, point 6.5),
//! built only under Miri, which cannot call the C library. They take what
//! the wrappers pass and honour the calls' contracts for it: `setrlimit`
//! with `RLIMIT_CORE` reads the `struct rlimit` through its pointer (so
//! Miri checks the pointer) and returns 0; `prctl` keeps one dumpable
//! attribute, set by `PR_SET_DUMPABLE` with 0 or 1 and the other arguments
//! 0, and returned by `PR_GET_DUMPABLE` with every argument 0. Anything
//! else returns -1 with `EINVAL`.
//!
//! A test can [`plan`] an answer instead: a failure of one of the three
//! calls, with an errno (which [`last_error`] then returns, as the C
//! library's errno would be), or a read-back of another value. A plan is
//! used once, by the call it applies to.

use core::ffi::{c_int, c_ulong};
use core::sync::atomic::{AtomicI32, Ordering};
use std::cell::Cell;
use std::io;

use super::{PR_GET_DUMPABLE, PR_SET_DUMPABLE, RLIMIT_CORE, rlimit};

/// The next answer.
#[derive(Clone, Copy, Debug)]
pub(in crate::unix_nodump) enum Plan {
    /// As the kernel answers the wrappers' calls.
    Answer,
    /// `setrlimit` returns -1, with this errno.
    LimitFail(c_int),
    /// `prctl(PR_SET_DUMPABLE)` returns -1, with this errno.
    SetFail(c_int),
    /// `prctl(PR_GET_DUMPABLE)` returns -1, with this errno.
    ReadFail(c_int),
    /// `prctl(PR_GET_DUMPABLE)` returns this value.
    Reads(c_int),
}

thread_local! {
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Answer) };
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
}

/// The stand-in's dumpable attribute: 1, as a process starts.
static DUMPABLE: AtomicI32 = AtomicI32::new(1);

/// Plans the next answer on this thread.
pub(in crate::unix_nodump) fn plan(next: Plan) {
    PLAN.with(|plan| plan.set(next));
}

/// The planned answer if `applies` accepts it, which then uses it; the
/// default answer otherwise.
fn take(applies: impl Fn(Plan) -> bool) -> Plan {
    PLAN.with(|plan| {
        if applies(plan.get()) {
            plan.replace(Plan::Answer)
        } else {
            Plan::Answer
        }
    })
}

/// The errno the last failed call set.
pub(in crate::unix_nodump) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

/// Reads the limit at `rlim` as `setrlimit` would.
///
/// # Safety
///
/// As the foreign function's: `rlim` is valid for one `rlimit` read.
#[expect(unsafe_code, reason = "unix-nodump: Miri stand-in")]
pub(in crate::unix_nodump) unsafe fn setrlimit(resource: c_int, rlim: *const rlimit) -> c_int {
    if resource != RLIMIT_CORE {
        return refused();
    }
    #[expect(unsafe_code, reason = "unix-nodump: Miri stand-in")]
    // SAFETY: the caller promises `rlim` is valid for one `rlimit` read, as
    // the foreign function requires; Miri checks that promise.
    let limit = unsafe { core::ptr::read(rlim) };
    if limit.rlim_cur > limit.rlim_max {
        return refused();
    }
    if let Plan::LimitFail(errno) = take(|plan| matches!(plan, Plan::LimitFail(_))) {
        return failed(errno);
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
            if let Plan::SetFail(errno) = take(|plan| matches!(plan, Plan::SetFail(_))) {
                return failed(errno);
            }
            DUMPABLE.store(if arg2 == 0 { 0 } else { 1 }, Ordering::Relaxed);
            0
        }
        (PR_GET_DUMPABLE, 0) if rest => {
            match take(|plan| matches!(plan, Plan::ReadFail(_) | Plan::Reads(_))) {
                Plan::ReadFail(errno) => failed(errno),
                Plan::Reads(value) => value,
                _ => DUMPABLE.load(Ordering::Relaxed),
            }
        }
        _ => refused(),
    }
}

/// A planned failure: -1, with `errno`.
fn failed(errno: c_int) -> c_int {
    ERRNO.with(|cell| cell.set(errno));
    -1
}

/// `EINVAL`.
const EINVAL: c_int = 22;

/// A call of a shape the stand-in does not model: -1, with `EINVAL` as
/// its errno, so no earlier errno is reported for it.
fn refused() -> c_int {
    ERRNO.with(|cell| cell.set(EINVAL));
    -1
}
