// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-bans --self-test`: runs the check on a workspace holding the
//! repository's `clippy.toml`, which must pass, then on copies with a
//! method ban removed, a type ban removed and a ban misspelt, and on a
//! crate whose own `clippy.toml` leaves out a ban, and proves the check
//! fails on each, naming that ban and that file.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, Found, Violation, check_tree};
use crate::Error;
use crate::fixture::{self, write_file};

/// The ban each planted case breaks.
const REMOVED: &str = "std::time::SystemTime::now";
const REMOVED_TYPE: &str = "std::process::Command";
const MISSPELT: &str = "std::time::Instant::elapsed";
const LEFT_OUT: &str = "std::collections::HashMap";

/// The workspace's own configuration, and the one a planted crate's own
/// configuration is written to.
const ROOT_FILE: &str = "clippy.toml";
const CRATE_DIR: &str = "crates/planted";
const CRATE_FILE: &str = "crates/planted/clippy.toml";

/// Where a case writes its planted configuration.
#[derive(Clone, Copy)]
enum Place {
    /// In place of the workspace's `clippy.toml`.
    Root,
    /// As a member crate's own `clippy.toml`, beside the workspace's.
    Crate,
}

/// A planted configuration, what every violation it gives must be, and
/// one violation it must give.
struct Case {
    name: &'static str,
    place: Place,
    plant: fn(&str) -> String,
    expect: fn(&Found) -> bool,
    needs: fn(&Found) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a ban removed from clippy.toml",
        place: Place::Root,
        plant: |text| without(text, REMOVED),
        expect: |found| found.file == ROOT_FILE && names(&found.violation, REMOVED),
        // Clippy itself stops firing, not only the listing check.
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == REMOVED),
    },
    Case {
        name: "a type ban removed from clippy.toml",
        place: Place::Root,
        plant: |text| without(text, REMOVED_TYPE),
        expect: |found| found.file == ROOT_FILE && names(&found.violation, REMOVED_TYPE),
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == REMOVED_TYPE),
    },
    Case {
        name: "a ban misspelt in clippy.toml",
        place: Place::Root,
        plant: |text| text.replace(&format!("\"{MISSPELT}\""), &format!("\"{MISSPELT}x\"")),
        expect: |found| {
            found.file == ROOT_FILE
                && (names(&found.violation, MISSPELT)
                    || matches!(&found.violation, Violation::Unresolved(line)
                        if line.contains(&format!("{MISSPELT}x"))))
        },
        // Clippy's own warning that the path names nothing is caught.
        needs: |found| matches!(found.violation, Violation::Unresolved(_)),
    },
    Case {
        // Clippy reads only the nearest configuration, so a crate's own
        // file replaces the workspace's for that crate.
        name: "a crate's own clippy.toml that leaves out a ban",
        place: Place::Crate,
        plant: |text| without(text, LEFT_OUT),
        expect: |found| found.file == CRATE_FILE && names(&found.violation, LEFT_OUT),
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == LEFT_OUT),
    },
];

/// `text` without the lines that name `path` as an entry.
fn without(text: &str, path: &str) -> String {
    text.lines()
        .filter(|line| !line.contains(&format!("\"{path}\"")))
        .flat_map(|line| [line, "\n"])
        .collect()
}

/// Whether `found` is a missing or silent ban on `path`.
fn names(found: &Violation, path: &str) -> bool {
    matches!(found, Violation::NotListed { path: named } | Violation::NotFired { path: named, .. }
        if *named == path)
}

/// Writes a workspace under `dir` with `root` as its `clippy.toml` and one
/// member crate, with `own` as that crate's `clippy.toml` if given, and
/// checks it.
fn check_planted(
    dir: &Path,
    root: &str,
    own: Option<&str>,
    toolchain_toml: &str,
) -> Result<Vec<Found>, Error> {
    let tree = dir.join("tree");
    write_file(&tree.join(ROOT_FILE), root)?;
    write_file(&tree.join(CRATE_DIR).join("Cargo.toml"), "")?;
    if let Some(own) = own {
        write_file(&tree.join(CRATE_FILE), own)?;
    }
    let members: Vec<PathBuf> = vec![tree.join(CRATE_DIR)];
    check_tree(&tree, &members, toolchain_toml, &dir.join("lint"))
}

/// Runs the clean configuration and every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    target_dir: &Path,
    clippy_toml: &str,
    toolchain_toml: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(&dir, clippy_toml, toolchain_toml, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_in(
    dir: &Path,
    clippy_toml: &str,
    toolchain_toml: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report check-bans self-test"));
    let clean = "the repository's clippy.toml";
    let found = check_planted(&dir.join("clean"), clippy_toml, None, toolchain_toml)?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    for (index, case) in CASES.iter().enumerate() {
        let planted = (case.plant)(clippy_toml);
        if planted == clippy_toml {
            return Err(failure(case.name, &[]));
        }
        let case_dir = dir.join(format!("case-{index}"));
        let found = match case.place {
            Place::Root => check_planted(&case_dir, &planted, None, toolchain_toml)?,
            Place::Crate => check_planted(&case_dir, clippy_toml, Some(&planted), toolchain_toml)?,
        };
        match found.first() {
            Some(_) if found.iter().all(case.expect) && found.iter().any(case.needs) => {
                for violation in &found {
                    report(format!("caught: {}: {violation}", case.name))?;
                }
            }
            _ => return Err(failure(case.name, &found)),
        }
    }
    report(format!(
        "check-bans --self-test: the repository's bans passed and {} planted cases failed as \
         planted",
        CASES.len()
    ))
}

fn failure(case: &'static str, found: &[Found]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
