// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plant --self-test`: runs the probe on an in-memory tree with a
//! scripted test runner and checks each way a probe can mislead (an anchor
//! that does not match its count, a plant that changes nothing, a write
//! that did not take or failed, a red baseline, a test that did not run or did not
//! build, a restore that did not happen, a test that still passes) ends as
//! "no result", "survived" or an error, never "caught", beside the clean
//! case that must be caught.

use std::io::Write;

use super::fake::{FakeFiles, ScriptedRunner};
use super::{CHECK, Plant, TestOutcome, Verdict, parse_record, probe};
use crate::Error;

const FILE: &str = "crates/a/src/lib.rs";
const SOURCE: &str = "fn f(a: u8, b: u8) -> Option<u8> {\n    a.checked_add(b)\n}\n";

fn plant(anchor: &str, count: usize, change: &str) -> Plant {
    Plant {
        line: 1,
        guard: "f refuses overflow".to_owned(),
        file: FILE.to_owned(),
        anchor: anchor.to_owned(),
        count,
        change: change.to_owned(),
        package: "a".to_owned(),
        test: "tests::f_overflow_is_none".to_owned(),
    }
}

enum Expect {
    Caught,
    Survived,
    NoResult,
    Error,
}

enum Writes {
    All,
    None,
    First,
    /// The plant's write fails with an error.
    PlantFails,
}

struct Case {
    name: &'static str,
    anchor: &'static str,
    count: usize,
    change: &'static str,
    writes: Writes,
    outcomes: [TestOutcome; 2],
    expect: Expect,
}

const GOOD_ANCHOR: &str = "a.checked_add(b)";
const GOOD_CHANGE: &str = "Some(a.wrapping_add(b))";

fn cases() -> Vec<Case> {
    let case = |name, outcomes, expect| Case {
        name,
        anchor: GOOD_ANCHOR,
        count: 1,
        change: GOOD_CHANGE,
        writes: Writes::All,
        outcomes,
        expect,
    };
    vec![
        case(
            "a plant its test fails on is caught",
            [TestOutcome::Passed, TestOutcome::Failed],
            Expect::Caught,
        ),
        case(
            "a plant its test passes on survived",
            [TestOutcome::Passed, TestOutcome::Passed],
            Expect::Survived,
        ),
        case(
            "a red baseline is no result",
            [TestOutcome::Failed, TestOutcome::Failed],
            Expect::NoResult,
        ),
        case(
            "a test that did not run is no result",
            [TestOutcome::Passed, TestOutcome::NoTestRan],
            Expect::NoResult,
        ),
        case(
            "a plant that did not build is no result",
            [TestOutcome::Passed, TestOutcome::DidNotBuild],
            Expect::NoResult,
        ),
        Case {
            name: "an anchor that does not match is no result",
            anchor: "a.checked_sub(b)",
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::NoResult,
            )
        },
        Case {
            name: "an anchor matching more often than stated is no result",
            anchor: "a",
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::NoResult,
            )
        },
        Case {
            name: "a plant that changes nothing is no result",
            change: GOOD_ANCHOR,
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::NoResult,
            )
        },
        Case {
            name: "a write that did not take is no result",
            writes: Writes::None,
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::NoResult,
            )
        },
        Case {
            name: "a plant write that fails is an error, and the original is restored",
            writes: Writes::PlantFails,
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::Error,
            )
        },
        Case {
            name: "a restore that did not happen is an error",
            writes: Writes::First,
            ..case(
                "",
                [TestOutcome::Passed, TestOutcome::Failed],
                Expect::Error,
            )
        },
    ]
}

/// Runs every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut names = Vec::new();
    for case in cases() {
        let mut files = FakeFiles::with(FILE, SOURCE);
        match case.writes {
            Writes::All => {}
            Writes::None => files.ignore_writes(),
            Writes::First => files.ignore_writes_after(1),
            Writes::PlantFails => files.fail_write(1),
        }
        let mut runner = ScriptedRunner::new(&case.outcomes);
        let found = probe(
            &plant(case.anchor, case.count, case.change),
            &mut files,
            &mut runner,
        );
        let ok = matches!(
            (&case.expect, &found),
            (Expect::Caught, Ok(Verdict::Caught))
                | (Expect::Survived, Ok(Verdict::Survived))
                | (Expect::NoResult, Ok(Verdict::NoResult(_)))
                | (Expect::Error, Err(_))
        );
        let restored = matches!(case.writes, Writes::First) || files.text(FILE) == SOURCE;
        if !ok || !restored {
            return Err(Error::SelfTest {
                check: CHECK,
                case: case.name,
                found: vec![format!("{found:?}"), format!("restored: {restored}")],
            });
        }
        names.push(case.name);
    }

    let (_, violations) =
        parse_record("guard: g\nfile: f.rs\nanchor: a\nchange: b\npackage: p\ntest: t\n");
    if !violations.iter().any(|v| v.contains("count")) {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "a record entry with no count",
            found: violations,
        });
    }
    names.push("a record entry with no count");

    for name in names {
        writeln!(output, "{CHECK} --self-test: {name}: as planted")
            .map_err(Error::io("write self-test result"))?;
    }
    Ok(())
}
