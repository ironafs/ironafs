// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `mutants --self-test`: plants each way a mutation run can mislead (a
//! run that selected no test, a mutant whose tests did not build, a
//! listed mutant with no outcome, another tool version, a failed
//! baseline, a test run that allowed retries, an accepted mutant with no
//! reason, a skip attribute in source) in the tool's output as text, runs
//! it through the same reader and report as the real run, and checks each
//! is refused, beside clean cases that must pass. It never runs the tool.

use std::io::Write;

use super::accepted::parse_accepted;
use super::fixture::{
    BASELINE_OK, TEST_ARGV, baseline_failed, list, mutant, outcomes, outcomes_of_version, phase,
};
use super::report::{Scope, Shard, diff_text, report};
use super::{CHECK, skip_attribute_violations, whole_run_violation};
use crate::Error;

const SHARD: Scope = Scope::Shard(Shard { index: 0, count: 4 });

enum Expect {
    /// No violation, and the report holds this text.
    Pass(&'static str),
    /// A violation holding this text.
    Fail(&'static str),
}

struct Case {
    name: &'static str,
    outcomes: Option<String>,
    list: String,
    expect: Expect,
}

fn tested(change: &str, status: &str) -> String {
    mutant(
        change,
        if status == "\"Success\"" {
            "MissedMutant"
        } else {
            "CaughtMutant"
        },
        &[
            phase("Build", "\"Success\"", "[]"),
            phase("Test", status, TEST_ARGV),
        ],
    )
}

fn run_cases(cases: Vec<Case>) -> Result<Vec<&'static str>, Error> {
    let mut names = Vec::new();
    for case in cases {
        let found = report(SHARD, case.outcomes.as_deref(), &case.list, &[]);
        let ok = match case.expect {
            Expect::Pass(text) => found.violations.is_empty() && found.text.contains(text),
            Expect::Fail(text) => found.violations.iter().any(|v| v.contains(text)),
        };
        if !ok {
            let mut shown = found.violations;
            shown.push(found.text);
            return Err(Error::SelfTest {
                check: CHECK,
                case: case.name,
                found: shown,
            });
        }
        names.push(case.name);
    }
    Ok(names)
}

fn cases() -> Vec<Case> {
    let one = || list(&["a"]);
    let ok = |scenario: String| Some(outcomes(&[BASELINE_OK, &scenario]));
    vec![
        Case {
            name: "a clean shard passes",
            outcomes: Some(outcomes(&[
                BASELINE_OK,
                &tested("a", "{\"Failure\":100}"),
                &tested("b", "\"Success\""),
            ])),
            list: list(&["a", "b"]),
            expect: Expect::Pass("| ironafs-time | 1 | 1 | 0 | 0 | 0 |"),
        },
        Case {
            name: "a shard with no mutants says so",
            outcomes: None,
            list: "[]".to_owned(),
            expect: Expect::Pass("This shard has no mutants."),
        },
        Case {
            name: "a run that selected no test is never caught",
            outcomes: ok(tested("a", "{\"Failure\":4}")),
            list: one(),
            expect: Expect::Fail("no test ran"),
        },
        Case {
            name: "tests that did not build are never caught",
            outcomes: ok(tested("a", "{\"Failure\":101}")),
            list: one(),
            expect: Expect::Fail("exited 101"),
        },
        Case {
            name: "a mutant that does not build is unviable, not caught",
            outcomes: ok(mutant(
                "a",
                "Unviable",
                &[phase("Build", "{\"Failure\":101}", "[]")],
            )),
            list: one(),
            expect: Expect::Pass("| ironafs-time | 0 | 0 | 0 | 1 | 0 |"),
        },
        Case {
            name: "a timed-out mutant is counted apart",
            outcomes: ok(mutant(
                "a",
                "Timeout",
                &[
                    phase("Build", "\"Success\"", "[]"),
                    phase("Test", "\"Timeout\"", TEST_ARGV),
                ],
            )),
            list: one(),
            expect: Expect::Pass("| ironafs-time | 0 | 0 | 1 | 0 | 0 |"),
        },
    ]
}

/// The cases where a mutant's result or the whole run cannot be trusted.
fn untrusted_cases() -> Vec<Case> {
    let one = || list(&["a"]);
    let ok = |scenario: String| Some(outcomes(&[BASELINE_OK, &scenario]));
    vec![
        Case {
            name: "a listed mutant with no outcome",
            outcomes: ok(tested("a", "{\"Failure\":100}")),
            list: list(&["a", "b"]),
            expect: Expect::Fail("has no outcome"),
        },
        Case {
            name: "a listed shard whose tool did not run",
            outcomes: None,
            list: one(),
            expect: Expect::Fail("no outcomes"),
        },
        Case {
            name: "another tool version",
            outcomes: Some(outcomes_of_version(
                "27.0.0",
                &[BASELINE_OK, &tested("a", "{\"Failure\":100}")],
            )),
            list: one(),
            expect: Expect::Fail("not the pinned"),
        },
        Case {
            name: "a failed baseline",
            outcomes: Some(outcomes(&[
                &baseline_failed(),
                &tested("a", "{\"Failure\":100}"),
            ])),
            list: one(),
            expect: Expect::Fail("baseline"),
        },
        Case {
            name: "a test run that allowed retries",
            outcomes: ok(mutant(
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
            )),
            list: one(),
            expect: Expect::Fail("--retries 0"),
        },
        Case {
            name: "malformed outcomes",
            outcomes: Some("{\"outcomes\":".to_owned()),
            list: one(),
            expect: Expect::Fail("malformed"),
        },
    ]
}

/// Runs every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut names = run_cases(cases())?;
    names.extend(run_cases(untrusted_cases())?);

    let (_, violations) = parse_accepted("mutant: crates/a/src/lib.rs: replace f with ()\n");
    if !violations.iter().any(|v| v.contains("no reason")) {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "an accepted mutant with no reason",
            found: violations,
        });
    }
    names.push("an accepted mutant with no reason");

    if whole_run_violation("[]").is_none() {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "exclusions that leave the whole run nothing to mutate",
            found: Vec::new(),
        });
    }
    names.push("exclusions that leave the whole run nothing to mutate");

    let empty = report(Scope::Diff, None, "[]", &[]);
    if !(empty.violations.is_empty() && empty.text.contains("The diff touches no mutants.")) {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "a diff with no mutants says so",
            found: empty.violations,
        });
    }
    names.push("a diff with no mutants says so");

    let no_tests = list(&["a"]);
    let untrusted = report(
        Scope::Diff,
        Some(&outcomes(&[BASELINE_OK, &tested("a", "{\"Failure\":4}")])),
        &no_tests,
        &[],
    );
    let shown = diff_text(untrusted);
    if !(shown.contains("### Not trusted") && shown.contains("no test ran")) {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "a diff run's untrusted result is shown, not hidden",
            found: vec![shown],
        });
    }
    names.push("a diff run's untrusted result is shown, not hidden");

    let planted = "#[mutants::skip]\nfn f() {}\n";
    let found = skip_attribute_violations([("crates/a/src/lib.rs", planted)]);
    if found.is_empty() {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "a skip attribute in source",
            found,
        });
    }
    names.push("a skip attribute in source");

    for name in names {
        writeln!(output, "{CHECK} --self-test: {name}: as planted")
            .map_err(Error::io("write self-test result"))?;
    }
    Ok(())
}
