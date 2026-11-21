// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for drawing, recording and replaying choices.

use std::panic;

use super::Source;
use crate::SplitMix64;

fn draw_mixture(source: &mut Source) -> (u64, bool, u8, i64, Vec<u8>, Vec<u16>) {
    (
        source.u64_in(10..=20),
        source.bool(),
        source.u8(),
        source.i64(),
        source.bytes(0..=8),
        source.vec_of(1..=4, Source::u16),
    )
}

#[test]
fn replay_of_record_reproduces_values() {
    for seed in 0..32 {
        let mut fresh = Source::from_seed(seed);
        let original = draw_mixture(&mut fresh);
        let record = fresh.into_record();
        let mut replay = Source::from_choices(record.clone());
        assert_eq!(draw_mixture(&mut replay), original, "seed {seed}");
        assert_eq!(replay.into_record(), record, "seed {seed}");
    }
}

#[test]
fn replay_past_end_draws_zero_choices() {
    let mut source = Source::from_choices(vec![3]);
    assert_eq!(source.u64_in(0..=9), 3);
    assert_eq!(source.u64_in(5..=9), 5);
    assert!(!source.bool());
    assert_eq!(source.i64(), 0);
    assert_eq!(source.bytes(2..=5), vec![0, 0]);
    assert_eq!(source.into_record(), vec![3, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn replay_reduces_out_of_range_choice_into_range() {
    let mut source = Source::from_choices(vec![25, 7]);
    assert_eq!(source.u64_in(100..=109), 105);
    assert_eq!(source.u8(), 7);
    assert_eq!(source.into_record(), vec![5, 7]);
}

#[test]
fn fresh_draws_stay_in_their_ranges() {
    let mut source = Source::from_seed(7);
    for _ in 0..1000 {
        let value = source.u64_in(3..=5);
        assert!((3..=5).contains(&value), "{value}");
        let length = source.usize_in(2..=3);
        assert!((2..=3).contains(&length), "{length}");
        let bytes = source.bytes(1..=2);
        assert!((1..=2).contains(&bytes.len()), "{bytes:?}");
    }
}

#[test]
fn fresh_draws_cover_a_small_range() {
    let mut source = Source::from_seed(11);
    let mut seen = [false; 4];
    for _ in 0..200 {
        seen[usize::try_from(source.u64_in(0..=3)).unwrap()] = true;
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn fresh_draw_is_scaled_generator_output_or_boundary() {
    let mut generator = SplitMix64::new(5);
    let mut source = Source::from_seed(5);
    for _ in 0..1000 {
        let output = generator.next_u64();
        let value = source.u64();
        assert!(
            value == output || [0, 1, u64::MAX - 1, u64::MAX].contains(&value),
            "{value:#x} for output {output:#x}"
        );
    }
}

#[test]
fn fresh_draws_hit_boundaries() {
    let mut source = Source::from_seed(13);
    let values: Vec<u64> = (0..2000).map(|_| source.u64()).collect();
    for boundary in [0, 1, u64::MAX - 1, u64::MAX] {
        assert!(values.contains(&boundary), "{boundary:#x}");
    }
    let ranged: Vec<u64> = (0..2000).map(|_| source.u64_in(10..=1_000_000)).collect();
    for boundary in [10, 11, 999_999, 1_000_000] {
        assert!(ranged.contains(&boundary), "{boundary}");
    }
    let signed: Vec<i64> = (0..2000).map(|_| source.i64()).collect();
    for boundary in [0, -1, i64::MIN, i64::MAX] {
        assert!(signed.contains(&boundary), "{boundary}");
    }
}

#[test]
fn fresh_draws_are_mostly_not_boundaries() {
    let mut source = Source::from_seed(17);
    let boundaries = (0..1600)
        .map(|_| source.u64())
        .filter(|value| [0, 1, u64::MAX - 1, u64::MAX].contains(value))
        .count();
    // One draw in 16 is a boundary: about 100 of 1600.
    assert!((50..=150).contains(&boundaries), "{boundaries}");
}

#[test]
fn from_seed_draws_as_a_fresh_source() {
    let mut seeded = Source::from_seed(3);
    let mut fresh = Source::from_seed(3);
    assert_eq!(draw_mixture(&mut seeded), draw_mixture(&mut fresh));
    assert_eq!(seeded.choices(), fresh.into_record().as_slice());
}

#[test]
fn from_choices_replays_and_records() {
    let mut source = Source::from_choices(vec![5, 1, 9]);
    assert_eq!((source.u8(), source.option_of(Source::u8)), (5, Some(9)));
    assert_eq!(source.choices(), [5, 1, 9]);
}

#[test]
fn signed_draws_shrink_toward_zero() {
    let mut source = Source::from_choices(vec![0, 1, 2, 3, 4]);
    assert_eq!(
        [source.i64(), source.i64(), source.i64(), source.i64()],
        [0, -1, 1, -2]
    );
    assert_eq!(source.i32(), 2);
}

#[test]
fn i64_in_shrinks_toward_zero_inside_range() {
    let mut source = Source::from_choices((0..=7).collect());
    let values: Vec<i64> = (0..=7).map(|_| source.i64_in(-2..=5)).collect();
    assert_eq!(values, [0, -1, 1, -2, 2, 3, 4, 5]);
    let mut source = Source::from_choices((0..=6).collect());
    let values: Vec<i64> = (0..=6).map(|_| source.i64_in(-5..=1)).collect();
    assert_eq!(values, [0, -1, 1, -2, -3, -4, -5]);
}

#[test]
fn i64_in_shrinks_toward_end_nearer_zero() {
    let mut source = Source::from_choices(vec![2, 2]);
    assert_eq!((source.i64_in(3..=9), source.i64_in(-9..=-3)), (5, -5));
}

#[test]
fn i64_in_full_range_reaches_the_extremes() {
    let mut source = Source::from_choices(vec![u64::MAX, u64::MAX - 1]);
    assert_eq!(
        (
            source.i64_in(i64::MIN..=i64::MAX),
            source.i64_in(i64::MIN..=i64::MAX)
        ),
        (i64::MIN, i64::MAX)
    );
}

#[test]
fn option_of_shrinks_toward_none() {
    let mut source = Source::from_choices(vec![0, 1, 7]);
    assert_eq!(
        (source.option_of(Source::u8), source.option_of(Source::u8)),
        (None, Some(7))
    );
}

#[test]
fn weighted_replays_index_choice() {
    let mut source = Source::from_choices(vec![2, 0, 4]);
    let weights = [5, 1, 1];
    assert_eq!(
        [
            source.weighted(&weights),
            source.weighted(&weights),
            source.weighted(&weights)
        ],
        [2, 0, 1]
    );
}

#[test]
fn weighted_fresh_follows_weights() {
    let mut source = Source::from_seed(19);
    let mut counts = [0_u32; 3];
    for _ in 0..1000 {
        counts[source.weighted(&[1, 8, 1])] += 1;
    }
    // Expected 100, 800, 100.
    assert!((700..=900).contains(&counts[1]), "{counts:?}");
    assert!(counts[0] > 50 && counts[2] > 50, "{counts:?}");
}

#[test]
fn signed_draws_reach_the_extremes() {
    let mut source = Source::from_choices(vec![u64::MAX, u64::MAX - 1, u64::from(u32::MAX)]);
    assert_eq!(source.i64(), i64::MIN);
    assert_eq!(source.i64(), i64::MAX);
    assert_eq!(source.i32(), i32::MIN);
}

#[test]
fn vec_of_draws_continuation_before_each_element_past_minimum() {
    let mut source = Source::from_choices(vec![9, 8, 1, 7, 1, 6, 0, 5]);
    assert_eq!(
        (source.vec_of(2..=5, Source::u8), source.u8()),
        (vec![9, 8, 7, 6], 5)
    );
}

#[test]
fn vec_of_reads_and_ignores_continuation_at_maximum() {
    let mut source = Source::from_choices(vec![1, 5, 1, 6, 1, 3]);
    assert_eq!(
        (source.vec_of(0..=2, Source::u8), source.u8()),
        (vec![5, 6], 3)
    );
}

#[test]
fn fresh_vec_lengths_cover_their_range() {
    let mut seen = [false; 4];
    for seed in 0..200 {
        let mut source = Source::from_seed(seed);
        seen[source.vec_of(0..=3, Source::u8).len()] = true;
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn pick_selects_by_choice() {
    let items = ["a", "b", "c"];
    let mut source = Source::from_choices(vec![2, 0, 4]);
    assert_eq!(
        [
            *source.pick(&items),
            *source.pick(&items),
            *source.pick(&items)
        ],
        ["c", "a", "b"]
    );
}

#[test]
fn u64_in_empty_range_panics_with_message() {
    let (start, end) = (5, 4);
    let result = panic::catch_unwind(|| Source::from_choices(Vec::new()).u64_in(start..=end));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"u64_in needs a non-empty range")
    );
}

#[test]
fn pick_from_empty_slice_panics_with_message() {
    let result = panic::catch_unwind(|| *Source::from_choices(Vec::new()).pick::<u8>(&[]));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"pick needs at least one item")
    );
}

#[test]
fn i64_in_empty_range_panics_with_message() {
    let (start, end) = (5, 4);
    let result = panic::catch_unwind(|| Source::from_choices(Vec::new()).i64_in(start..=end));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"i64_in needs a non-empty range")
    );
}

#[test]
fn weighted_rejects_empty_and_zero_weights() {
    for weights in [&[][..], &[1, 0][..]] {
        let result = panic::catch_unwind(|| Source::from_choices(Vec::new()).weighted(weights));
        let payload = result.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"weighted needs at least one weight, and no zero weight"),
            "{weights:?}"
        );
    }
}

