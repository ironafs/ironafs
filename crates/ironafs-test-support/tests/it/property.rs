// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Planted failing properties: each shrinks to a known minimum, and every
//! failure report carries the seed that replays it.

use std::panic::{self, AssertUnwindSafe};

use ironafs_test_support::{Property, Source};

#[test]
fn run_passing_property_returns_ok() {
    let result = Property::new("u8 widens to u16 losslessly").run(Source::u8, |value| {
        assert_eq!(u8::try_from(u16::from(*value)), Ok(*value));
    });
    assert!(result.is_ok());
}

#[test]
fn run_planted_threshold_shrinks_to_threshold() {
    let failure = Property::new("planted threshold")
        .seed(1)
        .run(Source::u64, |value| assert!(*value < 1000))
        .unwrap_err();
    assert_eq!(*failure.value(), 1000);
    assert_eq!(failure.message(), "assertion failed: *value < 1000");
    assert!(!failure.shrink_budget_exhausted());
}

#[test]
fn run_planted_element_shrinks_to_single_element() {
    for seed in 0..8 {
        let failure = Property::new("planted element")
            .seed(seed)
            .run(
                |source| source.vec_of(0..=32, Source::u8),
                |values| {
                    if values.iter().all(|value| *value < 100) {
                        Ok(())
                    } else {
                        Err("an element is 100 or more")
                    }
                },
            )
            .unwrap_err();
        assert_eq!(*failure.value(), vec![100], "seed {seed}");
        assert_eq!(failure.message(), "\"an element is 100 or more\"");
    }
}

#[test]
fn run_planted_decoder_bug_shrinks_to_shortest_trigger() {
    // A planted decoder that misreads any input whose first two bytes are
    // equal and nonzero.
    let decode = |bytes: &[u8]| match bytes {
        [first, second, ..] if first == second && *first != 0 => None,
        _ => Some(bytes.len()),
    };
    let failure = Property::new("planted decoder bug")
        .seed(9)
        .cases(10_000)
        .run(
            |source| source.bytes(0..=16),
            |bytes| assert_eq!(decode(bytes), Some(bytes.len())),
        )
        .unwrap_err();
    assert_eq!(*failure.value(), vec![1, 1]);
}

#[test]
fn run_same_seed_gives_same_counterexample() {
    let run = || {
        Property::new("same seed")
            .seed(0xfeed)
            .run(
                |source| source.vec_of(0..=8, Source::u32),
                |values| {
                    assert!(values.len() < 3);
                },
            )
            .unwrap_err()
    };
    let (first, second) = (run(), run());
    assert_eq!(first.seed(), 0xfeed);
    assert_eq!(
        (first.case(), first.value(), first.shrink_runs()),
        (second.case(), second.value(), second.shrink_runs())
    );
    assert_eq!(*first.value(), vec![0, 0, 0]);
}

#[test]
fn run_stops_shrinking_at_budget() {
    let failure = Property::new("budget")
        .seed(2)
        .shrink_budget(3)
        .run(
            |source| source.bytes(0..=64),
            |bytes| assert!(bytes.len() < 10),
        )
        .unwrap_err();
    assert_eq!(failure.shrink_runs(), 3);
    assert!(failure.shrink_budget_exhausted());
}

#[test]
fn check_failure_message_carries_the_seed() {
    let payload = panic::catch_unwind(|| {
        Property::new("check prints seed")
            .seed(0xabcd)
            .check(Source::u16, |value| assert!(*value < 10));
    })
    .unwrap_err();
    let message = payload.downcast_ref::<String>().unwrap();
    assert!(
        message.starts_with(
            "property `check prints seed` failed on case 1 of 256 (seed 0x000000000000abcd)\n\
             minimal input: 10\n"
        ),
        "{message}"
    );
    assert!(
        message.ends_with("\nreplay with IRONAFS_TEST_SEED=0x000000000000abcd"),
        "{message}"
    );
}

#[test]
fn check_generator_panic_propagates_its_payload() {
    let payload = panic::catch_unwind(AssertUnwindSafe(|| {
        Property::new("generator panics")
            .seed(1)
            .check(|source| *source.pick::<u8>(&[]), |_| ());
    }))
    .unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"pick needs at least one item")
    );
}
