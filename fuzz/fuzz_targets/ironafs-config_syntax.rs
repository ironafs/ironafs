// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]

//! Parses arbitrary bytes as a configuration file and asserts the
//! properties of `docs/specs/config-syntax.md`, section 10.4: the parser
//! returns; an error's position lies within the input; a file over the
//! file cap is refused as too large; an accepted input respects every cap
//! of section 5.2, on its bytes and on its document; and the same input
//! always gives the same result. Exact error kinds and
//! positions are the reject corpus's.

use ironafs_config::parse;
use libfuzzer_sys::fuzz_target;

/// The checks, from the property tests' own file: it uses only the
/// crate's public API and the standard library, so both check the same
/// invariants.
#[path = "../../crates/ironafs-config/tests/it/invariants.rs"]
mod invariants;

fuzz_target!(|input: &[u8]| {
    let first = parse(input);
    let checked = invariants::check(input, &first);
    assert!(checked.is_ok(), "{checked:?}");
    assert!(parse(input) == first, "the same input gave another result");
});
