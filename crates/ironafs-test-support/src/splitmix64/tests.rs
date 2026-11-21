// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the `SplitMix64` generator against its reference outputs.

use super::SplitMix64;

#[test]
fn next_u64_from_seed_zero_matches_reference_outputs() {
    // The first outputs of the reference `splitmix.c` seeded with zero, as
    // the module documentation records how they were checked.
    let expected = [
        0xe220_a839_7b1d_cdaf,
        0x6e78_9e6a_a1b9_65f4,
        0x06c4_5d18_8009_454f,
        0xf88b_b8a8_724c_81ec,
        0x1b39_896a_51a8_749b,
    ];
    let mut generator = SplitMix64::new(0);
    let actual: Vec<u64> = (0..expected.len()).map(|_| generator.next_u64()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn next_u64_same_seed_gives_same_sequence() {
    let mut first = SplitMix64::new(0x1234_5678_9abc_def0);
    let mut second = first.clone();
    for _ in 0..64 {
        assert_eq!(first.next_u64(), second.next_u64());
    }
}
