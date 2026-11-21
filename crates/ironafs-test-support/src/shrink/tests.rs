// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for shrinking choice sequences: known minima and bounded runs.

use super::{Shrunk, shrink};
use crate::source::Recording;
use crate::{Source, SplitMix64};

/// Replays `choices` through `generate` and returns the record and value
/// when `failing` holds for the value, as the property runner does.
fn replay_oracle<T>(
    mut generate: impl FnMut(&mut Source) -> T,
    mut failing: impl FnMut(&T) -> bool,
) -> impl FnMut(&[u64]) -> Option<(Recording, T)> {
    move |choices| {
        let mut source = Source::from_choices(choices.to_vec());
        let value = generate(&mut source);
        failing(&value).then(|| (source.into_recording(), value))
    }
}

/// A time limit that never runs out.
fn never() -> bool {
    false
}

/// Generates fresh inputs from `seed` until one fails, then shrinks it.
fn shrink_first_failure<T>(
    seed: u64,
    budget: u32,
    mut generate: impl FnMut(&mut Source) -> T,
    mut failing: impl FnMut(&T) -> bool,
) -> Shrunk<T> {
    let mut generator = SplitMix64::new(seed);
    for _ in 0..10_000 {
        let mut source = Source::from_seed(generator.next_u64());
        let value = generate(&mut source);
        if failing(&value) {
            let oracle = replay_oracle(&mut generate, &mut failing);
            return shrink(source.into_recording(), value, budget, never, oracle);
        }
    }
    panic!("no failing input found");
}

#[test]
fn shrink_integer_reaches_threshold() {
    let shrunk = shrink_first_failure(1, 10_000, Source::u64, |value| *value >= 1000);
    assert_eq!(shrunk.payload, 1000);
    assert!(!shrunk.budget_exhausted);
}

#[test]
fn shrink_vector_reaches_single_failing_element() {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            10_000,
            |source| source.vec_of(0..=20, Source::u8),
            |values| values.iter().any(|value| *value >= 100),
        );
        assert_eq!(shrunk.payload, vec![100], "seed {seed}");
    }
}

#[test]
fn shrink_vector_followed_by_draw_reaches_minimum() {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            10_000,
            |source| (source.vec_of(0..=8, Source::u8), source.u8()),
            |(values, trailer)| values.iter().any(|value| *value >= 100) && *trailer >= 7,
        );
        assert_eq!(shrunk.payload, (vec![100], 7), "seed {seed}");
    }
}

#[test]
fn shrink_nested_vectors_reach_minimum() {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            10_000,
            |source| {
                (
                    source.vec_of(0..=4, |source| source.vec_of(0..=4, Source::u8)),
                    source.u8(),
                )
            },
            |(outer, trailer)| outer.iter().flatten().any(|value| *value >= 100) && *trailer >= 7,
        );
        assert_eq!(shrunk.payload, (vec![vec![100]], 7), "seed {seed}");
    }
}

#[test]
fn shrink_integer_finds_small_value_below_threshold() {
    // Not monotone: 2 fails, 3 to 999 pass, 1000 and above fail. Binary
    // search alone would stop at 1000.
    let shrunk = shrink_first_failure(1, 10_000, Source::u64, |value| {
        *value == 2 || *value >= 1000
    });
    assert_eq!(shrunk.payload, 2);
}

#[test]
fn shrink_signed_integer_keeps_its_sign() {
    // Zigzag order interleaves signs, so "at most -3" fails on every other
    // choice from 5 up, which binary search over all choices misses.
    for seed in 0..16 {
        let shrunk = shrink_first_failure(seed, 10_000, Source::i64, |value| *value <= -3);
        assert_eq!(shrunk.payload, -3, "seed {seed}");
        let shrunk = shrink_first_failure(
            seed,
            10_000,
            |source| source.i64_in(-1000..=1000),
            |value| *value >= 3,
        );
        assert_eq!(shrunk.payload, 3, "seed {seed}");
    }
}

