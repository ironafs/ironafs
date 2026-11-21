// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(test)]

//! Integration tests for `ironafs-net`, through its public API.

#[cfg(target_os = "linux")]
mod monitor;
