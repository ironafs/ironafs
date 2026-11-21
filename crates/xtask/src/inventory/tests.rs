// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the inventory's format, its reconciliation with a run, and the
//! rules checked against the base revision's inventory.

use super::{Files, Listed, Outcome, Run, Violation, reconcile, update};
use crate::junit::Case;

const TARGET: &str = "x86_64-unknown-linux-gnu";

/// A run on [`TARGET`] that listed `listed` (binary, name) in crate `demo`
/// and passed `passed`; every other listed test did not run.
fn run(listed: &[(&str, &str)], passed: &[(&str, &str)]) -> Run {
    Run {
        target: TARGET.to_owned(),
        features: "default".to_owned(),
        listed: listed
            .iter()
            .map(|(binary, name)| Listed {
                package: "demo".to_owned(),
                binary: (*binary).to_owned(),
                name: (*name).to_owned(),
            })
            .collect(),
        results: passed
            .iter()
            .map(|(binary, name)| Case {
                binary: (*binary).to_owned(),
                name: (*name).to_owned(),
                passed: true,
            })
            .collect(),
    }
}

/// The usual run: `demo` lists `a` and `b`, and both pass.
fn ordinary_run() -> Run {
    run(
        &[("demo", "a"), ("demo", "b")],
        &[("demo", "a"), ("demo", "b")],
    )
}

fn files(demo: &str, removed: Option<&str>) -> Files {
    Files {
        inventories: vec![("demo".to_owned(), demo.to_owned())],
        removed: removed.map(str::to_owned),
    }
}

fn violations(outcome: &Outcome) -> Vec<String> {
    outcome.violations.iter().map(ToString::to_string).collect()
}

fn assert_only(outcome: &Outcome, expect: impl Fn(&Violation) -> bool) {
    assert!(!outcome.violations.is_empty(), "nothing found");
    assert!(
        outcome.violations.iter().all(expect),
        "{:?}",
        violations(outcome)
    );
}

#[test]
fn inventory_check_passes_when_inventory_matches_the_run() {
    let current = files("demo a\ndemo b\n", None);
    let outcome = reconcile(&current, Some(&current), &ordinary_run());

    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(outcome.compared, 2);
    assert_eq!(
        outcome.counts,
        ["demo: 2 tests on x86_64-unknown-linux-gnu (base 2, +0 -0)"]
    );
}

#[test]
fn inventory_check_fails_on_removed_test() {
    // The test `b` is gone from the source; its line is still listed.
    let current = files("demo a\ndemo b\n", None);
    let outcome = reconcile(
        &current,
        Some(&current),
        &run(&[("demo", "a")], &[("demo", "a")]),
    );

    assert_only(
        &outcome,
        |found| matches!(found, Violation::ListedNotRun { key } if key.name == "b"),
    );
}

#[test]
fn inventory_check_fails_on_unlisted_test() {
    let current = files("demo a\n", None);
    let outcome = reconcile(&current, Some(&current), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::Unlisted { key, .. } if key.name == "b"),
    );
    assert!(violations(&outcome)[0].contains("demo b target=x86_64-unknown-linux-gnu"));
}

#[test]
fn inventory_check_fails_on_unrecorded_removal_against_main() {
    // Test and line removed together: the pull request's own files agree.
    let base = files("demo a\ndemo b\ndemo c\n", None);
    let current = files("demo a\ndemo b\n", None);
    let outcome = reconcile(&current, Some(&base), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::UnrecordedRemoval { binary, name } if binary == "demo" && name == "c"),
    );

    let recorded = files("demo a\ndemo b\n", Some("demo c #9 merged into b\n"));
    let outcome = reconcile(&recorded, Some(&base), &ordinary_run());
    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(outcome.removed, ["demo c (#9 merged into b)"]);
}

#[test]
fn inventory_check_fails_on_narrowed_target_against_main() {
    // Narrowing a line to one target removes it from the others.
    let base = files("demo a\ndemo b\ndemo c\n", None);
    let current = files(
        "demo a\ndemo b\ndemo c target=x86_64-unknown-linux-gnu\n",
        None,
    );
    let all = [("demo", "a"), ("demo", "b"), ("demo", "c")];
    let outcome = reconcile(&current, Some(&base), &run(&all, &all));

    assert_only(
        &outcome,
        |found| matches!(found, Violation::UnrecordedRemoval { name, .. } if name == "c"),
    );
}