#[test]
fn shrink_pair_reaches_shortlex_minimum() {
    let shrunk = shrink_first_failure(
        3,
        10_000,
        |source| (source.u64_in(0..=1000), source.u64_in(0..=1000)),
        |(first, second)| first + second > 50,
    );
    // One choice, [51], is shorter than any two-choice sequence such as
    // [0, 51]: a trailing zero choice costs nothing.
    assert_eq!(shrunk.payload, (51, 0));
}

#[test]
fn shrink_always_failing_reaches_empty_choices() {
    let shrunk = shrink_first_failure(4, 10_000, |source| source.bytes(0..=64), |_| true);
    assert_eq!(shrunk.payload, Vec::<u8>::new());
}

#[test]
fn shrink_result_still_fails() {
    let failing = |values: &Vec<u16>| values.iter().map(|v| u32::from(*v)).sum::<u32>() > 3000;
    let shrunk = shrink_first_failure(
        5,
        10_000,
        |source| source.vec_of(0..=10, Source::u16),
        failing,
    );
    assert!(failing(&shrunk.payload), "{:?}", shrunk.payload);
}

/// Fails for an unpredictable half of all choice sequences, so no pass can
/// rely on failures being monotone in the choices.
fn erratic(choices: &[u64]) -> Option<(Recording, Vec<u64>)> {
    let mut hash = SplitMix64::new(choices.len() as u64);
    for choice in choices {
        hash = SplitMix64::new(hash.next_u64() ^ choice);
    }
    (hash.next_u64() & 1 == 1).then(|| (choices.to_vec().into(), choices.to_vec()))
}

fn erratic_start() -> Vec<u64> {
    let mut generator = SplitMix64::new(6);
    loop {
        let choices: Vec<u64> = (0..200).map(|_| generator.next_u64()).collect();
        if erratic(&choices).is_some() {
            return choices;
        }
    }
}

#[test]
fn shrink_erratic_property_stops_at_budget() {
    let start = erratic_start();
    let shrunk = shrink(start.clone().into(), start, 20, never, erratic);
    assert_eq!(shrunk.runs, 20);
    assert!(shrunk.budget_exhausted);
    assert!(erratic(&shrunk.payload).is_some());
}

#[test]
fn shrink_erratic_property_terminates_without_reaching_large_budget() {
    let start = erratic_start();
    let shrunk = shrink(
        start.clone().into(),
        start.clone(),
        u32::MAX,
        never,
        erratic,
    );
    assert!(!shrunk.budget_exhausted);
    assert!(shrunk.runs < 1_000_000, "{}", shrunk.runs);
    assert!(shrunk.payload.len() < start.len());
    assert!(erratic(&shrunk.payload).is_some());
}

#[test]
fn shrink_keeps_original_when_nothing_simpler_fails() {
    let shrunk = shrink(
        vec![0, 7].into(),
        "original",
        1000,
        never,
        |choices: &[u64]| (choices == [0, 7]).then(|| (choices.to_vec().into(), "replayed")),
    );
    assert_eq!(shrunk.payload, "original");
    assert!(!shrunk.budget_exhausted);
}

#[test]
fn shrink_ignores_trailing_zero_choices() {
    // Replaying [7] records [7, 0]: without trailing zeros it is shorter
    // than [0, 7], so the failure moves to the first draw.
    let oracle = replay_oracle(
        |source| (source.u8(), source.u8()),
        |(first, second)| *first == 7 || *second == 7,
    );
    let shrunk = shrink(vec![0, 7].into(), (0, 7), 1000, never, oracle);
    assert_eq!(shrunk.payload, (7, 0));
}

#[test]
fn shrink_lowers_repeated_choices_together() {
    let shrunk = shrink_first_failure(
        8,
        10_000,
        |source| (source.u8(), source.u8()),
        |(first, second)| first == second && *first != 0,
    );
    assert_eq!(shrunk.payload, (1, 1));
}

