// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The FUSE mount tests of ADR 0005, sheet 15.

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;
