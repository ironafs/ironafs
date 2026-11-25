// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SYS_getrandom` on `x86_64`, as `<sys/syscall.h>` defines it.

use core::ffi::c_long;

/// The system-call number of `getrandom` on `x86_64`.
#[expect(
    non_upper_case_globals,
    reason = "the C name, which the ABI check matches"
)]
pub(in crate::linux_random) const SYS_getrandom: c_long = 318;
