// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-bans --self-test`: runs the check on the repository's
//! `clippy.toml`, which must pass, then on copies with one ban removed and
//! one ban misspelt, and proves the check fails on each, naming that ban.

use std::io::Write;
use std::path::Path;

use super::{CHECK, Violation, check};
use crate::Error;
use crate::fixture;

/// The ban each planted case breaks.
const REMOVED: &str = "std::time::SystemTime::now";
const MISSPELT: &str = "std::time::Instant::elapsed";

/// A planted configuration, what every violation it gives must be, and
/// one violation it must give.
struct Case {
    name: &'static str,
    plant: fn(&str) -> String,
    expect: fn(&Violation) -> bool,
    needs: fn(&Violation) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a ban removed from clippy.toml",
        plant: |text| {
            text.lines()
                .filter(|line| !line.contains(&format!("\"{REMOVED}\"")))
                .flat_map(|line| [line, "\n"])
                .collect()
        },
        expect: |found| names(found, REMOVED),
        // Clippy itself stops firing, not only the listing check.
        needs: |found| matches!(found, Violation::NotFired { path, .. } if *path == REMOVED),
    },
    Case {
        name: "a ban misspelt in clippy.toml",
        plant: |text| text.replace(&format!("\"{MISSPELT}\""), &format!("\"{MISSPELT}x\"")),
        expect: |found| {
            names(found, MISSPELT)
                || matches!(found, Violation::Unresolved(line) if line.contains(&format!("{MISSPELT}x")))
        },
        // Clippy's own warning that the path names nothing is caught.
        needs: |found| matches!(found, Violation::Unresolved(_)),
    },
];

/// Whether `found` is a missing or silent ban on `path`.
fn names(found: &Violation, path: &str) -> bool {
    matches!(found, Violation::NotListed { path: named } | Violation::NotFired { path: named, .. }
        if *named == path)
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
    let found = check(clippy_toml, toolchain_toml, &dir.join("clean"))?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    for (index, case) in CASES.iter().enumerate() {
        let planted = (case.plant)(clippy_toml);
        if planted == clippy_toml {
            return Err(failure(case.name, &[]));
        }
        let found = check(&planted, toolchain_toml, &dir.join(format!("case-{index}")))?;
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

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
