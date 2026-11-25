// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The foreign declaration of site `linux-random`.

use core::ffi::c_long;

// SAFETY: `long syscall(long number, ...)`, as `syscall(2)` and
// `<unistd.h>` declare it in glibc and musl; it is in the C library
// itself, so no `#[link]` is needed beyond the C library the standard
// library links. It is declared unsafe to call. Neither the manual page
// nor the header states the types of the variadic arguments, so the
// caller passes every one as the declared type of the fixed parameter,
// `long`, and passes six of them, as many as any system call takes: then
// a C library that reads up to six `long`s after the number reads exactly
// what was passed, and the C rule for variadic arguments (C11 7.16.1.1:
// the type read must be compatible with the type passed) holds whichever
// way it reads them. `crate::linux_random::arguments` builds them.
#[expect(unsafe_code, reason = "linux-random")]
unsafe extern "C" {
    pub(in crate::linux_random) fn syscall(number: c_long, ...) -> c_long;
}
