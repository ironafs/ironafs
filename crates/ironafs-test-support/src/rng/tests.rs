// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seeded random source: a value-stable stream, independent
//! of how fills split it, and failure on demand.

use ironafs_time::{Rng, RngError};

use super::SeededRng;

fn bytes(source: &SeededRng, count: usize) -> Vec<u8> {
    let mut bytes = vec![0; count];
    source.fill(&mut bytes).unwrap();
    bytes
}

#[test]
fn fill_from_seed_zero_gives_the_generator_outputs_little_endian() {
    // The first two SplitMix64 outputs for seed zero.
    let mut expected = 0xe220_a839_7b1d_cdaf_u64.to_le_bytes().to_vec();
    expected.extend(0x6e78_9e6a_a1b9_65f4_u64.to_le_bytes());
    assert_eq!(bytes(&SeededRng::new(0), 16), expected);
}

#[test]
fn fill_stream_does_not_depend_on_how_it_is_split() {
    let whole = bytes(&SeededRng::new(42), 29);
    let split = SeededRng::new(42);
    let mut joined = Vec::new();
    for count in [3, 0, 8, 1, 17] {
        joined.extend(bytes(&split, count));
    }
    assert_eq!(joined, whole);
}

#[test]
fn different_seeds_give_different_streams() {
    assert_ne!(bytes(&SeededRng::new(1), 32), bytes(&SeededRng::new(2), 32));
}

#[test]
fn failing_fill_returns_the_error_and_draws_nothing() {
    let expected = bytes(&SeededRng::new(5), 8);
    let source = SeededRng::new(5);
    source.set_failing(true);
    let mut buffer = [0_u8; 8];
    assert_eq!(source.fill(&mut buffer), Err(RngError::new()));
    source.set_failing(false);
    assert_eq!(bytes(&source, 8), expected);
}
