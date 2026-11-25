// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SYS_getrandom`, the system-call number of `getrandom`, for the
//! architecture being built: 318 on `x86_64` (`syscall_64.tbl`) and 278 on
//! `aarch64` (the generic table, `asm-generic/unistd.h`). Each value is in
//! its own file, which only that architecture's build reads, so the ABI
//! check proves each against `<sys/syscall.h>` on its own target.

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
pub(super) use aarch64::SYS_getrandom;
#[cfg(target_arch = "x86_64")]
pub(super) use x86_64::SYS_getrandom;
