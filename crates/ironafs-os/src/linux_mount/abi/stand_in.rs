// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `mount`. A test plans the expected arguments and
//! the result on its thread. Each read stops at the first mismatch or the
//! expected NUL, so the stand-in examines only the expected string bytes.

use core::ffi::{c_char, c_int, c_ulong, c_void};
use std::cell::{Cell, RefCell};
use std::ffi::CString;

pub(in crate::linux_mount) use crate::stand_in_errno::last_error;
use crate::stand_in_errno::{refused, set_errno};

thread_local! {
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

/// The next call's pathname, options, return value and errno.
pub(in crate::linux_mount) struct Plan {
    pub(in crate::linux_mount) target: CString,
    pub(in crate::linux_mount) options: CString,
    pub(in crate::linux_mount) status: c_int,
    pub(in crate::linux_mount) errno: c_int,
}

/// Sets the next call's plan on this thread.
pub(in crate::linux_mount) fn plan(next: Plan) {
    PLAN.with(|plan| plan.replace(Some(next)));
}

/// Counts every call on this thread, including a refused one.
pub(in crate::linux_mount) fn calls() -> usize {
    CALLS.with(Cell::get)
}

/// Checks the call's actual pointers and flags and returns the planned result.
///
/// # Safety
///
/// As for `mount(2)` with type `fuse`, the four pointers must name
/// readable C strings for the duration of the call.
#[expect(unsafe_code, reason = "linux-mount: Miri stand-in")]
pub(in crate::linux_mount) unsafe fn mount(
    source: *const c_char,
    target: *const c_char,
    filesystem_type: *const c_char,
    flags: c_ulong,
    data: *const c_void,
) -> c_int {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    let Some(plan) = PLAN.with(|plan| plan.take()) else {
        return refused();
    };
    let strings = [
        (source, c"ironafs"),
        (target, plan.target.as_c_str()),
        (filesystem_type, c"fuse"),
        (data.cast(), plan.options.as_c_str()),
    ];
    if flags != 6
        || !strings.into_iter().all(|(pointer, expected)| {
            expected
                .to_bytes_with_nul()
                .iter()
                .enumerate()
                .all(|(index, expected)| {
                    let byte = pointer.cast::<u8>().wrapping_add(index);
                    #[expect(unsafe_code, reason = "linux-mount: Miri stand-in")]
                    // SAFETY: the caller supplies a readable C string. Each read
                    // before this one matched a non-NUL byte, and the comparison
                    // ends at the first mismatch or the expected terminator.
                    let found = unsafe { core::ptr::read(byte) };
                    found == *expected
                })
        })
    {
        return refused();
    }
    set_errno(plan.errno);
    plan.status
}
