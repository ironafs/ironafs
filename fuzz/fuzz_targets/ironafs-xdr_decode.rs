// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]

//! Decodes arbitrary bytes as each test type of `docs/specs/xdr.md`,
//! section 10, and asserts the properties of 10.4: no panic; every request
//! within the memory budget, and on success within r(T) times the input's
//! length (P6); on success, the re-encoding is the whole input (P2); an
//! error's offset is within the input; and the same input always gives the
//! same result. Exact error kinds and offsets are the reject corpus's.

use ironafs_xdr as xdr;
use libfuzzer_sys::fuzz_target;

#[path = "../../crates/ironafs-xdr/tests/it/shaded.rs"]
#[expect(dead_code, reason = "only the integration properties use Shade::ALL")]
mod shaded;

/// The test types, built from the integration tests' own file: it uses
/// only the crate's public API, so both build the same types.
#[path = "../../crates/ironafs-xdr/tests/it/types.rs"]
#[expect(
    dead_code,
    reason = "the target decodes each test type; the caps and rows it does not name stay \
              for the integration tests"
)]
mod types;

#[path = "../../crates/ironafs-xdr/tests/it/decode_invariants.rs"]
mod decode_invariants;

fuzz_target!(|input: &[u8]| {
    decode_invariants::check_input(input);
});
