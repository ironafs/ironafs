// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-dontdump`, named as the C headers
//! name them, written from `madvise(2)` and `<sys/mman.h>`, and proved on
//! each Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6).

use core::ffi::{c_int, c_void};

/// `MADV_DONTDUMP`: exclude the pages from a core dump (Linux 3.4).
pub(super) const MADV_DONTDUMP: c_int = 16;

// SAFETY: `int madvise(void *addr, size_t length, int advice)`, as
// `madvise(2)` and `<sys/mman.h>` declare it in glibc and musl; it is in
// the C library itself, so no `#[link]` is needed beyond the C library the
// standard library links. It is declared unsafe to call: some advice
// values change or discard the contents of the range, and the range must
// be memory the caller may advise on; `super` passes only
// `MADV_DONTDUMP`, which changes no content, over memory it owns.
#[expect(unsafe_code, reason = "linux-dontdump")]
unsafe extern "C" {
    pub(super) fn madvise(addr: *mut c_void, length: usize, advice: c_int) -> c_int;
}
