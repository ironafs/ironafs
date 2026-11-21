// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for seed selection, verdicts and the counterexample report.

use std::env::VarError;
use std::ffi::OsString;
use std::panic;

use super::{
    Counterexample, Property, Verdict, parse_seed, resolve_seed, seed_from_name, seed_text,
};
use crate::Source;

/// Runs `property` from `seed`, ignoring `IRONAFS_TEST_SEED`, so that
/// assertions on the seed hold however the tests are run.
fn run_seeded<T, V: Verdict>(
    property: &Property,
    seed: u64,
    mut generate: impl FnMut(&mut Source) -> T,
    mut test: impl FnMut(&T) -> V,
) -> Result<(), Counterexample<T>> {
    property.run_seeded(seed, &mut generate, &mut test)
}

#[test]
fn parse_seed_accepts_decimal_and_hex() {
    assert_eq!(parse_seed("1234"), Some(1234));
    assert_eq!(parse_seed("0x00ff"), Some(255));
    assert_eq!(parse_seed("0XFF"), Some(255));
    assert_eq!(parse_seed(" 7 "), Some(7));
    assert_eq!(parse_seed("0xffffffffffffffff"), Some(u64::MAX));
}

#[test]
fn parse_seed_rejects_malformed_text() {
    for text in ["", "0x", "-1", "12a", "0x1_0", "18446744073709551616"] {
        assert_eq!(parse_seed(text), None, "{text:?}");
    }
}

#[test]
fn resolve_seed_prefers_environment_then_explicit_then_name() {
    assert_eq!(resolve_seed(Some("0x10"), Some(5), "name"), Ok(16));
    assert_eq!(resolve_seed(None, Some(5), "name"), Ok(5));
    assert_eq!(resolve_seed(None, None, "name"), Ok(seed_from_name("name")));
}

#[test]
fn resolve_seed_rejects_malformed_environment() {
    assert_eq!(
        resolve_seed(Some("seed"), Some(5), "name"),
        Err(
            "IRONAFS_TEST_SEED is not a decimal or 0x-prefixed hexadecimal u64: \"seed\""
                .to_owned()
        )
    );
}

#[test]
fn resolve_seed_treats_empty_environment_as_unset() {
    assert_eq!(resolve_seed(Some(""), Some(5), "name"), Ok(5));
    assert_eq!(
        resolve_seed(Some("  "), None, "name"),
        Ok(seed_from_name("name"))
    );
}

#[test]
fn seed_text_reads_present_absent_and_non_unicode_variable() {
    assert_eq!(seed_text(Ok("12".to_owned())), Ok(Some("12".to_owned())));
    assert_eq!(seed_text(Err(VarError::NotPresent)), Ok(None));
    assert_eq!(
        seed_text(Err(VarError::NotUnicode(OsString::from("seed")))),
        Err("IRONAFS_TEST_SEED is not valid Unicode: \"seed\"".to_owned())
    );
}

#[test]
fn cases_zero_panics_with_message() {
    let payload = panic::catch_unwind(|| Property::new("no cases").cases(0)).unwrap_err();
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"a property needs at least one case")
    );
}

#[test]
fn run_seeded_reports_its_seed_and_repeats_its_counterexample() {
    let run = || {
        run_seeded(
            &Property::new("same seed"),
            0xfeed,
            |source| source.vec_of(0..=8, Source::u32),
            |values| assert!(values.len() < 3),
        )
        .unwrap_err()
    };
    let (first, second) = (run(), run());
    assert_eq!(first.seed(), 0xfeed);
    assert_eq!(
        (
            first.case(),
            first.value(),
            first.shrink_runs(),
            first.choices()
        ),
        (
            second.case(),
            second.value(),
            second.shrink_runs(),
            second.choices()
        )
    );
    assert_eq!(*first.value(), vec![0, 0, 0]);
}

#[test]
fn run_seeded_choices_replay_the_minimal_input() {
    let failure = run_seeded(
        &Property::new("choices"),
        1,
        |source| source.vec_of(0..=8, Source::u8),
        |values| assert!(values.iter().all(|value| *value < 100)),
    )
    .unwrap_err();
    assert_eq!(failure.choices(), [1, 100]);
    let mut replay = Source::from_choices(failure.choices().to_vec());
    assert_eq!(replay.vec_of(0..=8, Source::u8), vec![100]);
}

#[test]
fn run_seeded_panic_records_its_location() {
    let line = line!() + 2;
    let failure = run_seeded(&Property::new("location"), 1, Source::u8, |value| {
        assert!(*value < 10);
    })
    .unwrap_err();
    let location = failure.location().unwrap();
    assert!(
        location.starts_with(&format!("{}:{line}:", file!())),
        "{location}"
    );
}

#[test]
fn run_seeded_error_displays_without_location() {
    let failure = run_seeded(&Property::new("error"), 1, Source::u8, |value| {
        if *value < 10 {
            Ok(())
        } else {
            Err(format!("{value} is too big"))
        }
    })
    .unwrap_err();
    assert_eq!(
        (failure.message(), failure.location()),
        ("10 is too big", None)
    );
}

#[test]
fn seed_from_name_is_stable_and_distinguishes_names() {
    assert_eq!(seed_from_name("decode"), seed_from_name("decode"));
    assert_ne!(seed_from_name("decode"), seed_from_name("encode"));
    assert_ne!(seed_from_name(""), seed_from_name("\0"));
}

#[test]
fn verdict_of_unit_is_pass() {
    assert_eq!(().failure(), None);
}

#[test]
fn verdict_of_result_reports_error_display() {
    assert_eq!(Ok::<(), &str>(()).failure(), None);
    assert_eq!(
        Err::<(), _>("bad length").failure(),
        Some("bad length".to_owned())
    );
}

fn sample(shrink_budget_exhausted: bool) -> Counterexample<Vec<u8>> {
    Counterexample {
        location: Some("src/decode.rs:12:5".to_owned()),
        choices: vec![1, 100],
        name: "decode rejects truncated input".to_owned(),
        seed: 0x1234,
        case: 3,
        cases: 256,
        value: vec![100],
        message: "assertion failed: ok".to_owned(),
        shrink_runs: 57,
        shrink_budget_exhausted,
    }
}

#[test]
fn counterexample_display_names_seed_and_replay() {
    assert_eq!(
        sample(false).to_string(),
        "property `decode rejects truncated input` failed on case 4 of 256 \
         (seed 0x0000000000001234)\n\
         minimal input: [100]\n\
         failure: assertion failed: ok\n\
         panicked at: src/decode.rs:12:5\n\
         minimal choices: [1, 100]\n\
         shrunk in 57 runs\n\
         replay with IRONAFS_TEST_SEED=0x0000000000001234"
    );
}

#[test]
fn counterexample_display_warns_when_budget_ran_out() {
    let report = sample(true).to_string();
    assert!(
        report.contains(
            "shrunk in 57 runs; the shrink budget ran out, so the input may not be minimal\n"
        ),
        "{report}"
    );
}

#[test]
fn counterexample_display_omits_location_of_error() {
    let report = Counterexample {
        location: None,
        ..sample(false)
    }
    .to_string();
    assert!(
        report.contains("failure: assertion failed: ok\nminimal choices: [1, 100]\n"),
        "{report}"
    );
}

#[test]
fn counterexample_report_uses_description() {
    let report = sample(false).report(|value| format!("{} bytes", value.len()));
    assert!(report.contains("\nminimal input: 1 bytes\n"), "{report}");
}
