// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declaration of site `linux-clock`.

use core::ffi::c_int;

use super::timespec;

// SAFETY: `int clock_gettime(clockid_t clockid, struct timespec *tp)`, as
// `clock_gettime(2)` documents it; `clockid_t` is `int` in glibc and musl,
// and the function is in the C library itself from glibc 2.17 (before,
// `librt`), so no `#[link]` is needed beyond the C library the standard
// library links. It is declared unsafe to call: `tp` must point to memory
// valid for one `timespec` write, which `crate::linux_clock::read` provides.
#[expect(unsafe_code, reason = "linux-clock")]
unsafe extern "C" {
    pub(in crate::linux_clock) fn clock_gettime(clockid: c_int, tp: *mut timespec) -> c_int;
}
