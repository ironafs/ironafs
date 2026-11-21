// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-dontdump`, named as the C headers
//! name them, written from `madvise(2)` and `<sys/mman.h>`, and proved on
//! each Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6).

use core::ffi::c_int;

/// `MADV_DONTDUMP`: exclude the pages from a core dump (Linux 3.4).
pub(super) const MADV_DONTDUMP: c_int = 16;

// The foreign function, or under Miri, which cannot call it, the in-crate
// stand-in of ADR 0005, point 6.5, which the same call reaches, so the
// wrapper's pointer and length run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::madvise;
#[cfg(miri)]
pub(super) use stand_in::madvise;
