// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's network drivers (ADR 0004, C4, point 3). On Linux: the
//! network-change monitor, which moves the network generation
//! (docs/specs/network-generation.md).

#[cfg(target_os = "linux")]
mod monitor;

#[cfg(target_os = "linux")]
pub use monitor::{MonitorError, MonitorHandle, spawn};
