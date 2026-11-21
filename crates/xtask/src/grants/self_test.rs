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

/// The evaluator module: every guarded name defined, the first as a type
/// with `visibility`, and an operator on it whose associated type names
/// `Self`.
fn evaluator(visibility: &str) -> String {
    let first = guarded();
    CONFINEMENT
        .guarded
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if index == 0 {
                format!("{visibility} struct {name}(u32);\n")
            } else {
                format!("pub(crate) fn {name}() {{}}\n")
            }
        })
        .chain([format!(
            "impl core::ops::Not for {first} {{\n    type Output = Self;\n    \
             fn not(self) -> Self {{ self }}\n}}\n"
        )])
        .collect()
}

/// A workspace with the evaluator defining every guarded name visible only
/// inside its crate, and code in each allowed path that uses them.
fn after() -> Fixture {
    let mut fixture = before();
    fixture.add_crate(HOME, &[]);
    let uses = format!(
        "use crate::grants::{{{}}};\n",
        CONFINEMENT.guarded.join(", ")
    );
    fixture.file(
        HOME,
        &in_home(CONFINEMENT.evaluator),
        Some(&evaluator("pub(crate)")),
    );
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
        name: "a guarded name in a hidden directory",
        build: || {
            let mut fixture = after();
            let source = format!("pub fn f(_: {}) {{}}\n", guarded());
            fixture.file("ironafs-cache", ".hidden/x.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, "crates/ironafs-cache/.hidden/x.rs"),
    },
    Case {
        name: "a guarded name in a source directory named target",
        build: || {
            let mut fixture = after();
            let source = format!("pub fn f(_: {}) {{}}\n", guarded());
            fixture.file("ironafs-cache", "src/target/x.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, "crates/ironafs-cache/src/target/x.rs"),
    },
    Case {
        name: "a guarded name written as a raw identifier",
        build: || {
            let mut fixture = after();
            let source = format!("pub fn f(_: r#{}) {{}}\n", guarded());
            fixture.file("ironafs-client", "src/lib.rs", Some(&source));
            fixture
        },
        expect: |found| is_outside(found, "crates/ironafs-client/src/lib.rs"),
    },
    Case {
        name: "a guarded name re-exported under another name in an allowed file",
        build: || {
            let mut fixture = after();
            let source = format!("pub use crate::grants::{} as Other;\n", guarded());
            fixture.file(HOME, &allowed_file(), Some(&source));
            fixture
        },
        expect: |found| is_renamed(found, &format!("crates/{HOME}/{}", allowed_file())),
    },
    Case {
        name: "a type alias of a guarded name in an allowed file",
        build: || {
            let mut fixture = after();
            let source = format!("pub type Other = crate::grants::{};\n", guarded());
            fixture.file(HOME, &allowed_file(), Some(&source));
            fixture
        },
        expect: |found| is_renamed(found, &format!("crates/{HOME}/{}", allowed_file())),
    },
    Case {
        name: "an associated type of another type naming a guarded name",
        build: || {
            let mut fixture = after();
            let source = format!(
                "pub trait Carry {{ type Assoc; }}\n\
                 impl Carry for u32 {{ type Assoc = crate::grants::{}; }}\n",
                guarded()
            );
            fixture.file(HOME, &allowed_file(), Some(&source));
            fixture
        },
        expect: |found| is_renamed(found, &format!("crates/{HOME}/{}", allowed_file())),
    },
    Case {
        name: "a type alias inside a function that returns impl Trait",
        build: || {
            let mut fixture = after();
            let source = format!(
                "pub fn f() -> impl Sized {{\n    type Hidden = crate::grants::{};\n    0_u8\n}}\n",
                guarded()
            );
            fixture.file(HOME, &allowed_file(), Some(&source));
            fixture
        },
        expect: |found| is_renamed(found, &format!("crates/{HOME}/{}", allowed_file())),
    },
    Case {
        name: "a guarded definition visible outside its crate",
        build: || {
            let mut fixture = after();
            fixture.file(
                HOME,
                &in_home(CONFINEMENT.evaluator),
                Some(&evaluator("pub")),
            );
            fixture
        },
        expect: |found| is_public(found, CONFINEMENT.evaluator),
    },
    Case {
        name: "a guarded definition with a macro's visibility",
        build: || {
            let mut fixture = after();
            let source = format!(
                "macro_rules! define {{ ($v:vis) => {{ $v struct {}(u32); }}; }}\n",
                guarded()
            );
            let tail = evaluator("pub(crate)");
            let rest = tail.lines().skip(1).collect::<Vec<_>>().join("\n");
            fixture.file(
                HOME,
                &in_home(CONFINEMENT.evaluator),
                Some(&format!("{source}define!(pub);\n{rest}\n")),
            );
            fixture
        },
        expect: |found| is_public(found, CONFINEMENT.evaluator),
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

fn is_public(violation: &Violation, path: &str) -> bool {
    matches!(violation, Violation::Public { file, .. } if file == Path::new(path))
}

fn is_renamed(violation: &Violation, path: &str) -> bool {
    matches!(violation, Violation::Renamed { file, .. } if file == Path::new(path))
}

/// A file, relative to the home crate, in the first allowed directory,
/// beside the one the clean fixture writes there.
fn allowed_file() -> String {
    let dir = CONFINEMENT
        .allowed
        .iter()
        .find(|allowed| allowed.ends_with('/'))
        .copied()
        .unwrap_or(CONFINEMENT.evaluator);
    format!("{}planted.rs", in_home(dir))
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
        packages: Vec::new(),
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
