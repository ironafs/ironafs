// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `unix-nodump` on Linux, named as the C
//! headers name them, written from `setrlimit(2)`, `PR_SET_DUMPABLE(2const)`,
//! `PR_GET_DUMPABLE(2const)`, `<sys/resource.h>` and `<sys/prctl.h>`, and
//! proved on each Linux target by `cargo xtask check-abi` (ADR 0005, point
//! 5.6). The constants have the same values on both allowlisted Linux
//! architectures.

use core::ffi::c_int;

/// `rlim_t`, "an unsigned integer type" (`setrlimit(2)`): 64 bits on the
/// allowlisted targets, spelled `unsigned long` in glibc and `unsigned long
/// long` in musl, so the ABI check names the typedef itself.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type rlim_t = u64;

/// `struct rlimit`: the soft limit, then the hard limit.
#[repr(C)]
#[derive(Debug)]
pub(super) struct rlimit {
    pub(super) rlim_cur: rlim_t,
    pub(super) rlim_max: rlim_t,
}

/// `RLIMIT_CORE`: the largest core file, in bytes; 0 means none.
pub(super) const RLIMIT_CORE: c_int = 4;

/// `PR_GET_DUMPABLE`: returns the dumpable attribute.
pub(super) const PR_GET_DUMPABLE: c_int = 3;

/// `PR_SET_DUMPABLE`: sets the dumpable attribute to 0 or 1.
pub(super) const PR_SET_DUMPABLE: c_int = 4;

// The foreign functions, or under Miri, which cannot call them, the
// in-crate stand-ins of ADR 0005, point 6.5, which the same calls reach,
// so the wrappers' arguments and the read-back run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{prctl, setrlimit};
#[cfg(miri)]
pub(super) use stand_in::{prctl, setrlimit};