#[test]
fn inventory_check_fails_on_deleted_removal_record() {
    let base = files(
        "demo a\ndemo b\n",
        Some("demo c #9 merged\ndemo d #10 gone\n"),
    );
    let current = files("demo a\ndemo b\n", Some("demo d #10 gone\n"));
    let outcome = reconcile(&current, Some(&base), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::RemovalRecordDeleted { line } if line == "demo c #9 merged"),
    );

    let edited = files(
        "demo a\ndemo b\n",
        Some("demo c #9 merged!\ndemo d #10 gone\n"),
    );
    let outcome = reconcile(&edited, Some(&base), &ordinary_run());
    assert_only(&outcome, |found| {
        matches!(found, Violation::RemovalRecordDeleted { .. })
    });
}

#[test]
fn inventory_check_fails_on_malformed_removal_record() {
    for line in ["demo c", "demo c 9 reason", "demo c #9", "demo c #x reason"] {
        let current = files("demo a\ndemo b\n", Some(&format!("{line}\n")));
        let outcome = reconcile(&current, Some(&current), &ordinary_run());
        assert_only(
            &outcome,
            |found| matches!(found, Violation::Format { file, .. } if file == "removed.txt"),
        );
    }
}

#[test]
fn inventory_check_fails_on_dropped_quarantine_against_main() {
    let base = files("demo a\ndemo b\ndemo q quarantined=#7\n", None);
    let quarantined_run = run(
        &[("demo", "a"), ("demo", "b"), ("demo", "q")],
        &[("demo", "a"), ("demo", "b")],
    );

    // Dropped together with its test, even with a removal record.
    let dropped = files("demo a\ndemo b\n", Some("demo q #9 flaky\n"));
    let outcome = reconcile(&dropped, Some(&base), &ordinary_run());
    assert_only(
        &outcome,
        |found| matches!(found, Violation::DroppedQuarantine { key, issue: 7 } if key.name == "q"),
    );

    // The mark removed while the test still does not run.
    let unmarked = files("demo a\ndemo b\ndemo q\n", None);
    let outcome = reconcile(&unmarked, Some(&base), &quarantined_run);
    assert!(
        violations(&outcome)
            .iter()
            .any(|found| found.contains("demo q") && found.contains("#7")),
        "{:?}",
        violations(&outcome)
    );

    // The mark removed with the test running again passes.
    let running = run(
        &[("demo", "a"), ("demo", "b"), ("demo", "q")],
        &[("demo", "a"), ("demo", "b"), ("demo", "q")],
    );
    let outcome = reconcile(&unmarked, Some(&base), &running);
    assert_eq!(violations(&outcome), [] as [String; 0]);
}

#[test]
fn inventory_check_fails_on_quarantine_without_issue() {
    let quarantined_run = run(
        &[("demo", "a"), ("demo", "b"), ("demo", "q")],
        &[("demo", "a"), ("demo", "b")],
    );
    for mark in [
        "quarantined",
        "quarantined=",
        "quarantined=7",
        "quarantined=#",
        "quarantined=#x",
    ] {
        let current = files(&format!("demo a\ndemo b\ndemo q {mark}\n"), None);
        let outcome = reconcile(&current, Some(&current), &quarantined_run);
        assert_only(&outcome, |found| {
            matches!(found, Violation::QuarantineWithoutIssue { .. })
        });
    }
}

#[test]
fn inventory_check_fails_on_listed_test_not_run() {
    // A feature-gated binary listed under the default feature set is never
    // built, so it is neither listed by nextest nor run.
    let current = files("demo a\ndemo b\ndemo::gated g\n", None);
    let outcome = reconcile(&current, Some(&current), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::ListedNotRun { key } if key.binary == "demo::gated"),
    );

    // Listed under a feature set no job runs: refused as unknown.
    let current = files("demo a\ndemo b\ndemo::gated g features=extra\n", None);
    let outcome = reconcile(&current, Some(&current), &ordinary_run());
    assert_only(&outcome, |found| matches!(found, Violation::Format { .. }));
}

#[test]
fn inventory_check_fails_on_listed_test_that_failed() {
    let current = files("demo a\ndemo b\n", None);
    let mut failed = ordinary_run();
    failed.results[1].passed = false;
    let outcome = reconcile(&current, Some(&current), &failed);

    assert_only(
        &outcome,
        |found| matches!(found, Violation::ListedNotRun { key } if key.name == "b"),
    );
}

