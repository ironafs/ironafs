// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-retries --self-test`: plants a retry in each place nextest takes
//! one from, a missing profile file and an unreadable one, in fixture
//! repositories, and proves the check fails on each and passes on the clean
//! fixture.

use std::io::Write;
use std::path::Path;

use super::{CHECK, GITHUB, PROFILES, Violation, check};
use crate::Error;
use crate::fixture::{self, Fixture};

const WORKFLOW: &str = ".github/workflows/ci.yml";
const ACTION: &str = ".github/actions/test/action.yml";
const CARGO_CONFIG: &str = ".cargo/config.toml";

const CLEAN_PROFILES: &str = "# A --retries option or NEXTEST_RETRIES overrides this.\n\
                              [profile.default]\nretries = 0\n\n\
                              [[profile.default.overrides]]\nfilter = 'test(#slow)'\n\
                              threads-required = 2\n\n[profile.ci]\nretries = 0\n";
const CLEAN_WORKFLOW: &str = "jobs:\n  test:\n    env:\n      NEXTEST_RETRIES: 0\n    steps:\n\
                              \x20     - run: cargo nextest run --profile ci --retries 0\n";
const CLEAN_CARGO_CONFIG: &str = "[alias]\nxtask = \"run --package xtask --\"\n";

/// The files of the clean fixture, with `path` set to `text`, or removed
/// when `text` is `None`.
fn with(path: &str, text: Option<&str>) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = [
        (PROFILES, CLEAN_PROFILES),
        (WORKFLOW, CLEAN_WORKFLOW),
        (CARGO_CONFIG, CLEAN_CARGO_CONFIG),
    ]
    .into_iter()
    .filter(|(existing, _)| *existing != path)
    .map(|(existing, text)| (existing.to_owned(), text.to_owned()))
    .collect();
    if let Some(text) = text {
        files.push((path.to_owned(), text.to_owned()));
    }
    files
}

/// A planted case: its files, the file every violation must name, and what
/// every violation must be. At least one violation is required.
struct Case {
    name: &'static str,
    files: fn() -> Vec<(String, String)>,
    expect: fn(&Violation) -> bool,
}

fn names(violation: &Violation, path: &str) -> bool {
    let path = Path::new(path);
    match violation {
        Violation::ProfilesMissing(file)
        | Violation::Unreadable { file, .. }
        | Violation::Profile { file, .. }
        | Violation::Option { file, .. }
        | Violation::Environment { file, .. }
        | Violation::ConfigFile { file, .. } => file == path,
    }
}

const CASES: &[Case] = &[
    Case {
        name: "a profile with retries = 1",
        files: || {
            with(
                PROFILES,
                Some(
                    &CLEAN_PROFILES
                        .replace("[profile.ci]\nretries = 0", "[profile.ci]\nretries = 1"),
                ),
            )
        },
        expect: |found| {
            matches!(found, Violation::Profile { value, .. } if value == "1")
                && names(found, PROFILES)
        },
    },
    Case {
        name: "a per-test override with retries = 2",
        files: || {
            with(
                PROFILES,
                Some(&CLEAN_PROFILES.replace("threads-required = 2", "retries = 2")),
            )
        },
        expect: |found| matches!(found, Violation::Profile { .. }) && names(found, PROFILES),
    },
    Case {
        name: "a profile with retries as a backoff table",
        files: || {
            with(
                PROFILES,
                Some(&CLEAN_PROFILES.replace(
                    "[profile.ci]\nretries = 0",
                    "[profile.ci]\nretries = { backoff = \"fixed\", count = 2 }",
                )),
            )
        },
        expect: |found| matches!(found, Violation::Profile { .. }) && names(found, PROFILES),
    },
    Case {
        name: "an unreadable profile file",
        files: || with(PROFILES, Some("[profile.ci]\nretries = \"0\n")),
        expect: |found| matches!(found, Violation::Unreadable { .. }) && names(found, PROFILES),
    },
    Case {
        name: "no profile file",
        files: || with(PROFILES, None),
        expect: |found| matches!(found, Violation::ProfilesMissing(_)) && names(found, PROFILES),
    },
    Case {
        name: "a workflow command with the retries option at 1",
        files: || {
            with(
                WORKFLOW,
                Some(&CLEAN_WORKFLOW.replace("--retries 0", "--retries 1")),
            )
        },
        expect: |found| matches!(found, Violation::Option { .. }) && names(found, WORKFLOW),
    },
    Case {
        name: "a workflow environment with the retries variable at 1",
        files: || {
            with(
                WORKFLOW,
                Some(&CLEAN_WORKFLOW.replace("NEXTEST_RETRIES: 0", "NEXTEST_RETRIES: 1")),
            )
        },
        expect: |found| matches!(found, Violation::Environment { .. }) && names(found, WORKFLOW),
    },
    Case {
        name: "a composite action setting the retries variable",
        files: || {
            let mut files = with(WORKFLOW, Some(CLEAN_WORKFLOW));
            files.push((
                ACTION.to_owned(),
                "runs:\n  steps:\n    - run: echo \"NEXTEST_RETRIES=2\" >> \"$GITHUB_ENV\"\n"
                    .to_owned(),
            ));
            files
        },
        expect: |found| matches!(found, Violation::Environment { .. }) && names(found, ACTION),
    },
    Case {
        name: "a workflow pointing nextest at another configuration",
        files: || {
            with(
                WORKFLOW,
                Some(&CLEAN_WORKFLOW.replace("--profile ci", "--tool-config-file t:x.toml")),
            )
        },
        expect: |found| matches!(found, Violation::ConfigFile { .. }) && names(found, WORKFLOW),
    },
    Case {
        name: "a cargo alias with the retries option",
        files: || {
            with(
                CARGO_CONFIG,
                Some("[alias]\nt = [\"nextest\", \"run\", \"--retries\", \"3\"]\n"),
            )
        },
        expect: |found| matches!(found, Violation::Option { .. }) && names(found, CARGO_CONFIG),
    },
    Case {
        name: "a cargo environment setting the retries variable",
        files: || with(CARGO_CONFIG, Some("[env]\nNEXTEST_RETRIES = \"1\"\n")),
        expect: |found| {
            matches!(found, Violation::Environment { .. }) && names(found, CARGO_CONFIG)
        },
    },
];

/// Runs the clean fixture and every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(target_dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(&dir, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_in(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let mut report = |line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-retries self-test"))
    };

    let name = "the clean fixture";
    let clean = with("", None);
    let (found, read) = check_fixture(&clean, &dir.join("clean"))?;
    if !found.is_empty() || read != clean.len() {
        return Err(failure(name, &found));
    }
    report(format!("passed: {name} ({read} files read)"))?;

    for (index, case) in CASES.iter().enumerate() {
        let (found, _) = check_fixture(&(case.files)(), &dir.join(format!("case-{index}")))?;
        match found.first() {
            Some(first) if found.iter().all(case.expect) => {
                report(format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }
    report(format!(
        "{CHECK} --self-test: the clean fixture passed and {} planted cases failed as planted \
         (profiles, overrides, {GITHUB}/ commands and environments, cargo configuration)",
        CASES.len()
    ))
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}

/// Writes a fixture repository and runs the check on it.
fn check_fixture(files: &[(String, String)], dir: &Path) -> Result<(Vec<Violation>, usize), Error> {
    let mut fixture = Fixture::default();
    for (path, text) in files {
        fixture.root_file(path, text);
    }
    let manifest = fixture.write(dir)?;
    let root = manifest.parent().unwrap_or(dir);
    check(root, &root.join("target"))
}
