// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The standard-library paths a pure crate may not name (ADR 0004, C3,
//! point 1, and C4, point 1). The standard library is not a crate edge, so
//! the graph alone cannot show a pure crate opening a socket.

use std::path::{Path, PathBuf};

use super::Kind;
use super::Violation;
use crate::Error;
use crate::metadata::Workspace;
use crate::rust_source;

#[cfg(test)]
mod tests;

const CLOCK: &str = "clocks come only through the Clock interface of ironafs-time";

/// Paths forbidden with everything below them.
const FORBIDDEN_TREES: &[(&str, &str)] = &[
    (
        "std::net",
        "sockets and name lookup belong in a driver; address values are in core::net",
    ),
    ("std::fs", "files belong in a driver"),
    (
        "std::thread",
        "threads belong in ironafs-client or a driver",
    ),
    ("std::time::Instant", CLOCK),
    ("std::time::SystemTime", CLOCK),
    ("std::time::UNIX_EPOCH", CLOCK),
];

/// Paths forbidden as written, because they would hide the ones above.
const FORBIDDEN_EXACT: &[(&str, &str)] = &[
    (
        "std",
        "renaming the standard library hides the paths this check looks for",
    ),
    (
        "std::*",
        "a glob of the standard library hides the paths this check looks for",
    ),
    (
        "std::time",
        "importing the module hides its clocks from this check; name core::time::Duration",
    ),
    (
        "std::time::*",
        "a glob of std::time brings in its clocks; name core::time::Duration",
    ),
];

/// Why a pure crate may not name `path`, if it may not.
pub(super) fn forbidden(path: &str) -> Option<&'static str> {
    let exact = FORBIDDEN_EXACT
        .iter()
        .find_map(|(forbidden, reason)| (*forbidden == path).then_some(*reason));
    exact.or_else(|| {
        FORBIDDEN_TREES.iter().find_map(|(tree, reason)| {
            let below = path
                .strip_prefix(tree)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"));
            below.then_some(*reason)
        })
    })
}

/// Reads the library source of every pure crate and reports each forbidden
/// standard-library path. Unit-test files (`tests.rs` and anything under a
/// `tests` directory, coding standards rule 15) are left out: they never
/// reach a shipped build. Returns the violations and the files read.
///
/// # Errors
///
/// Returns an error if a source directory cannot be listed or a file cannot
/// be read.
pub(super) fn scan(workspace: &Workspace) -> Result<(Vec<Violation>, Vec<PathBuf>), Error> {
    let mut violations = Vec::new();
    let mut scanned = Vec::new();
    for member in &workspace.members {
        if super::kind_of(&member.name) != Some(Kind::Pure) {
            continue;
        }
        let Some(lib_dir) = &member.lib_dir else {
            violations.push(Violation::NoLibrary(member.name.clone()));
            continue;
        };
        let mut files = Vec::new();
        rust_source::files(
            lib_dir,
            &|name| name == "tests" || name == "tests.rs",
            &mut files,
        )?;
        if files.is_empty() {
            violations.push(Violation::NoSource(member.name.clone()));
        }
        for file in files {
            let shown = file
                .strip_prefix(&workspace.root)
                .unwrap_or(&file)
                .to_path_buf();
            check_file(&file, &shown, &mut violations)?;
            scanned.push(shown);
        }
    }
    Ok((violations, scanned))
}

fn check_file(file: &Path, shown: &Path, violations: &mut Vec<Violation>) -> Result<(), Error> {
    match rust_source::read(file)? {
        Ok(tokens) => {
            for found in rust_source::std_paths(&tokens) {
                if let Some(reason) = forbidden(&found.path) {
                    violations.push(Violation::ForbiddenStdPath {
                        file: shown.to_path_buf(),
                        line: found.line,
                        path: found.path,
                        reason,
                    });
                }
            }
        }
        Err(error) => violations.push(Violation::Unreadable {
            file: shown.to_path_buf(),
            error,
        }),
    }
    Ok(())
}