#[test]
fn inventory_check_passes_on_marked_quarantine() {
    // `q` is ignored: listed by nextest, not run.
    let skipped_run = run(
        &[("demo", "a"), ("demo", "b"), ("demo", "q")],
        &[("demo", "a"), ("demo", "b")],
    );
    let marked = files("demo a\ndemo b\ndemo q quarantined=#7\n", None);
    let outcome = reconcile(&marked, Some(&marked), &skipped_run);
    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(
        outcome.skipped,
        ["demo q on x86_64-unknown-linux-gnu, features default (quarantined, #7)"]
    );

    // A new mark is printed by name.
    let base = files("demo a\ndemo b\ndemo q\n", None);
    let outcome = reconcile(&marked, Some(&base), &skipped_run);
    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(outcome.newly_quarantined, ["demo q (#7)"]);

    // The same test skipped without the mark fails.
    let unmarked = files("demo a\ndemo b\ndemo q\n", None);
    let outcome = reconcile(&unmarked, Some(&unmarked), &skipped_run);
    assert_only(
        &outcome,
        |found| matches!(found, Violation::ListedNotRun { key } if key.name == "q"),
    );
}

#[test]
fn inventory_check_compares_nothing_on_an_empty_run() {
    let current = files("", None);
    let outcome = reconcile(&current, Some(&current), &run(&[], &[]));

    assert_eq!(outcome.compared, 0);
}

#[test]
fn inventory_check_ignores_entries_for_other_targets() {
    // A run on the development Mac leaves the Linux-only line to CI.
    let current = files(
        "demo a\ndemo b\ndemo m target=x86_64-unknown-linux-gnu\n",
        None,
    );
    let mut mac = ordinary_run();
    mac.target = "aarch64-apple-darwin".to_owned();
    let outcome = reconcile(&current, Some(&current), &mac);

    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(outcome.compared, 2);
}

#[test]
fn inventory_check_fails_on_malformed_lines() {
    for (text, bad) in [
        ("demo b\ndemo a\n", "unsorted"),
        ("demo a\ndemo a\ndemo b\n", "repeated"),
        (
            "demo a\ndemo a target=x86_64-unknown-linux-gnu\ndemo b\n",
            "overlapping",
        ),
        ("demo a\ndemo b\ndemo c tier=nightly\n", "unknown tag"),
        ("demo a\ndemo b\ndemo c venue=lab\n", "unknown venue tag"),
        (
            "demo a\ndemo b\ndemo c target=riscv64gc-unknown-linux-gnu\n",
            "unknown target",
        ),
        (
            "demo a\ndemo b\ndemo c target=x86_64-unknown-linux-gnu target=x86_64-unknown-linux-gnu\n",
            "repeated tag",
        ),
        ("demo a\ndemo b\nother c\n", "other crate"),
        ("demo a\ndemo b\ndemo\n", "no name"),
        ("demo a\ndemo b\n\n", "blank line"),
        ("demo a\ndemo  b\n", "double space"),
        ("demo a\ndemo b", "no final newline"),
    ] {
        let current = files(text, None);
        let outcome = reconcile(&current, Some(&current), &ordinary_run());
        assert!(
            outcome
                .violations
                .iter()
                .any(|found| matches!(found, Violation::Format { .. })),
            "{bad}: {:?}",
            violations(&outcome)
        );
    }
}

#[test]
fn inventory_check_without_base_inventory_checks_the_run() {
    let current = files("demo a\n", None);
    let outcome = reconcile(&current, None, &ordinary_run());

    assert_only(&outcome, |found| {
        matches!(found, Violation::Unlisted { .. })
    });
}

#[test]
fn update_adds_new_tests_and_keeps_every_line() {
    let current = files("demo a quarantined=#7\ndemo gone\n", None);
    let listed = run(&[("demo", "a"), ("demo", "b"), ("demo::test/it", "c")], &[]);

    let (changed, untag) = update(&current, &listed);
    assert_eq!(
        changed,
        [(
            "demo".to_owned(),
            "demo a quarantined=#7\ndemo b\ndemo gone\ndemo::test/it c\n".to_owned()
        )]
    );
    assert_eq!(untag, [] as [String; 0]);
}

