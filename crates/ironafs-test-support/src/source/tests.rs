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
        let mut fresh = Source::fresh(SplitMix64::new(seed));
        let original = draw_mixture(&mut fresh);
        let record = fresh.into_record();
        let mut replay = Source::replay(record.clone());
        assert_eq!(draw_mixture(&mut replay), original, "seed {seed}");
        assert_eq!(replay.into_record(), record, "seed {seed}");
    }
}

#[test]
fn replay_past_end_draws_zero_choices() {
    let mut source = Source::replay(vec![3]);
    assert_eq!(source.u64_in(0..=9), 3);
    assert_eq!(source.u64_in(5..=9), 5);
    assert!(!source.bool());
    assert_eq!(source.i64(), 0);
    assert_eq!(source.bytes(2..=5), vec![0, 0]);
    assert_eq!(source.into_record(), vec![3, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn replay_reduces_out_of_range_choice_into_range() {
    let mut source = Source::replay(vec![25, 7]);
    assert_eq!(source.u64_in(100..=109), 105);
    assert_eq!(source.u8(), 7);
    assert_eq!(source.into_record(), vec![5, 7]);
}

#[test]
fn fresh_draws_stay_in_their_ranges() {
    let mut source = Source::fresh(SplitMix64::new(7));
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
    let mut source = Source::fresh(SplitMix64::new(11));
    let mut seen = [false; 4];
    for _ in 0..200 {
        seen[usize::try_from(source.u64_in(0..=3)).unwrap()] = true;
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn full_range_draw_is_the_raw_generator_output() {
    let mut generator = SplitMix64::new(5);
    let mut source = Source::fresh(generator.clone());
    assert_eq!(source.u64(), generator.next_u64());
    assert_eq!(source.u64_in(0..=u64::MAX), generator.next_u64());
}

#[test]
fn signed_draws_shrink_toward_zero() {
    let mut source = Source::replay(vec![0, 1, 2, 3, 4]);
    assert_eq!(
        [source.i64(), source.i64(), source.i64(), source.i64()],
        [0, -1, 1, -2]
    );
    assert_eq!(source.i32(), 2);
}

#[test]
fn signed_draws_reach_the_extremes() {
    let mut source = Source::replay(vec![u64::MAX, u64::MAX - 1, u64::from(u32::MAX)]);
    assert_eq!(source.i64(), i64::MIN);
    assert_eq!(source.i64(), i64::MAX);
    assert_eq!(source.i32(), i32::MIN);
}

#[test]
fn vec_of_draws_length_then_elements() {
    let mut source = Source::replay(vec![2, 9, 8, 7]);
    assert_eq!(source.vec_of(1..=5, Source::u8), vec![9, 8, 7]);
}

#[test]
fn pick_selects_by_choice() {
    let items = ["a", "b", "c"];
    let mut source = Source::replay(vec![2, 0, 4]);
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
    let result = panic::catch_unwind(|| Source::replay(Vec::new()).u64_in(start..=end));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"u64_in needs a non-empty range")
    );
}

#[test]
fn pick_from_empty_slice_panics_with_message() {
    let result = panic::catch_unwind(|| *Source::replay(Vec::new()).pick::<u8>(&[]));
    let payload = result.unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"pick needs at least one item")
    );
}
