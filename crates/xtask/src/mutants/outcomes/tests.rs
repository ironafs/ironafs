// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the tool's `outcomes.json` and `--list --json`, and
//! for classing each mutant by IronAFS's rules rather than the tool's.

use super::{Class, parse_list, parse_outcomes, reconcile};
use crate::mutants::fixture::{
    BASELINE_OK, FILE, TEST_ARGV, baseline_failed, list, mutant, outcomes, phase,
};

fn class_of(scenario: &str) -> Class {
    let parsed = parse_outcomes(&outcomes(&[BASELINE_OK, scenario])).expect("parses");
    assert_eq!(parsed.mutants.len(), 1);
    parsed.mutants[0].class.clone()
}

fn no_result(class: &Class) -> &str {
    match class {
        Class::NoResult(reason) => reason,
        other => panic!("expected no result, found {other:?}"),
    }
}

#[test]
fn caught_needs_the_test_phase_to_report_failed_tests() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Failure\":100}", TEST_ARGV),
        ],
    );
    assert_eq!(class_of(&scenario), Class::Caught);
}

#[test]
fn no_tests_run_is_no_result_not_caught() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Failure\":4}", TEST_ARGV),
        ],
    );
    assert!(no_result(&class_of(&scenario)).contains("no test ran"));
}

#[test]
fn build_failure_in_the_test_phase_is_no_result() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Failure\":101}", TEST_ARGV),
        ],
    );
    assert!(no_result(&class_of(&scenario)).contains("101"));
}

#[test]
fn a_signalled_test_phase_is_no_result() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Signalled\":9}", TEST_ARGV),
        ],
    );
    no_result(&class_of(&scenario));
}

#[test]
fn caught_without_a_test_phase_is_no_result() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[phase("Build", "{\"Failure\":101}", "[]")],
    );
    no_result(&class_of(&scenario));
}

#[test]
fn missed_needs_a_passing_test_phase() {
    let scenario = mutant(
        "a",
        "MissedMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "\"Success\"", TEST_ARGV),
        ],
    );
    assert_eq!(class_of(&scenario), Class::Missed);
}

#[test]
fn mutants_run_never_counts_unbuilt_mutant_caught() {
    let scenario = mutant(
        "a",
        "Unviable",
        &[phase("Build", "{\"Failure\":101}", "[]")],
    );
    assert_eq!(class_of(&scenario), Class::Unviable);
}

#[test]
fn unviable_without_a_failed_build_is_no_result() {
    let scenario = mutant("a", "Unviable", &[phase("Build", "\"Success\"", "[]")]);
    no_result(&class_of(&scenario));
}

#[test]
fn a_timeout_is_its_own_class() {
    let scenario = mutant(
        "a",
        "Timeout",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "\"Timeout\"", TEST_ARGV),
        ],
    );
    assert_eq!(class_of(&scenario), Class::Timeout);
}

#[test]
fn a_test_phase_without_retries_zero_is_no_result() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase(
                "Test",
                "{\"Failure\":100}",
                "[\"cargo\",\"nextest\",\"run\",\"--profile=mutants\"]",
            ),
        ],
    );
    assert!(no_result(&class_of(&scenario)).contains("--retries"));
}

#[test]
fn a_test_phase_not_run_by_nextest_is_no_result() {
    let scenario = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase(
                "Test",
                "{\"Failure\":101}",
                "[\"cargo\",\"test\",\"--retries=0\"]",
            ),
        ],
    );
    assert!(no_result(&class_of(&scenario)).contains("nextest"));
}

#[test]
fn another_summary_is_no_result() {
    let scenario = mutant(
        "a",
        "Failure",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "\"Other\"", TEST_ARGV),
        ],
    );
    no_result(&class_of(&scenario));
}

