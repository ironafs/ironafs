// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-clock`, named as the C headers name
//! them, written from `clock_gettime(2)` and `<time.h>` and proved on each
//! Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6). Every
//! item of this file is a C fact that check compares.

use core::ffi::{c_int, c_long};

/// `struct timespec`: `time_t tv_sec` and `long tv_nsec`. Both are `long`
/// on the allowlisted 64-bit Linux targets, in glibc and in musl.
#[repr(C)]
#[derive(Debug)]
pub(super) struct timespec {
    pub(super) tv_sec: c_long,
    pub(super) tv_nsec: c_long,
}

/// `CLOCK_MONOTONIC`: does not count time the system is suspended.
pub(super) const CLOCK_MONOTONIC: c_int = 1;

/// `CLOCK_BOOTTIME`: counts time the system is suspended (Linux 2.6.39).
pub(super) const CLOCK_BOOTTIME: c_int = 7;

// SAFETY: `int clock_gettime(clockid_t clockid, struct timespec *tp)`, as
// `clock_gettime(2)` documents it; `clockid_t` is `int` in glibc and musl,
// and the function is in the C library itself from glibc 2.17 (before,
// `librt`), so no `#[link]` is needed beyond the C library the standard
// library links. It is declared unsafe to call: `tp` must point to memory
// valid for one `timespec` write, which `super::read` provides.
#[expect(unsafe_code, reason = "linux-clock")]
unsafe extern "C" {
    pub(super) fn clock_gettime(clockid: c_int, tp: *mut timespec) -> c_int;
}
