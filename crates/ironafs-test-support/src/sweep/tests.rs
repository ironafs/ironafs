// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the exhaustive sweep: every input once, in choice order,
//! within the printed bound.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};

use super::{Exhaustive, SweepFailure};
use crate::Source;

/// Sweeps `generate` with a test that always passes, returning every
/// input in the order the sweep gave them and the notice it printed.
fn sweep_all<T: Clone>(
    bound: u64,
    mut generate: impl FnMut(&mut Source) -> T,
) -> (Result<u64, SweepFailure<T>>, Vec<T>, Vec<String>) {
    let mut seen = Vec::new();
    let mut notices = Vec::new();
    let result = Exhaustive::new("sweep under test", bound).run_with(
        &mut generate,
        &mut |value: &T| seen.push(value.clone()),
        &mut |notice: &str| notices.push(notice.to_owned()),
    );
    (result, seen, notices)
}

#[track_caller]
fn assert_distinct<T: Ord + Debug>(values: &[T]) {
    let distinct: BTreeSet<&T> = values.iter().collect();
    assert_eq!(distinct.len(), values.len(), "{values:?}");
}

#[test]
fn run_visits_every_u8_once_in_order() {
    let (result, seen, _) = sweep_all(256, Source::u8);
    assert_eq!(result.ok(), Some(256));
    assert_eq!(seen, (0..=u8::MAX).collect::<Vec<_>>());
}

#[test]
fn run_success_prints_count_and_bound() {
    let (_, _, notices) = sweep_all(1000, Source::bool);
    assert_eq!(
        notices,
        vec!["exhaustive sweep `sweep under test`: all 2 inputs passed (bound 1000)"]
    );
}

#[test]
fn run_visits_product_in_choice_order() {
    let (result, seen, _) = sweep_all(10, |source| (source.bool(), source.u64_in(5..=6)));
    assert_eq!(result.ok(), Some(4));
    assert_eq!(seen, vec![(false, 5), (false, 6), (true, 5), (true, 6)]);
}

#[test]
fn run_visits_dependent_draws_once_each() {
    // The second draw's range depends on the first: 1 + 2 + 3 + 4 inputs.
    let (result, seen, _) = sweep_all(100, |source| {
        let top = source.u64_in(0..=3);
        (top, source.u64_in(0..=top))
    });
    assert_eq!(result.ok(), Some(10));
    assert_distinct(&seen);
    assert!(seen.iter().all(|(top, value)| value <= top));
}

#[test]
fn run_visits_each_vector_once_including_full_length() {
    // Lengths 0, 1 and 2 over booleans: 1 + 2 + 4 vectors.
    let (result, seen, _) = sweep_all(100, |source| source.vec_of(0..=2, Source::bool));
    assert_eq!(result.ok(), Some(7));
    assert_distinct(&seen);
}

#[test]
fn run_visits_each_short_string_once() {
    // The empty string and the 128 one-character ASCII strings.
    let (result, seen, _) = sweep_all(1000, |source| source.ascii_string(1));
    assert_eq!(result.ok(), Some(129));
    assert_distinct(&seen);
}

#[test]
fn run_visits_each_picked_item_and_option() {
    let (result, seen, _) = sweep_all(100, |source| {
        source.option_of(|source| *source.pick(&["a", "b", "c"]))
    });
    assert_eq!(result.ok(), Some(4));
    assert_eq!(seen, vec![None, Some("a"), Some("b"), Some("c")]);
}

#[test]
fn run_domain_over_bound_stops_at_bound() {
    let (result, seen, notices) = sweep_all(1000, Source::u16);
    assert!(matches!(
        result,
        Err(SweepFailure::TooLarge { bound: 1000, .. })
    ));
    assert_eq!(seen.len(), 1000);
    assert_eq!(notices, Vec::<String>::new());
}

#[test]
fn run_domain_equal_to_bound_passes() {
    let (result, _, _) = sweep_all(4, |source| source.u64_in(0..=3));
    assert_eq!(result.ok(), Some(4));
}

#[test]
fn run_failing_input_reports_first_in_choice_order() {
    let result = Exhaustive::new("u8 below 200", 256).run_with(
        &mut Source::u8,
        &mut |value: &u8| assert!(*value < 200, "{value} is too large"),
        &mut |_: &str| {},
    );
    let Err(SweepFailure::Failed(failed)) = result else {
        panic!("expected a failed input, got {result:?}");
    };
    assert_eq!(*failed.value(), 200);
    assert_eq!(failed.index(), 200);
    assert_eq!(failed.choices(), &[200]);
    assert_eq!(failed.message(), "200 is too large");
    assert!(failed.location().unwrap().contains("tests.rs"));
}

