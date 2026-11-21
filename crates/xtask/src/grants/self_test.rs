// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-grants --self-test`: plants the evaluator's names outside its
//! module, and a missing or empty module, in fixture workspaces built with
//! the configured confinement, and proves the check fails on each and passes
//! on the clean fixtures.

use std::io::Write;
use std::path::Path;

use super::{CONFINEMENT, Violation, check};
use crate::Error;
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;

const CHECK: &str = "check-grants";
const HOME: &str = CONFINEMENT.home_crate;

/// The first guarded name, used in planted files.
fn guarded() -> &'static str {
    CONFINEMENT.guarded.first().copied().unwrap_or_default()
}

/// A workspace before the evaluator's crate exists. Comments and strings
/// may still mention the guarded names.
fn before() -> Fixture {
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-cache", &[]);
    fixture.add_crate("ironafs-client", &["ironafs-cache"]);
    let mentions = format!(
        "//! {}\npub const NOTE: &str = \"{}\";\n",
        CONFINEMENT.guarded.join(", "),
        CONFINEMENT.guarded.join(", ")
    );
    fixture.file("ironafs-cache", "src/lib.rs", Some(&mentions));
    fixture
}

/// Relative to the home crate's directory: the configured path with the
/// `crates/<home>/` prefix taken off.
fn in_home(path: &str) -> String {
    path.strip_prefix(&format!("crates/{HOME}/"))
        .unwrap_or(path)
        .to_owned()
}

/// A workspace with the evaluator defining every guarded name, and code in
/// each allowed path that uses them.
fn after() -> Fixture {
    let mut fixture = before();
    fixture.add_crate(HOME, &[]);
    let definitions: String = CONFINEMENT
        .guarded
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if index == 0 {
                format!("pub struct {name}(u32);\n")
            } else {
                format!("pub fn {name}() {{}}\n")
            }
        })
        .collect();
    let uses = format!(
        "use crate::grants::{{{}}};\n",
        CONFINEMENT.guarded.join(", ")
    );
    fixture.file(HOME, &in_home(CONFINEMENT.evaluator), Some(&definitions));
    for allowed in CONFINEMENT.allowed {
        if allowed.ends_with('/') {
            fixture.file(HOME, &format!("{}inner.rs", in_home(allowed)), Some(&uses));
        }
    }
    fixture.depend("ironafs-cache", HOME);
    fixture
}

/// A planted case: how to build it, and what every violation it produces
/// must match. At least one violation is required.
struct Case {
    name: &'static str,
    build: fn() -> Fixture,
    expect: fn(&Violation) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a guarded name before the evaluator's crate exists",
        build: || {
            let mut fixture = before();
            let source = format!("pub fn f(_: {}) {{}}\n", guarded());
            fixture.file("ironafs-client", "src/lib.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, "crates/ironafs-client/src/lib.rs"),
    },
    Case {
        name: "a guarded name re-exported from the crate root",
        build: || {
            let mut fixture = after();
            let source = format!("mod grants;\npub use grants::{};\n", guarded());
            fixture.file(HOME, "src/lib.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, &format!("crates/{HOME}/src/lib.rs")),
    },
    Case {
        name: "a guarded name in another crate's integration test",
        build: || {
            let mut fixture = after();
            let source = format!("#![cfg(test)]\nuse ironafs_proto::grants::{};\n", guarded());
            fixture.file("ironafs-cache", "tests/it/main.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, "crates/ironafs-cache/tests/it/main.rs"),
    },
    Case {
        name: "the evaluator's crate without the evaluator module",
        build: || {
            let mut fixture = after();
            fixture.file(HOME, &in_home(CONFINEMENT.evaluator), None);
            fixture
        },
        expect: |found| matches!(found, Violation::EvaluatorMissing(file) if file == Path::new(CONFINEMENT.evaluator)),
    },
    Case {
        name: "the evaluator module without the guarded definitions",
        build: || {
            let mut fixture = after();
            fixture.file(
                HOME,
                &in_home(CONFINEMENT.evaluator),
                Some("pub struct Other;\n"),
            );
            fixture
        },
        expect: |found| matches!(found, Violation::NotDefined(_)),
    },
];

fn is_outside(violation: &Violation, path: &str) -> bool {
    matches!(violation, Violation::Outside { file, .. } if file == Path::new(path))
}

/// Runs the clean fixtures, every planted case, and the empty workspace.
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
        writeln!(output, "{line}").map_err(Error::io("report check-grants self-test"))
    };
    if CONFINEMENT.guarded.is_empty() {
        return Err(failure("the configured confinement names something", &[]));
    }

    for (name, clean, files) in [
        (
            "the clean fixture before the evaluator's crate",
            before(),
            2,
        ),
        (
            "the clean fixture with the evaluator",
            after(),
            4 + CONFINEMENT
                .allowed
                .iter()
                .filter(|path| path.ends_with('/'))
                .count(),
        ),
    ] {
        let (found, scanned) = check_fixture(&clean, &dir.join(name.replace(' ', "-")), name)?;
        if !found.is_empty() || scanned != files {
            return Err(failure(name, &found));
        }
        report(format!("passed: {name} ({scanned} Rust files read)"))?;
    }

    for (index, case) in CASES.iter().enumerate() {
        let (found, _) = check_fixture(
            &(case.build)(),
            &dir.join(format!("case-{index}")),
            case.name,
        )?;
        match found.first() {
            Some(first) if found.iter().all(case.expect) => {
                report(format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }

    let empty = Workspace {
        root: dir.to_path_buf(),
        target_dir: dir.to_path_buf(),
        members: Vec::new(),
    };
    let (found, _) = check(&empty, &CONFINEMENT)?;
    if found != [Violation::NothingScanned] {
        return Err(failure("an empty workspace", &found));
    }
    report("caught: an empty workspace".to_owned())?;
    report(format!(
        "check-grants --self-test: the clean fixtures passed and {} planted cases failed as planted",
        CASES.len() + 1
    ))
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}

/// Writes a fixture, reads it back through `cargo metadata`, confirms every
/// crate was read, and runs the check on it with the configured confinement.
fn check_fixture(
    fixture: &Fixture,
    dir: &Path,
    case: &'static str,
) -> Result<(Vec<Violation>, usize), Error> {
    let workspace = fixture.load(dir, CHECK, case)?;
    check(&workspace, &CONFINEMENT)
}
