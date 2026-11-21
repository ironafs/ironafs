// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `inventory --self-test`: runs nextest on a fixture workspace (two tests
//! that pass, one ignored test, and a test binary gated behind a feature
//! the run does not enable), reads its list and `JUnit` report, then commits
//! a base inventory in a git repository of the fixture and plants each
//! failure case in the working tree, proving the check fails on each and
//! passes on the clean inventory and on a recorded removal.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::source::{self, Cargo};
use super::{CHECK, DIR, Outcome, REMOVED, Run, Violation, check_against, verdict};
use crate::Error;
use crate::fixture::{self, Fixture};

const CRATE: &str = "inv-demo";

/// The clean inventory: the ignored test is quarantined with its issue.
const CLEAN: &str = "inv-demo tests::alpha\ninv-demo tests::beta\n\
                     inv-demo tests::quiet quarantined=#1\n";

/// The fixture: unit tests `alpha` and `beta`, `quiet` ignored, and a test
/// binary `gated` that only the `extra` feature builds.
fn fixture() -> Fixture {
    let mut fixture = Fixture::default();
    fixture.add_crate(CRATE, &[]);
    fixture.file(CRATE, "src/lib.rs", Some("#[cfg(test)]\nmod tests;\n"));
    fixture.file(
        CRATE,
        "src/tests.rs",
        Some(
            "#[test]\nfn alpha() {}\n\n#[test]\nfn beta() {}\n\n\
             #[test]\n#[ignore = \"quarantined\"]\nfn quiet() {}\n",
        ),
    );
    fixture.file(CRATE, "tests/gated.rs", Some("#[test]\nfn gated() {}\n"));
    fixture.manifest_tail(
        CRATE,
        "\n[features]\nextra = []\n\n[[test]]\nname = \"gated\"\nrequired-features = [\"extra\"]\n",
    );
    fixture.root_file(
        ".config/nextest.toml",
        "[profile.ci]\nretries = 0\nfail-fast = false\n\n[profile.ci.junit]\npath = \"junit.xml\"\n",
    );
    fixture.root_file(".gitignore", "target/\n");
    fixture
}

/// One revision's inventory: the crate's file and the removal record.
type Revision = (Option<&'static str>, Option<&'static str>);

/// A planted case: the base revision's inventory, the working tree's, a
/// change to the run, and what the violations must show.
struct Case {
    name: &'static str,
    base: Revision,
    current: Revision,
    plant: fn(&mut Run),
    expect: fn(&Outcome) -> bool,
}

fn keep(_: &mut Run) {}

fn only(outcome: &Outcome, expect: impl Fn(&Violation) -> bool) -> bool {
    !outcome.violations.is_empty() && outcome.violations.iter().all(expect)
}

fn named(key: &super::Key, name: &str) -> bool {
    key.name == name
}

