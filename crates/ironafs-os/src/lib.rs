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
//! declarations are written and proved for those targets alone. The guard
//! below names every property that tells them apart from the compiler's
//! other targets, and `cargo xtask check-os` evaluates it against every
//! target the pinned compiler knows: it must accept exactly those seven.

#[cfg(not(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(
        all(
            target_os = "linux",
            target_vendor = "unknown",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            any(target_env = "gnu", target_env = "musl"),
            target_abi = "",
        ),
        all(
            target_os = "macos",
            target_vendor = "apple",
            target_arch = "aarch64",
            target_abi = "",
        ),
        all(
            target_os = "windows",
            target_vendor = "pc",
            target_env = "msvc",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            target_abi = "",
        ),
    ),
)))]
compile_error!(
    "ironafs-os builds only for the targets of ADR 0005, point 5.3: x86_64 and aarch64 Linux \
     (glibc and musl), aarch64 macOS, and x86_64 and aarch64 Windows (MSVC)"
);

mod clock;
mod dump;
mod fdpass;
mod peer;
mod random;
mod space;
mod udp6;

#[cfg(target_os = "linux")]
mod linux_clock;
#[cfg(target_os = "linux")]
mod linux_dontdump;
#[cfg(target_os = "linux")]
mod linux_fdpass;
#[cfg(target_os = "linux")]
mod linux_peercred;
#[cfg(target_os = "linux")]
mod linux_random;
#[cfg(target_os = "linux")]
mod linux_statvfs;
#[cfg(target_os = "linux")]
mod linux_udp6;
mod mem_wipe;
#[cfg(target_os = "linux")]
mod unix_nodump;

pub use clock::{ClockError, OsClock};
pub use dump::DumpError;
pub use fdpass::FdPassError;
#[cfg(target_os = "linux")]
pub use linux_clock::read_clock;
#[cfg(target_os = "linux")]
pub use linux_dontdump::SecretBuffer;
#[cfg(target_os = "linux")]
pub use linux_fdpass::{receive_fd, send_fd};
#[cfg(target_os = "linux")]
pub use linux_peercred::peer_credentials;
#[cfg(target_os = "linux")]
pub use linux_random::fill_random;
#[cfg(target_os = "linux")]
pub use linux_statvfs::available_space;
#[cfg(target_os = "linux")]
pub use linux_udp6::udp6_socket;
pub use mem_wipe::wipe;
pub use peer::{PeerCredentials, PeerError};
pub use random::RandomError;
pub use space::SpaceError;
pub use udp6::Udp6Error;
#[cfg(target_os = "linux")]
pub use unix_nodump::disable_dumps;
