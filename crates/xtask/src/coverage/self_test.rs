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
const LIBRARIES: &[&str] = &["ironafs-xdr", "ironafs-client", "xtask"];

fn libraries(names: &[&str]) -> Vec<Library> {
    names
        .iter()
        .map(|name| Library {
            name: (*name).to_owned(),
            dir: format!("crates/{name}/src"),
        })
        .collect()
}

fn counts(count: u64, covered: u64) -> String {
    format!(r#"{{"count": {count}, "covered": {covered}, "percent": 0.0}}"#)
}

/// A file summary: 10 lines (8 covered) and 2 functions (1 run), or nothing
/// instrumented.
fn file(relative: &str, instrumented: bool) -> String {
    let (lines, functions) = if instrumented {
        (counts(10, 8), counts(2, 1))
    } else {
        (counts(0, 0), counts(0, 0))
    };
    format!(
        r#"{{"filename": "{ROOT}/{relative}", "segments": [], "branches": [], "expansions": [],
            "summary": {{"branches": {none}, "functions": {functions}, "instantiations": {functions},
                         "lines": {lines}, "regions": {r}}}}}"#,
        none = counts(0, 0),
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

/// How a planted export departs from a clean one.
#[derive(Default)]
struct Plant {
    /// Leave out every function record.
    no_records: bool,
    /// A file with function records but no summary.
    record_without_file: Option<&'static str>,
    /// A file with nothing instrumented.
    uninstrumented: Option<&'static str>,
}

/// An export of `files` (relative paths), written by `version`. Each
/// instrumented file has one run and one unrun function record, matching
/// its summary.
fn export_planted(files: &[&str], version: &str, plant: &Plant) -> String {
    let mut records = Vec::new();
    let summaries: Vec<String> = files
        .iter()
        .map(|path| {
            let instrumented = plant.uninstrumented != Some(*path);
            if instrumented {
                records.push(function(path, 3, 1));
                records.push(function(path, 9, 0));
            }
            file(path, instrumented)
        })
        .collect();
    if let Some(path) = plant.record_without_file {
        records.push(function(path, 1, 0));
    }
    if plant.no_records {
        records.clear();
    }
    format!(
        r#"{{"data": [{{"files": [{}], "functions": [{}], "totals": {{}}}}],
            "type": "llvm.coverage.json.export", "version": "3.1.0",
            "cargo_llvm_cov": {{"version": "{version}", "manifest_path": "{ROOT}/Cargo.toml"}}}}"#,
        summaries.join(", "),
        records.join(", "),
    )
}

fn export(files: &[&str], version: &str) -> String {
    export_planted(files, version, &Plant::default())
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

fn planted(plant: &Plant) -> String {
    export_planted(&CLEAN_FILES, TOOL_VERSION, plant)
}

const TWICE: &[(&str, Table)] = &[
    ("ironafs-xdr", Table::One),
    ("ironafs-client", Table::Three),
    ("xtask", Table::Four),
    ("ironafs-client", Table::Four),
];

/// A planted case: the export, the workspace's libraries, the exclusion
/// list, the tables, and the text every violation must contain.
struct Case {
    name: &'static str,
    export: String,
    libraries: &'static [&'static str],
    exclusions: &'static str,
    tables: Tables<'static>,
    expect: &'static str,
}

fn cases() -> Vec<Case> {
    let case = |name, export, expect| Case {
        name,
        export,
        libraries: LIBRARIES,
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
        case(
            "function records that disagree with the file summaries",
            planted(&Plant {
                no_records: true,
                ..Plant::default()
            }),
            "function records give 0/0 run",
        ),
        case(
            "a function record for a library file without a summary",
            planted(&Plant {
                record_without_file: Some("crates/ironafs-xdr/src/lost.rs"),
                ..Plant::default()
            }),
            "function record for crates/ironafs-xdr/src/lost.rs, which has no file summary",
        ),
        case(
            "a measured crate with no instrumented line",
            planted(&Plant {
                uninstrumented: Some("crates/ironafs-client/src/lib.rs"),
                ..Plant::default()
            }),
            "crate `ironafs-client` has no instrumented line",
        ),
        Case {
            libraries: &[],
            tables: Tables {
                crates: &[],
                ..TABLES
            },
            ..case(
                "an export with no workspace library file",
                clean(),
                "no library file of the workspace was measured",
            )
        },
        Case {
            tables: Tables {
                generated: &["crates/ironafs-xdr/src/**"],
                ..TABLES
            },
            ..case(
                "a crate with only generated files",
                clean(),
                "expected crate `ironafs-xdr` has no file in the export",
            )
        },
        Case {
            tables: Tables {
                not_measured: &[("ironafs-client", TARGET)],
                ..TABLES
            },
            ..case(
                "a crate listed as not built that has files",
                clean(),
                "crate `ironafs-client` is listed as not built",
            )
        },
        Case {
            libraries: &[
                "ironafs-xdr",
                "ironafs-client",
                "xtask",
                "ironafs-unassigned",
            ],
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
        &libraries(case.libraries),
        &exclusions,
        &case.tables,
    )
}

pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut say =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report coverage self-test"));

    let text = report(&clean(), ROOT, TARGET, &libraries(LIBRARIES), &[], &TABLES)
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
