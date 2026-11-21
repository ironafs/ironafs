// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the planted-defect record and the probe that applies each
//! plant, runs its test and restores the file.

use super::fake::{FakeFiles, ScriptedRunner};
use super::{Plant, TestOutcome, Verdict, parse_record, probe};

const SOURCE: &str = "fn f(a: u8, b: u8) -> Option<u8> {\n    a.checked_add(b)\n}\n";

fn plant(anchor: &str, count: usize, change: &str) -> Plant {
    Plant {
        line: 1,
        guard: "f refuses overflow".to_owned(),
        file: "crates/a/src/lib.rs".to_owned(),
        anchor: anchor.to_owned(),
        count,
        change: change.to_owned(),
        package: "a".to_owned(),
        test: "tests::f_overflow_is_none".to_owned(),
    }
}

fn good_plant() -> Plant {
    plant("a.checked_add(b)", 1, "Some(a.wrapping_add(b))")
}

fn no_result(verdict: &Verdict) -> &str {
    match verdict {
        Verdict::NoResult(reason) => reason,
        other => panic!("expected no result, found {other:?}"),
    }
}

#[test]
fn mutation_probe_not_applied_reports_no_result() {
    for (anchor, count) in [("a.checked_sub(b)", 1), ("a", 1), ("a.checked_add(b)", 2)] {
        let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
        let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
        let verdict = probe(&plant(anchor, count, "None"), &mut files, &mut runner).unwrap();
        assert!(
            no_result(&verdict).contains("anchor"),
            "{anchor}: {verdict:?}"
        );
        assert_eq!(files.text("crates/a/src/lib.rs"), SOURCE, "untouched");
        assert_eq!(
            runner.calls(),
            0,
            "no test runs when the plant cannot apply"
        );
    }
}

#[test]
fn probe_catches_when_the_named_test_fails_and_restores() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
    assert_eq!(verdict, Verdict::Caught);
    assert_eq!(runner.calls(), 2);
    assert!(runner.saw_planted("Some(a.wrapping_add(b))"));
    assert_eq!(files.text("crates/a/src/lib.rs"), SOURCE);
}

#[test]
fn probe_reports_survived_when_the_test_still_passes() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Passed]);
    let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
    assert_eq!(verdict, Verdict::Survived);
    assert_eq!(files.text("crates/a/src/lib.rs"), SOURCE);
}

#[test]
fn probe_baseline_not_green_is_no_result_and_plants_nothing() {
    for baseline in [TestOutcome::Failed, TestOutcome::NoTestRan] {
        let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
        let mut runner = ScriptedRunner::new(&[baseline, TestOutcome::Failed]);
        let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
        assert!(no_result(&verdict).contains("baseline"), "{verdict:?}");
        assert_eq!(runner.calls(), 1);
        assert_eq!(files.writes(), 0);
    }
}

#[test]
fn probe_unchanged_file_is_no_result() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let unchanged = plant("a.checked_add(b)", 1, "a.checked_add(b)");
    let verdict = probe(&unchanged, &mut files, &mut runner).unwrap();
    assert!(
        no_result(&verdict).contains("did not change"),
        "{verdict:?}"
    );
}

#[test]
fn probe_write_that_did_not_take_is_no_result() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    files.ignore_writes();
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
    assert!(
        no_result(&verdict).contains("did not change"),
        "{verdict:?}"
    );
    assert_eq!(runner.calls(), 1);
}

#[test]
fn probe_restore_not_verified_is_an_error() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    files.ignore_writes_after(1);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let found = probe(&good_plant(), &mut files, &mut runner);
    let error = found.expect_err("a file left planted stops the probe");
    assert!(error.to_string().contains("restore"), "{error}");
}

#[test]
fn probe_test_that_did_not_run_or_build_is_no_result() {
    for planted in [
        TestOutcome::NoTestRan,
        TestOutcome::DidNotBuild,
        TestOutcome::Other("signal 9".to_owned()),
    ] {
        let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
        let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, planted]);
        let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
        no_result(&verdict);
        assert_eq!(files.text("crates/a/src/lib.rs"), SOURCE);
    }
}