#[test]
fn the_mutant_keeps_its_name_package_and_file() {
    let parsed = parse_outcomes(&outcomes(&[
        BASELINE_OK,
        &mutant(
            "a",
            "MissedMutant",
            &[
                phase("Build", "\"Success\"", "[]"),
                phase("Test", "\"Success\"", TEST_ARGV),
            ],
        ),
    ]))
    .expect("parses");
    let found = &parsed.mutants[0];
    assert_eq!(found.package, "ironafs-time");
    assert_eq!(found.file, FILE);
    assert_eq!(found.name, format!("{FILE}:10:5: a"));
    assert_eq!(found.key().as_deref(), Some(format!("{FILE}: a").as_str()));
    assert_eq!(parsed.version, "27.1.0");
    assert_eq!(parsed.baseline, Some(true));
}

#[test]
fn a_failed_baseline_is_recorded() {
    let parsed = parse_outcomes(&outcomes(&[&baseline_failed()])).expect("parses");
    assert_eq!(parsed.baseline, Some(false));
}

#[test]
fn malformed_outcomes_are_refused() {
    for text in [
        "",
        "{}",
        "[]",
        "{\"outcomes\":[],\"cargo_mutants_version\":7}",
        "{\"outcomes\":[{\"scenario\":\"Baseline\"}],\"cargo_mutants_version\":\"27.1.0\"}",
        "{\"outcomes\":[{\"scenario\":{\"Mutant\":{}},\"summary\":\"CaughtMutant\",\
         \"phase_results\":[]}],\"cargo_mutants_version\":\"27.1.0\"}",
    ] {
        assert!(parse_outcomes(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn the_list_gives_mutant_names() {
    assert_eq!(
        parse_list(&list(&["a", "b"])).expect("parses"),
        vec![format!("{FILE}:10:5: a"), format!("{FILE}:10:5: b")]
    );
    assert!(parse_list("{}").is_err());
    assert!(parse_list("[{}]").is_err());
}

#[test]
fn every_listed_mutant_needs_exactly_one_outcome() {
    let tested = |name| {
        mutant(
            name,
            "CaughtMutant",
            &[
                phase("Build", "\"Success\"", "[]"),
                phase("Test", "{\"Failure\":100}", TEST_ARGV),
            ],
        )
    };
    let parsed = parse_outcomes(&outcomes(&[BASELINE_OK, &tested("a"), &tested("a")])).unwrap();
    let listed = parse_list(&list(&["a", "b"])).unwrap();
    let classed = reconcile(&parsed, &listed);
    let reasons: Vec<&str> = classed.iter().map(|m| no_result(&m.class)).collect();
    assert_eq!(classed.len(), 2, "{classed:?}");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("more than one outcome"))
    );
    assert!(reasons.iter().any(|reason| reason.contains("no outcome")));
}

#[test]
fn an_outcome_that_was_not_listed_is_no_result() {
    let parsed = parse_outcomes(&outcomes(&[
        BASELINE_OK,
        &mutant(
            "c",
            "CaughtMutant",
            &[
                phase("Build", "\"Success\"", "[]"),
                phase("Test", "{\"Failure\":100}", TEST_ARGV),
            ],
        ),
    ]))
    .unwrap();
    let classed = reconcile(&parsed, &[]);
    assert!(no_result(&classed[0].class).contains("not in the shard's list"));
}

#[test]
fn reconcile_keeps_a_matched_class() {
    let parsed = parse_outcomes(&outcomes(&[
        BASELINE_OK,
        &mutant(
            "a",
            "MissedMutant",
            &[
                phase("Build", "\"Success\"", "[]"),
                phase("Test", "\"Success\"", TEST_ARGV),
            ],
        ),
    ]))
    .unwrap();
    let listed = parse_list(&list(&["a"])).unwrap();
    let classed = reconcile(&parsed, &listed);
    assert_eq!(classed.len(), 1);
    assert_eq!(classed[0].class, Class::Missed);
    assert_eq!(classed[0].package, "ironafs-time");
}
