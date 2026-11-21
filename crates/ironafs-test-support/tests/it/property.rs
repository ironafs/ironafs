// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Planted failing properties: each shrinks to a known minimum, and every
//! failure report carries the seed that replays it.
//!
//! Every assertion here holds whatever `IRONAFS_TEST_SEED` is set to; the
//! assertions on a particular seed are unit tests, which bypass it.

use std::cell::Cell;
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
        assert_eq!(failure.message(), "an element is 100 or more");
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
    assert_eq!(first.seed(), second.seed());
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

/// The report `check` panicked with.
fn check_report(check: impl FnOnce()) -> String {
    let payload = panic::catch_unwind(AssertUnwindSafe(check)).unwrap_err();
    payload.downcast_ref::<String>().unwrap().clone()
}

#[test]
fn check_failure_message_carries_the_seed() {
    let line = line!() + 4;
    let report = check_report(|| {
        Property::new("check prints seed")
            .seed(0xabcd)
            .check(Source::u16, |value| assert!(*value < 10));
    });
    let lines: Vec<&str> = report.lines().collect();
    let seed = lines[6]
        .strip_prefix("replay with IRONAFS_TEST_SEED=0x")
        .unwrap();
    assert_eq!(seed.len(), 16, "{report}");
    assert!(
        lines[0].starts_with("property `check prints seed` failed on case ")
            && lines[0].ends_with(&format!(" of 256 (seed 0x{seed})")),
        "{report}"
    );
    assert_eq!(
        lines[1..3],
        [
            "minimal input: 10",
            "failure: assertion failed: *value < 10"
        ],
        "{report}"
    );
    assert!(
        lines[3].starts_with(&format!("panicked at: {}:{line}:", file!())),
        "{report}"
    );
    assert_eq!(lines[4], "minimal choices: [10]", "{report}");
    assert!(lines[5].starts_with("shrunk in "), "{report}");
}

#[test]
fn check_described_reports_the_description() {
    let report = check_report(|| {
        Property::new("check describes").check_described(
            Source::u16,
            |value| assert!(*value < 10),
            |value| format!("{value} units"),
        );
    });
    assert!(report.contains("\nminimal input: 10 units\n"), "{report}");
}

#[test]
fn check_reruns_the_minimal_input_last() {
    let last = Cell::new(None);
    check_report(|| {
        Property::new("check reruns").check(Source::u16, |value| {
            last.set(Some(*value));
            assert!(*value < 10);
        });
    });
    assert_eq!(last.get(), Some(10));
}

#[test]
fn run_choices_pin_a_regression() {
    let failure = Property::new("pinned")
        .run(
            |source| (source.option_of(Source::u8), source.i64_in(-50..=50)),
            |(byte, number)| assert!(byte.is_none() || *number > -3),
        )
        .unwrap_err();
    assert_eq!(*failure.value(), (Some(0), -3));
    let mut pinned = Source::from_choices(failure.choices().to_vec());
    let replayed = (pinned.option_of(Source::u8), pinned.i64_in(-50..=50));
    assert_eq!(replayed, *failure.value());
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

#[test]
fn run_planted_record_array_shrinks_to_one_record() {
    // An array of records followed by another field, as in XDR.
    let generate = |source: &mut Source| {
        (
            source.vec_of(0..=8, |source| (source.u32(), source.u8(), source.bool())),
            source.ascii_string(8),
        )
    };
    for seed in 0..16 {
        let failure = Property::new("planted record array")
            .seed(seed)
            .run(generate, |(records, name)| {
                assert!(records.iter().all(|record| record.1 < 50) || name.is_empty());
            })
            .unwrap_err();
        assert_eq!(
            *failure.value(),
            (vec![(0, 50, false)], "\0".to_owned()),
            "seed {seed}"
        );
        let replayed = generate(&mut Source::from_choices(failure.choices().to_vec()));
        assert_eq!(replayed, *failure.value(), "seed {seed}");
    }
}

#[test]
fn run_planted_bulk_length_shrinks_to_threshold_and_seed_zero() {
    let failure = Property::new("planted bulk length")
        .seed(3)
        .run(
            |source| source.bulk_bytes(0..=64 << 10),
            |bytes| assert!(bytes.len() < 1000),
        )
        .unwrap_err();
    assert_eq!(failure.value().len(), 1000);
    assert_eq!(failure.choices(), [1000]);
}

#[test]
fn run_planted_bulk_content_keeps_a_failing_seed() {
    // Fails only when the payload is long enough and its first byte is odd,
    // so the seed cannot shrink to zero.
    let failure = Property::new("planted bulk content")
        .seed(4)
        .run(
            |source| source.bulk_bytes(0..=64 << 10),
            |bytes| assert!(bytes.len() < 100 || bytes[0] % 2 == 0),
        )
        .unwrap_err();
    assert_eq!(failure.value().len(), 100);
    let seed = failure.choices()[1];
    assert!(seed != 0 && failure.value()[0] % 2 == 1, "{seed}");
    let mut replay = Source::from_choices(failure.choices().to_vec());
    assert_eq!(replay.bulk_bytes(0..=64 << 10), *failure.value());
}