#[test]
fn run_failing_input_by_error_has_no_location() {
    let result = Exhaustive::new("bool is false", 2).run_with(
        &mut Source::bool,
        &mut |value: &bool| if *value { Err("true") } else { Ok(()) },
        &mut |_: &str| {},
    );
    let Err(SweepFailure::Failed(failed)) = result else {
        panic!("expected a failed input, got {result:?}");
    };
    assert_eq!(failed.message(), "true");
    assert_eq!(failed.location(), None);
}

#[test]
fn failed_choices_drop_trailing_zeros() {
    let result = Exhaustive::new("pair", 100).run_with(
        &mut |source: &mut Source| (source.u64_in(0..=3), source.u64_in(0..=3)),
        &mut |pair: &(u64, u64)| assert_ne!(*pair, (2, 0)),
        &mut |_: &str| {},
    );
    let Err(SweepFailure::Failed(failed)) = result else {
        panic!("expected a failed input, got {result:?}");
    };
    assert_eq!(failed.choices(), &[2]);
    assert_eq!(
        Source::from_choices(failed.choices().to_vec()).u64_in(0..=3),
        2
    );
}

#[test]
fn failure_report_names_bound_and_prints_regression() {
    let result = Exhaustive::new("u8 below 200", 256).run_with(
        &mut Source::u8,
        &mut |value: &u8| {
            if *value < 200 {
                Ok(())
            } else {
                Err("too large")
            }
        },
        &mut |_: &str| {},
    );
    let report = result.unwrap_err().to_string();
    assert_eq!(
        report,
        "exhaustive sweep `u8 below 200` failed on input 201 of at most 256\n\
         input: 200\n\
         failure: too large\n\
         choices: [200]\n\
         pin it with a regression test in tests/it/regressions.rs, \
         calling this property's generator and test:\n\
         #[test]\n\
         fn u8_below_200_holds_on_pinned_input() {\n    \
         Property::new(\"u8 below 200\").regression(&[200], generate, test);\n\
         }"
    );
}

#[test]
fn too_large_report_names_bound() {
    let (result, _, _) = sweep_all(10, Source::u8);
    assert_eq!(
        result.unwrap_err().to_string(),
        "exhaustive sweep `sweep under test` has more than 10 inputs; \
         raise its bound, or test the domain as a Property"
    );
}

#[test]
fn new_zero_bound_panics() {
    let panic = panic::catch_unwind(|| Exhaustive::new("empty", 0)).unwrap_err();
    assert_eq!(
        *panic.downcast_ref::<&str>().unwrap(),
        "an exhaustive sweep needs a bound of at least one input"
    );
}

#[test]
fn check_failure_panics_with_report() {
    let panic = panic::catch_unwind(AssertUnwindSafe(|| {
        Exhaustive::new("bool is false", 2).check(Source::bool, |value| {
            if *value { Err("true") } else { Ok(()) }
        });
    }))
    .unwrap_err();
    let message = panic.downcast_ref::<String>().unwrap();
    assert!(
        message.starts_with("exhaustive sweep `bool is false` failed on input 2 of at most 2\n"),
        "{message}"
    );
}

#[test]
fn rerun_failing_names_the_sweep_and_its_failing_input() {
    let mut notices = Vec::new();
    let mut runs = 0;
    Exhaustive::new("bool is false", 2).rerun_failing(
        &[1],
        &mut Source::bool,
        &mut |value: &bool| {
            runs += 1;
            assert!(!*value, "true");
        },
        &mut |notice: &str| notices.push(notice.to_owned()),
    );
    assert_eq!(runs, 1);
    assert_eq!(
        notices,
        vec![
            "exhaustive sweep `bool is false`: running the failing input once more, with its panic shown"
        ]
    );
}

#[test]
fn rerun_failing_passing_input_says_not_deterministic() {
    let mut notices = Vec::new();
    Exhaustive::new("flaky", 2).rerun_failing(
        &[1],
        &mut Source::bool,
        &mut |_: &bool| (),
        &mut |notice: &str| notices.push(notice.to_owned()),
    );
    assert_eq!(
        notices.last().map(String::as_str),
        Some(
            "exhaustive sweep `flaky`: the failing input passed when run again, so the test is not deterministic"
        )
    );
}
