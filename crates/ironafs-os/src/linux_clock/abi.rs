// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-clock`, named as the C headers name
//! them, written from `clock_gettime(2)` and `<time.h>` and proved on each
//! Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6). Every
//! item of this file and of `foreign` is a C fact that check compares.

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

// The foreign function, or under Miri, which cannot call it, the in-crate
// stand-in of ADR 0005, point 6.5, with the same signature, so the
// wrapper's pointer and its validation run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::clock_gettime;
#[cfg(miri)]
pub(super) use stand_in::clock_gettime;
