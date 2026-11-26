// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The declarations of site `linux-mount`, from `mount(2)` and
//! `<sys/mount.h>`. `cargo xtask check-abi` checks the constants and the
//! foreign prototype against the target's C headers.

use core::ffi::c_ulong;

/// Suppress set-user-ID, set-group-ID and file capabilities on execution.
pub(super) const MS_NOSUID: c_ulong = 2;
/// Disallow access to device nodes on the mount.
pub(super) const MS_NODEV: c_ulong = 4;

#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
pub(super) mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{last_error, mount};
#[cfg(miri)]
pub(super) use stand_in::{last_error, mount};