const CASES: &[Case] = &[
    Case {
        name: "the clean inventory, its quarantined test skipped",
        base: (Some(CLEAN), None),
        current: (Some(CLEAN), None),
        plant: keep,
        expect: |outcome| outcome.violations.is_empty() && outcome.skipped.len() == 1,
    },
    Case {
        name: "a test removed without its line",
        base: (Some(CLEAN), None),
        current: (
            Some(
                "inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet quarantined=#1\ninv-demo tests::removed\n",
            ),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::ListedNotRun { key } if named(key, "tests::removed")),
            )
        },
    },
    Case {
        name: "a test in the run missing from the inventory",
        base: (
            Some("inv-demo tests::alpha\ninv-demo tests::quiet quarantined=#1\n"),
            None,
        ),
        current: (
            Some("inv-demo tests::alpha\ninv-demo tests::quiet quarantined=#1\n"),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::Unlisted { key, .. } if named(key, "tests::beta")),
            )
        },
    },
    Case {
        name: "a test and its line removed together, with no removal record",
        base: (
            Some(
                "inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::old\ninv-demo tests::quiet quarantined=#1\n",
            ),
            None,
        ),
        current: (Some(CLEAN), None),
        plant: keep,
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::UnrecordedRemoval { name, .. } if name == "tests::old"),
            )
        },
    },
    Case {
        name: "the same removal, recorded",
        base: (
            Some(
                "inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::old\ninv-demo tests::quiet quarantined=#1\n",
            ),
            None,
        ),
        current: (
            Some(CLEAN),
            Some("inv-demo tests::old #1 merged into alpha\n"),
        ),
        plant: keep,
        expect: |outcome| outcome.violations.is_empty() && outcome.removed.len() == 1,
    },
    Case {
        name: "a removal-record line deleted",
        base: (
            Some(CLEAN),
            Some("inv-demo tests::old #1 merged into alpha\n"),
        ),
        current: (Some(CLEAN), None),
        plant: keep,
        expect: |outcome| {
            only(outcome, |found| {
                matches!(found, Violation::RemovalRecordDeleted { .. })
            })
        },
    },
    Case {
        name: "a quarantined test dropped from the inventory",
        base: (Some(CLEAN), None),
        current: (
            Some("inv-demo tests::alpha\ninv-demo tests::beta\n"),
            Some("inv-demo tests::quiet #1 never runs\n"),
        ),
        plant: keep,
        expect: |outcome| {
            outcome.violations.iter().any(|found| {
                matches!(found, Violation::DroppedQuarantine { key, issue: 1 } if named(key, "tests::quiet"))
            })
        },
    },
    Case {
        name: "a quarantine mark removed while the test still does not run",
        base: (Some(CLEAN), None),
        current: (
            Some("inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet\n"),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            outcome
                .violations
                .iter()
                .any(|found| matches!(found, Violation::QuarantineLifted { issue: 1, .. }))
        },
    },
    Case {
        name: "a quarantine mark with no issue",
        base: (Some(CLEAN), None),
        current: (
            Some(
                "inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet quarantined\n",
            ),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            only(outcome, |found| {
                matches!(found, Violation::QuarantineWithoutIssue { .. })
            })
        },
    },
    Case {
        name: "a skipped test without the quarantine mark",
        base: (
            Some("inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet\n"),
            None,
        ),
        current: (
            Some("inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet\n"),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::ListedNotRun { key } if named(key, "tests::quiet")),
            )
        },
    },
    Case {
        name: "a feature-gated binary listed but never executed",
        base: (Some(CLEAN), None),
        current: (
            Some(
                "inv-demo tests::alpha\ninv-demo tests::beta\ninv-demo tests::quiet quarantined=#1\ninv-demo::gated gated\n",
            ),
            None,
        ),
        plant: keep,
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::ListedNotRun { key } if key.binary == "inv-demo::gated"),
            )
        },
    },
    Case {
        name: "a listed test that failed",
        base: (Some(CLEAN), None),
        current: (Some(CLEAN), None),
        plant: |run| {
            for case in &mut run.results {
                case.passed = case.name != "tests::alpha";
            }
        },
        expect: |outcome| {
            only(
                outcome,
                |found| matches!(found, Violation::ListedNotRun { key } if named(key, "tests::alpha")),
            )
        },
    },
];

/// Runs git in the fixture with a fixed identity, no hooks and no signing.
fn git(root: &Path, args: &[&str]) -> Result<(), Error> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args([
            "-c",
            "user.name=xtask",
            "-c",
            "user.email=xtask@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    for variable in source::GIT_LOCATION {
        command.env_remove(variable);
    }
    let status = command.status().map_err(Error::io("git in the fixture"))?;
    Error::check_status("git in the fixture", status)
}

/// Replaces the fixture's inventory directory with `revision`.
fn write_revision(root: &Path, revision: Revision) -> Result<(), Error> {
    let dir = root.join(DIR);
    fixture::remove_dir(&dir)?;
    let (inventory, removed) = revision;
    let mut files = Vec::new();
    if let Some(text) = inventory {
        files.push((CRATE.to_owned(), text.to_owned()));
    }
    if let Some(text) = removed {
        files.push((REMOVED.trim_end_matches(".txt").to_owned(), text.to_owned()));
    }
    if files.is_empty() {
        return Ok(());
    }
    source::write_files(&dir, &files)
}

