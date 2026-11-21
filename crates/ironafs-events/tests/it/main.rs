// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(test)]

//! Integration tests for `ironafs-events`, through its public API, where
//! the standard library is available.

mod fit;
mod journald;
mod limit;
mod redaction;
mod server_text;
mod support;
mod syslog;
