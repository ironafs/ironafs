// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the shard report and the checks that fail it.

use super::{Scope, Shard, diff_text, report};
use crate::mutants::accepted::Accepted;
use crate::mutants::fixture::{
    BASELINE_OK, FILE, TEST_ARGV, baseline_failed, list, mutant, outcomes, outcomes_of_version,
    phase,
};

const SHARD: Scope = Scope::Shard(Shard { index: 1, count: 4 });

fn caught(change: &str) -> String {
    mutant(
        change,
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Failure\":100}", TEST_ARGV),
        ],
    )
}

fn missed(change: &str) -> String {
    mutant(
        change,
        "MissedMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "\"Success\"", TEST_ARGV),
        ],
    )
}

#[test]
fn shard_reads_k_of_n() {
    assert_eq!(Shard::parse("0/4"), Some(Shard { index: 0, count: 4 }));
    assert_eq!(Shard::parse("3/4"), Some(Shard { index: 3, count: 4 }));
    for text in ["4/4", "1/0", "1", "a/4", "-1/4", "1/4/2", ""] {
        assert_eq!(Shard::parse(text), None, "{text}");
    }
    assert_eq!(Shard { index: 1, count: 4 }.to_string(), "1/4");
    assert_eq!(SHARD.to_string(), "shard 1/4");
    assert_eq!(Scope::Diff.to_string(), "the pull request's diff");
}

#[test]
fn a_clean_shard_reports_its_counts_and_passes() {
    let found = report(
        SHARD,
        Some(&outcomes(&[BASELINE_OK, &caught("a"), &missed("b")])),
        &list(&["a", "b"]),
        &[],
    );
    assert_eq!(found.violations, Vec::<String>::new());
    assert!(found.text.contains("shard 1/4"), "{}", found.text);
    assert!(found.text.contains("cargo-mutants 27.1.0"));
    assert!(
        found.text.contains("| ironafs-time | 1 | 1 | 0 | 0 | 0 |"),
        "{}",
        found.text
    );
    assert!(
        found
            .text
            .contains(&format!("`{FILE}:10:5: b`: missed, new"))
    );
}

#[test]
fn an_accepted_survivor_shows_its_reason() {
    let accepted = [Accepted {
        key: format!("{FILE}: b"),
        reason: "logging only".to_owned(),
    }];
    let found = report(
        SHARD,
        Some(&outcomes(&[BASELINE_OK, &missed("b")])),
        &list(&["b"]),
        &accepted,
    );
    assert_eq!(found.violations, [] as [std::string::String; 0]);
    assert!(
        found.text.contains("missed, accepted: logging only"),
        "{}",
        found.text
    );
}

#[test]
fn a_shard_with_no_mutants_says_so() {
    let found = report(SHARD, None, "[]", &[]);
    assert_eq!(found.violations, [] as [std::string::String; 0]);
    assert!(found.text.contains("This shard has no mutants."));
}

#[test]
fn a_listed_shard_without_outcomes_fails() {
    let found = report(SHARD, None, &list(&["a"]), &[]);
    assert!(found.violations.iter().any(|v| v.contains("no outcomes")));
}

#[test]
fn another_tool_version_fails() {
    let found = report(
        SHARD,
        Some(&outcomes_of_version("27.0.0", &[BASELINE_OK, &caught("a")])),
        &list(&["a"]),
        &[],
    );
    assert!(
        found.violations.iter().any(|v| v.contains("27.0.0")),
        "{:?}",
        found.violations
    );
}

#[test]
fn a_failed_or_missing_baseline_fails() {
    for scenarios in [vec![baseline_failed(), caught("a")], vec![caught("a")]] {
        let refs: Vec<&str> = scenarios.iter().map(String::as_str).collect();
        let found = report(SHARD, Some(&outcomes(&refs)), &list(&["a"]), &[]);
        assert!(
            found.violations.iter().any(|v| v.contains("baseline")),
            "{:?}",
            found.violations
        );
    }
}

#[test]
fn a_no_result_mutant_fails_and_is_listed() {
    let no_tests = mutant(
        "a",
        "CaughtMutant",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "{\"Failure\":4}", TEST_ARGV),
        ],
    );
    let found = report(
        SHARD,
        Some(&outcomes(&[BASELINE_OK, &no_tests])),
        &list(&["a"]),
        &[],
    );
    assert!(found.violations.iter().any(|v| v.contains("no test ran")));
    assert!(
        found.text.contains("| ironafs-time | 0 | 0 | 0 | 0 | 1 |"),
        "{}",
        found.text
    );
}

#[test]
fn malformed_outputs_fail() {
    let found = report(SHARD, Some("{"), &list(&["a"]), &[]);
    assert!(found.violations.iter().any(|v| v.contains("outcomes.json")));
    let found = report(SHARD, None, "{", &[]);
    assert!(found.violations.iter().any(|v| v.contains("list")));
}

#[test]
fn survivors_never_fail_the_shard_while_report_only() {
    let timeout = mutant(
        "t",
        "Timeout",
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", "\"Timeout\"", TEST_ARGV),
        ],
    );
    let found = report(
        SHARD,
        Some(&outcomes(&[BASELINE_OK, &missed("b"), &timeout])),
        &list(&["b", "t"]),
        &[],
    );
    assert!(found.violations.is_empty(), "{:?}", found.violations);
    assert!(found.text.contains("timed out, new"));
}

#[test]
fn a_diff_with_no_mutants_says_so() {
    let found = report(Scope::Diff, None, "[]", &[]);
    assert_eq!(found.violations, [] as [std::string::String; 0]);
    assert!(
        found.text.contains("The diff touches no mutants."),
        "{}",
        found.text
    );
}

#[test]
fn the_diff_report_shows_untrusted_results_instead_of_failing() {
    let found = report(Scope::Diff, None, &list(&["a"]), &[]);
    assert_ne!(found.violations, [] as [std::string::String; 0]);
    let text = diff_text(found);
    assert!(text.contains("Report only"), "{text}");
    assert!(text.contains("### Not trusted"), "{text}");
    assert!(text.contains("no outcomes"), "{text}");
    let clean = diff_text(report(Scope::Diff, None, "[]", &[]));
    assert!(!clean.contains("Not trusted"), "{clean}");
}
