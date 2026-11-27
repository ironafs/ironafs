// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(test)]

//! Integration tests for `ironafs-config`, through its public API, where
//! the standard library is available: the property tests of the parser
//! and the cell name.

mod cell_name;
mod invariants;
mod seed_replay;
mod syntax;
