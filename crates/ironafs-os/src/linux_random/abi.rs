// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-random`, named as the C headers name
//! them, written from `getrandom(2)`, `syscall(2)`, `<unistd.h>` and
//! `<errno.h>`, and proved on each Linux target by `cargo xtask check-abi`
//! (ADR 0005, point 5.6). The system-call number differs by architecture
//! and is in `super::number`.

use core::ffi::{c_int, c_long};

/// `EINTR`: interrupted by a signal before any byte was written.
pub(super) const EINTR: c_int = 4;

/// `EPERM`: what a sandbox's filter may answer.
pub(super) const EPERM: c_int = 1;

/// `ENOSYS`: the kernel lacks the call, or a filter answers as if it did.
pub(super) const ENOSYS: c_int = 38;

// SAFETY: `long syscall(long number, ...)`, as `syscall(2)` and
// `<unistd.h>` declare it in glibc and musl; it is in the C library
// itself, so no `#[link]` is needed beyond the C library the standard
// library links. It is declared unsafe to call: the system call it makes
// reads its arguments as the kernel defines them, and `super::getrandom`
// passes exactly `getrandom`'s, each at register width.
#[expect(unsafe_code, reason = "linux-random")]
unsafe extern "C" {
    pub(super) fn syscall(number: c_long, ...) -> c_long;
}
