// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-fuzz-deny --self-test`: copies the repository's two policy files
//! into a fixture directory, proves the clean copy passes, then plants each
//! drift (the fuzz file a copy of the root, the engine's licence on the fuzz
//! file's global allow list or on the root's, the exception widened to a
//! second crate, a named entry's root line removed, the fuzz file missing)
//! and proves the check fails on each with the violation planted.

use std::io::Write;
use std::path::Path;

use super::{CHECK, ENGINE_LICENCE, FUZZ_FILE, NAMED, ROOT_FILE, Violation, check};
use crate::Error;
use crate::fixture::{self, write_file};

/// The two files of one planted case: the root policy and the fuzz
/// crate's, or `None` for a file left out.
struct Planted {
    root: String,
    fuzz: Option<String>,
}

/// A planted case: how it changes the repository's files, and what every
/// violation it gives must match. At least one violation is required.
struct Case {
    name: &'static str,
    plant: fn(&str, &str) -> Planted,
    expect: fn(&Violation) -> bool,
}

/// The licence entry on a global allow list, as the root writes them.
fn allow_entry() -> String {
    format!("  \"{ENGINE_LICENCE}\",\n")
}

/// Adds the engine's licence after the first entry of a file's global allow
/// list, or returns the text unchanged if it has none.
fn add_global_allow(text: &str) -> String {
    text.replacen("allow = [\n", &format!("allow = [\n{}", allow_entry()), 1)
}

fn is_drift(found: &Violation) -> bool {
    matches!(found, Violation::Drift { .. })
}

const CASES: &[Case] = &[
    Case {
        name: "the fuzz file a copy of the root",
        plant: |root, _| Planted {
            root: root.to_owned(),
            fuzz: Some(root.to_owned()),
        },
        expect: is_drift,
    },
    Case {
        name: "the engine's licence on the fuzz file's global allow list",
        plant: |root, fuzz| Planted {
            root: root.to_owned(),
            fuzz: Some(add_global_allow(fuzz)),
        },
        expect: is_drift,
    },
    Case {
        name: "the engine's licence on both global allow lists",
        plant: |root, fuzz| Planted {
            root: add_global_allow(root),
            fuzz: Some(add_global_allow(fuzz)),
        },
        expect: |found| matches!(found, Violation::EngineLicenceInRoot { .. }),
    },
    Case {
        name: "the licence exception widened to a second crate",
        plant: |root, fuzz| {
            Planted {
            root: root.to_owned(),
            fuzz: Some(fuzz.replacen(
                "exceptions = [\n",
                &format!(
                    "exceptions = [\n  {{ crate = \"planted@1.0.0\", allow = [\"{ENGINE_LICENCE}\"] }},\n"
                ),
                1,
            )),
        }
        },
        expect: is_drift,
    },
    Case {
        name: "a named entry's root line removed",
        plant: |root, fuzz| Planted {
            root: NAMED
                .first()
                .map_or_else(|| root.to_owned(), |named| root.replace(named.root, "")),
            fuzz: Some(fuzz.to_owned()),
        },
        expect: |found| matches!(found, Violation::Anchor { count: 0, .. }),
    },
    Case {
        name: "the fuzz file missing",
        plant: |root, _| Planted {
            root: root.to_owned(),
            fuzz: None,
        },
        expect: |found| matches!(found, Violation::Unreadable { file, .. } if file == Path::new(FUZZ_FILE)),
    },
];

/// Runs the self-test on copies of the policy files under `root`, in a
/// fixture directory under `target_dir`.
///
/// # Errors
///
/// Returns the first case that did not end as planted, or why the
/// fixtures could not be written.
pub(super) fn run(root: &Path, target_dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(root, &dir, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_in(root: &Path, dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let mut report = |line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-fuzz-deny self-test"))
    };
    let read = |file: &str| {
        let path = root.join(file);
        std::fs::read_to_string(&path).map_err(Error::path_io("read policy file", &path))
    };
    let root_text = read(ROOT_FILE)?;
    let fuzz_text = read(FUZZ_FILE)?;
    if NAMED.is_empty() {
        return Err(failure("the configured entries name something", &[]));
    }

    let clean = Planted {
        root: root_text.clone(),
        fuzz: Some(fuzz_text.clone()),
    };
    let found = check_planted(&clean, &dir.join("clean"))?;
    if !found.is_empty() {
        return Err(failure("the repository's own files", &found));
    }
    report("passed: the repository's own files".to_owned())?;

    for (index, case) in CASES.iter().enumerate() {
        let planted = (case.plant)(&root_text, &fuzz_text);
        let found = check_planted(&planted, &dir.join(format!("case-{index}")))?;
        match found.first() {
            Some(first) if found.iter().all(case.expect) => {
                report(format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }
    report(format!(
        "{CHECK} --self-test: the repository's files passed and {} planted cases failed as planted",
        CASES.len()
    ))
}

/// Writes one case's files under `dir` and runs the check there.
fn check_planted(planted: &Planted, dir: &Path) -> Result<Vec<Violation>, Error> {
    write_file(&dir.join(ROOT_FILE), &planted.root)?;
    if let Some(fuzz) = &planted.fuzz {
        write_file(&dir.join(FUZZ_FILE), fuzz)?;
    }
    Ok(check(dir, NAMED))
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
