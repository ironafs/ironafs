// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fuzz crate's dependency policy: `fuzz/deny.toml` is the root
//! `deny.toml` with only the named entries changed (coding standards rules
//! 20 and 21), so the fuzzing engine's NCSA licence exception stays scoped
//! to that one crate and never reaches the root policy.

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::metadata;

#[cfg(test)]
mod tests;

const CHECK: &str = "check-fuzz-deny";

/// The root policy and the fuzz crate's, relative to the workspace root.
const ROOT_FILE: &str = "deny.toml";
const FUZZ_FILE: &str = "fuzz/deny.toml";

/// The licence only the fuzzing engine's binding may carry.
const ENGINE_LICENCE: &str = "NCSA";

/// One entry `fuzz/deny.toml` changes: the root's line, and the text that
/// stands in its place in the fuzz crate's file.
#[derive(Debug)]
pub(crate) struct Named {
    pub(crate) root: &'static str,
    pub(crate) fuzz: &'static str,
}

/// The entries the fuzz crate's policy adds, each an owner decision taken
/// with the dependency it names (`add-dependency`, item 9).
pub(crate) const NAMED: &[Named] = &[
    Named {
        root: "exceptions = []\n",
        fuzz: "exceptions = [\n  \
               # The fuzzing engine's binding bundles LLVM's libFuzzer sources (owner\n  \
               # decision, coding standards rule 21; this file only, never the root).\n  \
               { crate = \"libfuzzer-sys@0.4.13\", allow = [\"NCSA\"] },\n\
               ]\n",
    },
    Named {
        root: "allow-build-scripts = []\n",
        fuzz: "allow-build-scripts = [\n  \
               # Compiles the bundled libFuzzer C++ sources with the cc crate.\n  \
               { crate = \"libfuzzer-sys@0.4.13\" },\n  \
               # Probes the compiler version for cfg flags; reached through cc and\n  \
               # jobserver.\n  \
               { crate = \"libc@0.2.191\" },\n  \
               # In the lock file through jobserver on Windows only, where fuzzing never\n  \
               # runs; probes the compiler for cfg flags.\n  \
               { crate = \"getrandom@0.4.3\" },\n\
               ]\n",
    },
];

/// One way the two policy files disagree with the rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// A file could not be read.
    Unreadable { file: PathBuf, error: String },
    /// A named entry's root line is not in the root file exactly once.
    Anchor { line: &'static str, count: usize },
    /// The root policy names the engine's licence.
    EngineLicenceInRoot { line: usize },
    /// The fuzz file differs from the root with the named entries applied,
    /// first at this line of the fuzz file.
    Drift {
        line: usize,
        found: Option<String>,
        expected: Option<String>,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { file, error } => {
                write!(formatter, "{}: cannot read: {error}", file.display())
            }
            Self::Anchor { line, count } => write!(
                formatter,
                "{ROOT_FILE}: the line {:?} that a named entry replaces appears {count} times, \
                 not once: update NAMED in crates/xtask/src/fuzz_deny.rs",
                line.trim_end()
            ),
            Self::EngineLicenceInRoot { line } => write!(
                formatter,
                "{ROOT_FILE}:{line}: names {ENGINE_LICENCE}, which only the fuzz crate's \
                 policy may allow, for the fuzzing engine's binding alone"
            ),
            Self::Drift {
                line,
                found,
                expected,
            } => write!(
                formatter,
                "{FUZZ_FILE}:{line}: differs from {ROOT_FILE} by more than the named entries: \
                 found {}, expected {}",
                shown(found.as_deref()),
                shown(expected.as_deref())
            ),
        }
    }
}

fn shown(line: Option<&str>) -> String {
    line.map_or_else(
        || "the end of the file".to_owned(),
        |line| format!("{line:?}"),
    )
}

/// Compares the fuzz crate's policy with the root's, the named entries
/// applied, and returns every violation.
pub(crate) fn compare(root: &str, fuzz: &str, named: &[Named]) -> Vec<Violation> {
    let root = root.replace("\r\n", "\n");
    let fuzz = fuzz.replace("\r\n", "\n");
    let mut violations: Vec<Violation> = root
        .lines()
        .zip(1..)
        .filter(|(text, _)| text.contains(ENGINE_LICENCE))
        .map(|(_, line)| Violation::EngineLicenceInRoot { line })
        .collect();
    let mut expected = root.clone();
    for entry in named {
        // Whole lines only: the root line starts the file or follows a
        // newline, and ends with its own.
        let count = format!("\n{root}")
            .matches(&format!("\n{}", entry.root))
            .count();
        if count == 1 {
            expected = format!("\n{expected}")
                .replacen(
                    &format!("\n{}", entry.root),
                    &format!("\n{}", entry.fuzz),
                    1,
                )
                .split_off(1);
        } else {
            violations.push(Violation::Anchor {
                line: entry.root,
                count,
            });
        }
    }
    if !violations.is_empty() {
        return violations;
    }
    let mut found_lines = fuzz.lines();
    let mut expected_lines = expected.lines();
    for line in 1.. {
        match (found_lines.next(), expected_lines.next()) {
            (None, None) => break,
            (found, expected) if found != expected => {
                violations.push(Violation::Drift {
                    line,
                    found: found.map(str::to_owned),
                    expected: expected.map(str::to_owned),
                });
                break;
            }
            _ => {}
        }
    }
    if violations.is_empty() && fuzz.ends_with('\n') != expected.ends_with('\n') {
        violations.push(Violation::Drift {
            line: fuzz.lines().count(),
            found: Some("a different final line ending".to_owned()),
            expected: None,
        });
    }
    violations
}

/// Reads one policy file, recording why it cannot be read.
fn read(root: &Path, file: &str, violations: &mut Vec<Violation>) -> Option<String> {
    let path = root.join(file);
    match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) => {
            violations.push(Violation::Unreadable {
                file: PathBuf::from(file),
                error: error.to_string(),
            });
            None
        }
    }
}

/// Reads both policy files under `root` and compares them.
fn check(root: &Path, named: &[Named]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let root_text = read(root, ROOT_FILE, &mut violations);
    let fuzz_text = read(root, FUZZ_FILE, &mut violations);
    if let (Some(root_text), Some(fuzz_text)) = (root_text, fuzz_text) {
        violations.extend(compare(&root_text, &fuzz_text, named));
    }
    violations
}

/// Checks this repository's two policy files, or with `self_test` proves
/// on planted copies of them that each drift is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    if self_test {
        return self_test::run(&workspace.root, &workspace.target_dir, output);
    }
    let violations = check(&workspace.root, NAMED);
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    writeln!(
        output,
        "{CHECK}: {FUZZ_FILE} is {ROOT_FILE} with {} named entries changed, and {ROOT_FILE} \
         does not name {ENGINE_LICENCE}",
        NAMED.len()
    )
    .map_err(Error::io("report check-fuzz-deny"))
}
