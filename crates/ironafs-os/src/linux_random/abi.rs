// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-random`, named as the C headers name
//! them, written from `getrandom(2)`, `syscall(2)`, `<unistd.h>` and
//! `<errno.h>`, and proved on each Linux target by `cargo xtask check-abi`
//! (ADR 0005, point 5.6), with `foreign`. The system-call number differs
//! by architecture and is in `super::number`.

use core::ffi::c_int;

/// `EINTR`: interrupted by a signal before any byte was written.
pub(super) const EINTR: c_int = 4;

/// `EPERM`: what a sandbox's filter may answer.
pub(super) const EPERM: c_int = 1;

/// `ENOSYS`: the kernel lacks the call, or a filter answers as if it did.
pub(super) const ENOSYS: c_int = 38;

// The foreign function, or under Miri, which cannot call it, the in-crate
// stand-in of ADR 0005, point 6.5, which the same call reaches, so the
// wrapper's arguments and its classification run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::syscall;
#[cfg(miri)]
pub(super) use stand_in::syscall;
