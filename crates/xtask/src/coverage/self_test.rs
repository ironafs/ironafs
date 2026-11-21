// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `coverage --self-test`: plants each case that makes an export unusable
//! (empty, malformed, summary-only, missing an expected crate, counting a
//! test file or an excluded file, written by another tool version), a crate
//! in no table or in two, and an exclusion without a reason, and proves
//! each fails through the same path as the real run. A clean export must
//! pass, with the branch column reading "not available" on every row.
//!
//! The exports are built here as JSON text in the tool's format, so the
//! reader is exercised too. The tool itself is never run.

use std::io::Write;

use super::exclusions::parse_list;
use super::tables::{Library, TABLES, Table, Tables};
use super::{CHECK, TOOL_VERSION, report};
use crate::Error;

const ROOT: &str = "/self-test/ironafs";
const CLEAN: &str = "the clean export";
const TARGET: &str = "x86_64-unknown-linux-gnu";

/// The fixture workspace: one crate from each table that has crates.
const LIBRARIES: [&str; 3] = ["ironafs-xdr", "ironafs-client", "xtask"];

fn libraries(extra: Option<&str>) -> Vec<Library> {
    LIBRARIES
        .iter()
        .copied()
        .chain(extra)
        .map(|name| Library {
            name: name.to_owned(),
            dir: format!("crates/{name}/src"),
        })
        .collect()
}

fn counts(count: u64, covered: u64) -> String {
    format!(r#"{{"count": {count}, "covered": {covered}, "percent": 0.0}}"#)
}

fn file(relative: &str) -> String {
    format!(
        r#"{{"filename": "{ROOT}/{relative}", "segments": [], "branches": [], "expansions": [],
            "summary": {{"branches": {none}, "functions": {f}, "instantiations": {f},
                         "lines": {l}, "regions": {r}}}}}"#,
        none = counts(0, 0),
        f = counts(2, 1),
        l = counts(10, 8),
        r = counts(6, 5),
    )
}

fn function(relative: &str, line: u64, count: u64) -> String {
    format!(
        r#"{{"branches": [], "count": {count}, "filenames": ["{ROOT}/{relative}"], "name": "_RNvCs0_1f",
            "regions": [[{line}, 1, {end}, 2, {count}, 0, 0, 0]]}}"#,
        end = line + 2,
    )
}

/// An export of `files` (relative paths) with one run and one unrun
/// function in the first file, written by `version`.
fn export(files: &[&str], version: &str) -> String {
    let first = files.first().copied().unwrap_or("none.rs");
    format!(
        r#"{{"data": [{{"files": [{}], "functions": [{}, {}], "totals": {{}}}}],
            "type": "llvm.coverage.json.export", "version": "3.1.0",
            "cargo_llvm_cov": {{"version": "{version}", "manifest_path": "{ROOT}/Cargo.toml"}}}}"#,
        files
            .iter()
            .map(|path| file(path))
            .collect::<Vec<_>>()
            .join(", "),
        function(first, 3, 1),
        function(first, 9, 0),
    )
}

const CLEAN_FILES: [&str; 4] = [
    "crates/ironafs-xdr/src/lib.rs",
    "crates/ironafs-xdr/src/decode.rs",
    "crates/ironafs-client/src/lib.rs",
    "crates/xtask/src/lib.rs",
];

fn clean() -> String {
    export(&CLEAN_FILES, TOOL_VERSION)
}

const TWICE: &[(&str, Table)] = &[
    ("ironafs-xdr", Table::One),
    ("ironafs-client", Table::Three),
    ("xtask", Table::Four),
    ("ironafs-client", Table::Four),
];

/// A planted case: the export, the workspace's extra crate, the exclusion
/// list, the tables, and the text every violation must contain.
struct Case {
    name: &'static str,
    export: String,
    extra: Option<&'static str>,
    exclusions: &'static str,
    tables: Tables<'static>,
    expect: &'static str,
}

