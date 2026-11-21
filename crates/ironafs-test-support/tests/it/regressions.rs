// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Failures kept as regression tests: each replays the minimal choices a
//! failing property reported, through that property's generator and test,
//! on every run. The failure report prints these tests ready to paste.

use ironafs_test_support::Property;

use crate::property::{pair_within_ranges, ranged_pair};

/// A shrunk sequence can hold a choice past a smaller draw's range; the
/// replay reduces it into the range instead of drawing outside it.
#[test]
fn ranged_draws_stay_in_their_ranges_holds_on_pinned_input() {
    Property::new("ranged draws stay in their ranges").regression(
        &[1000, 1000],
        ranged_pair,
        pair_within_ranges,
    );
}