/// Commits `base` as the base revision, then puts `current` in the
/// working tree, and checks it against the planted run.
fn check_case(root: &Path, run: &Run, base: Revision, current: Revision) -> Result<Outcome, Error> {
    write_revision(root, base)?;
    git(root, &["add", "--all", "--", "."])?;
    git(
        root,
        &[
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "-m",
            "base",
        ],
    )?;
    write_revision(root, current)?;
    // The report is the check's, not the self-test's.
    check_against(root, "HEAD", run, &mut Vec::new())
}

/// The fixture's real run must show what the cases rest on.
fn check_run(run: &Run) -> Vec<String> {
    let passed = |name: &str| {
        run.results
            .iter()
            .any(|case| case.binary == CRATE && case.name == name && case.passed)
    };
    let listed = |binary: &str, name: &str| {
        run.listed
            .iter()
            .any(|test| test.binary == binary && test.name == name)
    };
    let mut found = Vec::new();
    for name in ["tests::alpha", "tests::beta"] {
        if !passed(name) {
            found.push(format!("{name} did not pass in the fixture's JUnit report"));
        }
    }
    if !listed(CRATE, "tests::quiet") || run.results.iter().any(|case| case.name == "tests::quiet")
    {
        found.push("the ignored test was not listed, or it ran".to_owned());
    }
    if run.listed.iter().any(|test| test.binary.contains("gated")) {
        found.push("the feature-gated binary was built without its feature".to_owned());
    }
    found
}

/// Runs every planted case against a fixture under `target_dir`.
///
/// # Errors
///
/// Returns a self-test failure naming the first case that did not end as
/// planted, or an error if the fixture cannot be built or run.
pub(super) fn run(target_dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_cases(&dir, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_cases(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let manifest = fixture().write(dir)?;
    let root = manifest
        .parent()
        .ok_or(Error::UnexpectedOutput("the fixture's manifest path"))?;
    let run = source::run(&Cargo::Fixture(root))?;
    let found = check_run(&run);
    if !found.is_empty() {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "the fixture's run",
            found,
        });
    }
    git(root, &["init", "--quiet"])?;
    for case in CASES {
        let mut planted = Run {
            target: run.target.clone(),
            features: run.features.clone(),
            listed: Vec::new(),
            results: Vec::new(),
        };
        planted
            .listed
            .extend(run.listed.iter().map(|test| super::Listed {
                package: test.package.clone(),
                binary: test.binary.clone(),
                name: test.name.clone(),
            }));
        planted
            .results
            .extend(run.results.iter().map(|case| crate::junit::Case {
                binary: case.binary.clone(),
                name: case.name.clone(),
                passed: case.passed,
            }));
        (case.plant)(&mut planted);
        let outcome = check_case(root, &planted, case.base, case.current)?;
        if !(case.expect)(&outcome) {
            return Err(Error::SelfTest {
                check: CHECK,
                case: case.name,
                found: outcome.violations.iter().map(ToString::to_string).collect(),
            });
        }
    }
    // A run that selected nothing, with nothing listed: verified nothing.
    let empty = Run {
        target: run.target.clone(),
        features: run.features.clone(),
        listed: Vec::new(),
        results: Vec::new(),
    };
    let outcome = check_case(root, &empty, (None, None), (None, None))?;
    if !matches!(verdict(&outcome), Err(Error::CouldNotRun { .. })) {
        return Err(Error::SelfTest {
            check: CHECK,
            case: "a run that compared no test",
            found: vec!["the check did not report that it verified nothing".to_owned()],
        });
    }
    writeln!(
        output,
        "inventory --self-test: the fixture's run read ({} listed, {} ran); {} planted cases and \
         the empty run ended as planted",
        run.listed.len(),
        run.results.len(),
        CASES.len()
    )
    .map_err(Error::io("report inventory --self-test"))
}
