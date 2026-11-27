// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(test)]
#![expect(
    clippy::disallowed_macros,
    reason = "the tests build inputs and expectations outside the codec's ledger"
)]

//! Integration tests for `ironafs-xdr`, through its public API
//! (`docs/specs/xdr.md`, section 10).

mod decode_invariants;
mod entry;
mod golden;
mod reject;
mod roundtrip;
mod seed_replay;
mod stack;
mod support;
mod types;
mod unwind;
