// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's operating-system bindings: the few calls that have no safe
//! route, each behind a safe interface (ADR 0005; ADR 0004, C5, point 5).
//!
//! This is the one crate of the workspace that may hold `unsafe`. Every
//! foreign declaration, `unsafe` block and `unsafe impl` in it is a *site*
//! of ADR 0005's closed list, named by its `#[expect(unsafe_code, reason =
//! "<site id>")]` and argued in its `// SAFETY:` comment. `sites.json`
//! beside the manifest is the machine-readable copy of that list, and
//! `cargo xtask check-os` keeps the code equal to it, target by target.
//!
//! The public API is safe Rust: no `unsafe fn`, raw pointer or foreign type
//! leaves the crate, and no caller can cause undefined behaviour through it,
//! whatever it passes, from whatever thread (ADR 0005, point 1.2). No
//! function of the crate may be called from a signal handler; the crate
//! installs none and never forks. It holds mechanism, not policy (point
//! 1.3): what a failed clock read or an unavailable random source means is
//! its callers' to decide.
//!
//! The crate builds for the targets of ADR 0005, point 5.3, only: their
//! declarations are written and proved for those targets alone.

#[cfg(not(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64"),
        any(target_env = "gnu", target_env = "musl"),
    ),
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "windows",
        target_env = "msvc",
        any(target_arch = "x86_64", target_arch = "aarch64"),
    ),
)))]
compile_error!(
    "ironafs-os builds only for the targets of ADR 0005, point 5.3: x86_64 and aarch64 Linux \
     (glibc and musl), aarch64 macOS, and x86_64 and aarch64 Windows (MSVC)"
);

mod clock;
mod random;

#[cfg(target_os = "linux")]
mod linux_clock;
#[cfg(target_os = "linux")]
mod linux_random;
mod mem_wipe;

pub use clock::{ClockError, OsClock};
#[cfg(target_os = "linux")]
pub use linux_clock::read_clock;
#[cfg(target_os = "linux")]
pub use linux_random::fill_random;
pub use mem_wipe::wipe;
pub use random::RandomError;
