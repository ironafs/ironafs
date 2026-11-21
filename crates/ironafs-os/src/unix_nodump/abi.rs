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

// SAFETY: `int setrlimit(int resource, const struct rlimit *rlim)`, as
// `setrlimit(2)` and POSIX declare it (glibc's GNU mode spells the
// resource as an enum of the same size; musl writes `int`), and `int
// prctl(int op, ...)`, as `prctl(2)` and `<sys/prctl.h>` declare it in
// glibc and musl. Both are in the C library itself, so no `#[link]` is
// needed beyond the C library the standard library links. Both are
// declared unsafe to call: `setrlimit` reads one `struct rlimit` through
// its pointer, and `prctl` reads its variadic arguments as `unsigned long`
// values, which `super` passes as `c_ulong`.
#[expect(unsafe_code, reason = "unix-nodump")]
unsafe extern "C" {
    pub(super) fn setrlimit(resource: c_int, rlim: *const rlimit) -> c_int;
    pub(super) fn prctl(option: c_int, ...) -> c_int;
}