#[test]
fn update_names_a_test_tagged_for_another_target_instead_of_tagging_it() {
    // On the Mac, a test whose only line is tagged for Linux also runs here:
    // its line must lose the tag, which no CI job could check for the Mac.
    let current = files(
        "demo a\ndemo b\ndemo z target=x86_64-unknown-linux-gnu\n",
        None,
    );
    let mut mac = run(&[("demo", "a"), ("demo", "b"), ("demo", "z")], &[]);
    mac.target = "aarch64-apple-darwin".to_owned();

    let (changed, untag) = update(&current, &mac);
    assert_eq!(changed, []);
    assert_eq!(untag, ["demo z"]);
}

#[test]
fn update_writes_a_file_for_a_new_crate_and_nothing_when_complete() {
    let current = files("demo a\ndemo b\n", None);
    assert_eq!(update(&current, &ordinary_run()).0, []);

    let mut listed = ordinary_run();
    listed.listed.push(Listed {
        package: "fresh".to_owned(),
        binary: "fresh".to_owned(),
        name: "t".to_owned(),
    });
    assert_eq!(
        update(&current, &listed).0,
        [("fresh".to_owned(), "fresh t\n".to_owned())]
    );
}

#[test]
fn inventory_exits_2_when_it_could_not_run_and_1_on_violations() {
    use crate::Error;

    let unrun = super::could_not_run(Error::UnexpectedOutput("cargo nextest list"));
    assert!(matches!(
        unrun,
        Error::CouldNotRun {
            check: "inventory",
            ..
        }
    ));
    assert_eq!(unrun.exit_code(), 2);

    let found = super::could_not_run(Error::Rules {
        check: "inventory",
        violations: vec!["x".to_owned()],
    });
    assert_eq!(found.exit_code(), 1);
}

#[test]
fn inventory_verdict_refuses_a_run_that_compared_nothing() {
    use crate::Error;

    let empty = Outcome::default();
    assert!(matches!(
        super::verdict(&empty),
        Err(Error::CouldNotRun {
            check: "inventory",
            ..
        })
    ));

    let compared = Outcome {
        compared: 1,
        ..Outcome::default()
    };
    assert!(super::verdict(&compared).is_ok());

    let found = Outcome {
        violations: vec![Violation::RemovalRecordDeleted {
            line: "x".to_owned(),
        }],
        ..Outcome::default()
    };
    assert!(matches!(super::verdict(&found), Err(Error::Rules { .. })));
}

#[test]
fn inventory_check_fails_on_quarantined_test_not_listed() {
    // The test's source is deleted; its marked line stays. nextest lists
    // ignored and filtered tests, so a quarantined test is still listed.
    let current = files("demo a\ndemo b\ndemo gone quarantined=#7\n", None);
    let outcome = reconcile(&current, Some(&current), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::QuarantinedNotListed { key, issue: 7 } if key.name == "gone"),
    );
}

#[test]
fn inventory_check_fails_on_target_tag_no_job_reconciles() {
    for target in ["aarch64-apple-darwin", "aarch64-unknown-linux-gnu"] {
        let current = files(&format!("demo a\ndemo b\ndemo m target={target}\n"), None);
        let outcome = reconcile(&current, Some(&current), &ordinary_run());
        assert_only(
            &outcome,
            |found| matches!(found, Violation::Format { reason, .. } if reason.contains("no CI job")),
        );
    }
}

#[test]
fn inventory_check_fails_on_removal_excused_by_an_old_record() {
    // `c` was removed once, recorded, re-created, and is now removed again.
    let old = "demo c #9 merged into b\n";
    let base = files("demo a\ndemo b\ndemo c\n", Some(old));
    let current = files("demo a\ndemo b\n", Some(old));
    let outcome = reconcile(&current, Some(&base), &ordinary_run());

    assert_only(
        &outcome,
        |found| matches!(found, Violation::UnrecordedRemoval { name, .. } if name == "c"),
    );

    let again = files(
        "demo a\ndemo b\n",
        Some("demo c #9 merged into b\ndemo c #12 gone again\n"),
    );
    let outcome = reconcile(&again, Some(&base), &ordinary_run());
    assert_eq!(violations(&outcome), [] as [String; 0]);
    assert_eq!(outcome.removed, ["demo c (#12 gone again)"]);
}
