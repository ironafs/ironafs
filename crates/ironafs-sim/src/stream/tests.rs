// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streams are fixed by (seed, label, incarnation), separate per label and
//! per incarnation, and value-stable.

use super::{Stream, stream_seed};

#[test]
fn stream_seed_known_answers() {
    // Pinned: a change here moves every recorded seed and golden trace,
    // and is a deliberate change with its own review.
    assert_eq!(stream_seed(0, "schedule", 0), KNOWN[0]);
    assert_eq!(stream_seed(1, "schedule", 0), KNOWN[1]);
    assert_eq!(stream_seed(1, "rng", 0), KNOWN[2]);
    assert_eq!(stream_seed(1, "schedule", 1), KNOWN[3]);
}

/// Computed independently of this crate, from the published `SplitMix64`
/// and FNV-1a definitions, by a short script.
const KNOWN: [u64; 4] = [
    0x4832_5bd7_fcfa_2c40,
    0x48b0_af50_2c86_8bc9,
    0xc0d9_09d9_bf73_817f,
    0x5eac_dfd4_b0fb_d65f,
];

#[test]
fn stream_seed_separates_labels_and_incarnations() {
    let base = stream_seed(7, "schedule", 0);
    assert_ne!(base, stream_seed(7, "network", 0));
    assert_ne!(base, stream_seed(7, "schedule", 1));
    assert_ne!(base, stream_seed(8, "schedule", 0));
    assert_eq!(base, stream_seed(7, "schedule", 0));
}

#[test]
fn index_draws_inside_the_length_and_counts_draws() {
    let mut stream = Stream::new(3, "schedule", 0);
    assert_eq!(stream.index(0), None);
    assert_eq!(stream.draws(), 0);
    for len in 1..50 {
        let index = stream.index(len).unwrap();
        assert!(index < len, "{index} not below {len}");
    }
    assert_eq!(stream.index(1), Some(0));
    assert_eq!(stream.draws(), 50);
}

#[test]
fn same_seed_gives_the_same_choices() {
    let mut first = Stream::new(11, "network", 2);
    let mut second = Stream::new(11, "network", 2);
    let a: Vec<_> = (0..64).map(|_| first.u64_in(0..=1_000)).collect();
    let b: Vec<_> = (0..64).map(|_| second.u64_in(0..=1_000)).collect();
    assert_eq!(a, b);
    assert!(
        a.iter().any(|value| *value != a[0]),
        "the stream never varied"
    );
    let mut empty = Stream::new(11, "network", 2);
    #[expect(
        clippy::reversed_empty_ranges,
        reason = "the empty range is the case under test"
    )]
    let drawn = empty.u64_in(5..=4);
    assert_eq!(drawn, None);
}
