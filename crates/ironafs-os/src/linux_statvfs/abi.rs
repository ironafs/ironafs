// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-statvfs`, named as the C headers name
//! them, written from `statvfs(3)` and `<sys/statvfs.h>`, and proved on
//! each Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6),
//! with `foreign`.
//!
//! `struct statvfs` is one declaration for both C libraries: its named
//! fields have the same names, offsets and types in glibc and musl on
//! these 64-bit targets (`fsblkcnt_t` and `fsfilcnt_t` are typedefs of
//! one size and sign spelled differently, so they are aliases the check
//! asserts by name). Its last 24 bytes differ by library and release
//! (glibc 2.28: `int __f_spare[6]`; glibc 2.41: `unsigned int f_type; int
//! __f_spare[5]`; musl: `unsigned int f_type; int __reserved[5]`), and
//! are never read here, so they are an opaque tail, which the check binds
//! by the struct's size and alignment.

use core::ffi::{c_int, c_ulong};

/// `fsblkcnt_t`: a count of blocks, an unsigned 64-bit integer in glibc
/// (`unsigned long`) and musl (`unsigned long long`).
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type fsblkcnt_t = u64;

/// `fsfilcnt_t`: a count of inodes, as `fsblkcnt_t`.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type fsfilcnt_t = u64;

/// `struct statvfs` (`statvfs(3)`).
#[repr(C)]
#[derive(Debug)]
pub(super) struct statvfs {
    pub(super) f_bsize: c_ulong,
    pub(super) f_frsize: c_ulong,
    pub(super) f_blocks: fsblkcnt_t,
    pub(super) f_bfree: fsblkcnt_t,
    pub(super) f_bavail: fsblkcnt_t,
    pub(super) f_files: fsfilcnt_t,
    pub(super) f_ffree: fsfilcnt_t,
    pub(super) f_favail: fsfilcnt_t,
    pub(super) f_fsid: c_ulong,
    pub(super) f_flag: c_ulong,
    pub(super) f_namemax: c_ulong,
    pub(super) _opaque_tail: [c_int; 6],
}

impl statvfs {
    /// A value with every field zero, for the call to fill.
    pub(super) fn zeroed() -> Self {
        Self {
            f_bsize: 0,
            f_frsize: 0,
            f_blocks: 0,
            f_bfree: 0,
            f_bavail: 0,
            f_files: 0,
            f_ffree: 0,
            f_favail: 0,
            f_fsid: 0,
            f_flag: 0,
            f_namemax: 0,
            _opaque_tail: [0; 6],
        }
    }
}

// The foreign function, or under Miri, which cannot call it, the in-crate
// stand-in of ADR 0005, point 6.5, which the same call reaches, so the
// wrapper's pointer and the arithmetic run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
pub(super) mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{fstatvfs, last_error};
#[cfg(miri)]
pub(super) use stand_in::{fstatvfs, last_error};