#[test]
fn vec_of_empty_length_range_panics_with_message() {
    let (start, end) = (5, 4);
    let result =
        panic::catch_unwind(|| Source::from_choices(Vec::new()).vec_of(start..=end, Source::u8));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"vec_of needs a non-empty length range")
    );
}

#[test]
fn vec_of_records_a_span_for_each_element_past_minimum() {
    let mut source = Source::from_choices(vec![4, 1, 1, 5, 6, 1, 7, 8, 0, 9]);
    let values = source.vec_of(1..=4, |source| (source.u8(), source.u8()));
    assert_eq!(values, vec![(4, 1), (5, 6), (7, 8)]);
    assert_eq!(source.u8(), 9);
    let recording = source.into_recording();
    assert_eq!(recording.choices, vec![4, 1, 1, 5, 6, 1, 7, 8, 0, 9]);
    assert_eq!(recording.spans, vec![2..5, 5..8]);
}

#[test]
fn nested_vec_of_records_outer_spans_before_inner_ones() {
    let mut source = Source::from_choices(vec![1, 1, 3, 1, 4, 0, 0]);
    let values = source.vec_of(0..=2, |source| source.vec_of(0..=2, Source::u8));
    assert_eq!(values, vec![vec![3, 4]]);
    assert_eq!(source.into_recording().spans, vec![0..6, 1..3, 3..5]);
}