#[test]
fn probe_missing_file_is_no_result() {
    let mut files = FakeFiles::with("crates/b/src/lib.rs", SOURCE);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let verdict = probe(&good_plant(), &mut files, &mut runner).unwrap();
    assert!(no_result(&verdict).contains("read"), "{verdict:?}");
}

const RECORD: &str = "\
# comment

guard: f refuses overflow
file: crates/a/src/lib.rs
anchor: a.checked_add(b)
count: 1
change: Some(a.wrapping_add(b))
package: a
test: tests::f_overflow_is_none
";

#[test]
fn the_record_reads_each_plant() {
    let (plants, violations) = parse_record(RECORD);
    assert_eq!(violations, Vec::<String>::new());
    assert_eq!(plants.len(), 1);
    let found = &plants[0];
    assert_eq!(found.guard, "f refuses overflow");
    assert_eq!(found.file, "crates/a/src/lib.rs");
    assert_eq!(found.anchor, "a.checked_add(b)");
    assert_eq!(found.count, 1);
    assert_eq!(found.change, "Some(a.wrapping_add(b))");
    assert_eq!(found.package, "a");
    assert_eq!(found.test, "tests::f_overflow_is_none");
    assert_eq!(found.line, 3);
}

#[test]
fn the_record_refuses_incomplete_or_doubtful_plants() {
    let cases = [
        (RECORD.replace("count: 1\n", ""), "count"),
        (RECORD.replace("count: 1", "count: 0"), "count"),
        (RECORD.replace("count: 1", "count: one"), "count"),
        (
            RECORD.replace("test: tests::f_overflow_is_none\n", ""),
            "test",
        ),
        (
            RECORD.replace("package: a\n", "package: a\nflavour: x\n"),
            "flavour",
        ),
        (
            RECORD.replace(
                "file: crates/a/src/lib.rs\n",
                "file: crates/a/src/lib.rs\nfile: x\n",
            ),
            "twice",
        ),
        (
            format!("{RECORD}\n{}", RECORD.replace("# comment\n", "")),
            "twice",
        ),
        ("an orphan line\n".to_owned(), "line 1"),
        (
            RECORD.replace("anchor: a.checked_add(b)", "anchor:"),
            "anchor",
        ),
    ];
    for (text, expected) in cases {
        let (_, violations) = parse_record(&text);
        assert!(
            violations.iter().any(|found| found.contains(expected)),
            "{expected}: {violations:?}\n{text}"
        );
    }
}

#[test]
fn plant_self_test_ends_every_case_as_planted() {
    let mut output = Vec::new();
    super::self_test::run(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("a restore that did not happen is an error: as planted"));
}

#[test]
fn probe_failed_plant_write_still_restores_the_original() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    files.fail_write(1);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let found = probe(&good_plant(), &mut files, &mut runner);
    let error = found.expect_err("a failed plant write is an error");
    assert!(error.to_string().contains("plant"), "{error}");
    assert_eq!(files.writes(), 2, "the restore was attempted");
    assert_eq!(files.text("crates/a/src/lib.rs"), SOURCE);
    assert_eq!(runner.calls(), 1, "no test ran on a failed plant");
}

#[test]
fn probe_failed_restore_write_says_the_file_may_be_planted() {
    let mut files = FakeFiles::with("crates/a/src/lib.rs", SOURCE);
    files.fail_write(2);
    let mut runner = ScriptedRunner::new(&[TestOutcome::Passed, TestOutcome::Failed]);
    let error = probe(&good_plant(), &mut files, &mut runner).expect_err("restore failed");
    let text = error.to_string();
    assert!(
        text.contains("restore") && text.contains("crates/a/src/lib.rs"),
        "{text}"
    );
}
