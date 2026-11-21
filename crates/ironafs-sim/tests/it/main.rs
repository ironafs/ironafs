// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(test)]

//! Integration tests for `ironafs-sim`: every evidence run is a child
//! process under the supervisor.

mod golden;
mod replay;
mod support;
mod witness;