fn cases() -> Vec<Case> {
    let case = |name, export, expect| Case {
        name,
        export,
        extra: None,
        exclusions: "",
        tables: TABLES,
        expect,
    };
    vec![
        case("an empty export", String::new(), "the export is empty"),
        case(
            "a malformed export",
            clean().replacen('{', "[", 1),
            "the export is malformed",
        ),
        case(
            "a summary-only export",
            clean().replace(r#""functions": ["#, r#""unused": ["#),
            "summary-only",
        ),
        case(
            "an export missing an expected crate",
            export(
                &[
                    "crates/ironafs-xdr/src/lib.rs",
                    "crates/ironafs-xdr/src/decode.rs",
                    "crates/xtask/src/lib.rs",
                ],
                TOOL_VERSION,
            ),
            "expected crate `ironafs-client` has no file in the export",
        ),
        case(
            "an export counting a planted test file",
            export(
                &[
                    "crates/ironafs-xdr/src/lib.rs",
                    "crates/ironafs-xdr/src/decode/tests.rs",
                    "crates/ironafs-client/src/lib.rs",
                    "crates/xtask/src/lib.rs",
                ],
                TOOL_VERSION,
            ),
            "test file counted as covered code: crates/ironafs-xdr/src/decode/tests.rs",
        ),
        Case {
            exclusions: "crates/ironafs-xdr/src/decode.rs # planted\n",
            ..case(
                "an excluded file in the export",
                clean(),
                "excluded file in the export: crates/ironafs-xdr/src/decode.rs",
            )
        },
        case(
            "an export from another tool version",
            export(&CLEAN_FILES, "0.9.2"),
            "not the pinned 0.9.1",
        ),
        Case {
            extra: Some("ironafs-unassigned"),
            ..case(
                "a crate in no table",
                clean(),
                "crate `ironafs-unassigned` is in no coverage table",
            )
        },
        Case {
            tables: Tables {
                crates: TWICE,
                ..TABLES
            },
            ..case(
                "a crate in two tables",
                clean(),
                "crate `ironafs-client` is in 2 coverage tables",
            )
        },
        Case {
            exclusions: "# header\ncrates/ironafs-xdr/src/decode.rs\n",
            ..case("an exclusion without a reason", clean(), "has no reason")
        },
    ]
}

/// Runs one case through the real path: the exclusion list's reader, then
/// the export's reader, checks and report.
fn outcome(case: &Case) -> Result<String, Vec<String>> {
    let (exclusions, violations) = parse_list(case.exclusions);
    if !violations.is_empty() {
        return Err(violations);
    }
    report(
        &case.export,
        ROOT,
        TARGET,
        &libraries(case.extra),
        &exclusions,
        &case.tables,
    )
}

pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut say =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report coverage self-test"));

    let text = report(&clean(), ROOT, TARGET, &libraries(None), &[], &TABLES)
        .map_err(|found| failure(CLEAN, found))?;
    let rows: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| Crate"))
        .collect();
    let branch_unavailable = rows.len() == LIBRARIES.len()
        && rows
            .iter()
            .all(|row| row.ends_with("| not available |") && row.contains(TARGET))
        && text.contains("Branch coverage: not available");
    if !branch_unavailable {
        return Err(failure(
            CLEAN,
            vec![format!("rows or branch column not as expected:\n{text}")],
        ));
    }
    say(format!(
        "passed: {CLEAN} ({} rows, branch column \"not available\")",
        rows.len()
    ))?;

    let cases = cases();
    for case in &cases {
        match outcome(case) {
            Err(found) if !found.is_empty() && found.iter().all(|v| v.contains(case.expect)) => {
                say(format!("caught: {}: {}", case.name, found.join("; ")))?;
            }
            Err(found) => return Err(failure(case.name, found)),
            Ok(_) => return Err(failure(case.name, vec!["the check passed".to_owned()])),
        }
    }
    say(format!(
        "{CHECK} --self-test: the clean export passed and {} planted cases failed as planted",
        cases.len()
    ))
}

fn failure(case: &'static str, found: Vec<String>) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found,
    }
}