#[test]
fn string_replays_code_points_without_surrogates() {
    // Choice 0xd800 is the first scalar value after the surrogates.
    let mut source = Source::from_choices(vec![1, 0xe9, 1, 0xd800, 1, 0x41, 0]);
    assert_eq!(source.string(16), "\u{e9}\u{e000}A");
    let recording = source.into_recording();
    assert_eq!(recording.spans, vec![0..2, 2..4, 4..6]);
}

#[test]
fn string_stops_at_its_byte_cap() {
    // Two bytes left after "é": a three-byte choice is reduced to fit.
    let mut source = Source::from_choices(vec![1, 0xe9, 1, 0xd800, 1, 0x41]);
    let text = source.string(3);
    assert!(text.len() <= 3, "{text:?}");
    assert_eq!(text.chars().next(), Some('\u{e9}'));
    assert_eq!(Source::from_choices(vec![1, 0x41, 1]).string(1), "A");
}

#[test]
fn ascii_string_reduces_choices_to_ascii() {
    let mut source = Source::from_choices(vec![1, 0xe9, 0]);
    assert_eq!(source.ascii_string(8), "i");
}

#[test]
fn fresh_strings_stay_within_cap_and_cover_ranges() {
    let mut source = Source::from_seed(11);
    let (mut ascii, mut wider, mut astral, mut lengths) = (false, false, false, [false; 13]);
    for _ in 0..2000 {
        let text = source.string(12);
        assert!(text.len() <= 12, "{text:?}");
        if let Some(seen) = lengths.get_mut(text.len()) {
            *seen = true;
        }
        for character in text.chars() {
            match character.len_utf8() {
                1 => ascii = true,
                4 => astral = true,
                _ => wider = true,
            }
        }
        let text = source.ascii_string(12);
        assert!(text.len() <= 12 && text.is_ascii(), "{text:?}");
    }
    assert!(ascii && wider && astral);
    assert!(lengths.iter().all(|seen| *seen), "{lengths:?}");
}