/// Shrinks a vector of `fields`-choice elements followed by a trailer,
/// failing when some element starts at 100 or more and the trailer is 7, and
/// checks the exact minimum for every seed.
fn assert_struct_elements_reach_minimum(fields: usize) {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            100_000,
            |source| {
                (
                    source.vec_of(0..=8, |source| {
                        (0..fields).map(|_| source.u8()).collect::<Vec<u8>>()
                    }),
                    source.u8(),
                )
            },
            |(elements, trailer)| {
                elements
                    .iter()
                    .any(|element| element.first().is_some_and(|first| *first >= 100))
                    && *trailer == 7
            },
        );
        let mut element = vec![0; fields];
        if let Some(first) = element.first_mut() {
            *first = 100;
        }
        assert_eq!(
            shrunk.payload,
            (vec![element], 7),
            "{fields} fields, seed {seed}"
        );
        assert!(!shrunk.budget_exhausted, "{fields} fields, seed {seed}");
    }
}

#[test]
fn shrink_two_field_elements_followed_by_draw_reach_minimum() {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            100_000,
            |source| {
                (
                    source.vec_of(0..=8, |source| (source.u8(), source.u8())),
                    source.u8(),
                )
            },
            |(elements, trailer)| elements.iter().any(|element| element.0 >= 100) && *trailer == 7,
        );
        assert_eq!(shrunk.payload, (vec![(100, 0)], 7), "seed {seed}");
    }
}

#[test]
fn shrink_four_field_elements_followed_by_draw_reach_minimum() {
    assert_struct_elements_reach_minimum(4);
}

#[test]
fn shrink_ten_field_elements_followed_by_draw_reach_minimum() {
    assert_struct_elements_reach_minimum(10);
}

#[test]
fn shrink_struct_elements_replay_from_minimal_choices() {
    let generate = |source: &mut Source| {
        (
            source.vec_of(0..=8, |source| (source.u8(), source.u8(), source.u8())),
            source.u8(),
        )
    };
    let shrunk = shrink_first_failure(2, 100_000, generate, |(elements, trailer)| {
        elements.iter().any(|element| element.0 >= 100) && *trailer == 7
    });
    let replayed = generate(&mut Source::from_choices(shrunk.choices.clone()));
    assert_eq!(replayed, shrunk.payload);
    assert_eq!(shrunk.choices, vec![1, 100, 0, 0, 0, 7]);
}

#[test]
fn shrink_string_reaches_shortest_lowest_trigger() {
    for seed in 0..16 {
        let shrunk = shrink_first_failure(
            seed,
            100_000,
            |source| (source.string(32), source.u8()),
            |(text, trailer)| text.chars().any(|c| c >= '\u{e9}') && *trailer == 7,
        );
        assert_eq!(shrunk.payload, ("\u{e9}".to_owned(), 7), "seed {seed}");
    }
}

#[test]
fn shrink_out_of_time_stops_with_best_so_far() {
    let mut checks = 0;
    let out_of_time = || {
        checks += 1;
        checks > 30
    };
    let oracle = replay_oracle(Source::u64, |value| *value >= 1000);
    let start = 1 << 40;
    let shrunk = shrink(vec![start].into(), start, 10_000, out_of_time, oracle);
    assert_eq!(shrunk.runs, 30);
    assert!(shrunk.timed_out);
    assert!(!shrunk.budget_exhausted);
    assert!(
        shrunk.payload >= 1000 && shrunk.payload < start,
        "{}",
        shrunk.payload
    );
    assert_eq!(shrunk.choices, vec![shrunk.payload]);
}

#[test]
fn shrink_within_time_reports_no_timeout() {
    let shrunk = shrink_first_failure(1, 10_000, Source::u64, |value| *value >= 1000);
    assert!(!shrunk.timed_out);
}
