// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-peercred`, named as the C headers
//! name them, written from `unix(7)`, `getsockopt(2)` and
//! `<sys/socket.h>`, and proved on each Linux target by `cargo xtask
//! check-abi` (ADR 0005, point 5.6), with `foreign`. glibc and musl show
//! `struct ucred` only under `_GNU_SOURCE`, which the check defines for
//! this row. The constants have the same values on both allowlisted Linux
//! architectures.

use core::ffi::c_int;

/// `pid_t`: a signed 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type pid_t = i32;

/// `uid_t`: an unsigned 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type uid_t = u32;

/// `gid_t`: an unsigned 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type gid_t = u32;

/// `socklen_t`: an unsigned 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type socklen_t = u32;

/// `struct ucred`: the peer's process id, user id and group id (unix(7)).
#[repr(C)]
#[derive(Debug)]
pub(super) struct ucred {
    pub(super) pid: pid_t,
    pub(super) uid: uid_t,
    pub(super) gid: gid_t,
}

/// `SOL_SOCKET`: the socket level of options.
pub(super) const SOL_SOCKET: c_int = 1;

/// `SO_PEERCRED`: the peer's credentials, read-only.
pub(super) const SO_PEERCRED: c_int = 17;

// The foreign function, or under Miri, which cannot call it, the in-crate
// stand-in of ADR 0005, point 6.5, which the same call reaches, so the
// wrapper's pointers and the length check run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::getsockopt;
#[cfg(miri)]
pub(super) use stand_in::getsockopt;
