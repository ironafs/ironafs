// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the table assignment, the export checks and the report.

use super::super::exclusions::parse_list;
use super::super::export::{Counts, Export, FileSummary, FunctionRecord};
use super::{Input, Library, TABLES, Table, Tables, check};

const ROOT: &str = "/w";
const TARGET: &str = "x86_64-unknown-linux-gnu";

const CRATES: &[(&str, Table)] = &[
    ("ironafs-xdr", Table::One),
    ("ironafs-client", Table::Three),
    ("xtask", Table::Four),
];

fn tables() -> Tables<'static> {
    Tables {
        crates: CRATES,
        generated: &["crates/ironafs-xdr/src/generated/**"],
        not_measured: &[],
    }
}

fn libraries() -> Vec<Library> {
    ["ironafs-xdr", "ironafs-client", "xtask"]
        .iter()
        .map(|name| Library {
            name: (*name).to_owned(),
            dir: format!("crates/{name}/src"),
        })
        .collect()
}

fn summary(relative: &str, lines: (u64, u64)) -> FileSummary {
    FileSummary {
        path: format!("{ROOT}/{relative}"),
        lines: Counts {
            count: lines.0,
            covered: lines.1,
        },
        regions: Counts {
            count: 4,
            covered: 3,
        },
        functions: Counts {
            count: 2,
            covered: 1,
        },
    }
}

fn record(relative: &str, line: u64, executed: bool) -> FunctionRecord {
    FunctionRecord {
        file: format!("{ROOT}/{relative}"),
        line,
        column: 1,
        executed,
    }
}

fn clean_export() -> Export {
    Export {
        tool_version: "0.9.1".to_owned(),
        files: vec![
            summary("crates/ironafs-xdr/src/lib.rs", (10, 7)),
            summary("crates/ironafs-xdr/src/decode.rs", (5, 5)),
            summary("crates/ironafs-xdr/src/generated/fs.rs", (20, 2)),
            summary("crates/ironafs-client/src/lib.rs", (8, 8)),
            summary("crates/xtask/src/lib.rs", (30, 10)),
            summary("crates/xtask/src/bin/xtask/main.rs", (3, 3)),
        ],
        functions: vec![
            record("crates/ironafs-xdr/src/lib.rs", 3, true),
            record("crates/ironafs-xdr/src/lib.rs", 9, false),
            record("crates/ironafs-xdr/src/lib.rs", 20, false),
            record("crates/ironafs-xdr/src/lib.rs", 20, true),
            record("crates/ironafs-xdr/src/lib.rs", 9, false),
            record("crates/xtask/src/lib.rs", 4, false),
        ],
    }
}

fn run(
    export: &Export,
    libraries: &[Library],
    tables: &Tables<'_>,
    exclusions: &str,
) -> Result<String, Vec<String>> {
    let (exclusions, violations) = parse_list(exclusions);
    assert_eq!(violations, Vec::<String>::new());
    check(&Input {
        root: ROOT,
        target: TARGET,
        libraries,
        exclusions: &exclusions,
        export,
        tables,
    })
    .map(|report| report.render())
}

fn violations(
    export: &Export,
    libraries: &[Library],
    tables: &Tables<'_>,
    exclusions: &str,
) -> Vec<String> {
    run(export, libraries, tables, exclusions).expect_err("the check passed")
}

#[test]
fn clean_export_renders_four_tables_with_targets_and_no_branch_figure() {
    let report = run(&clean_export(), &libraries(), &tables(), "").unwrap();

    for heading in ["## Table 1", "## Table 2", "## Table 3", "## Table 4"] {
        assert!(report.contains(heading), "{heading} missing:\n{report}");
    }
    assert!(report.contains(&format!(
        "| ironafs-xdr | {TARGET} | 12/15 (3 uncovered) | 6/8 (2 uncovered) | 2/4 (2 uncovered) | not available |"
    )), "{report}");
    assert!(report.contains(&format!(
        "| ironafs-xdr | {TARGET} | 2/20 (18 uncovered) | 3/4 (1 uncovered) | 1/2 (1 uncovered) | not available |"
    )), "generated row:\n{report}");
    assert!(
        report.contains(&format!("| ironafs-client | {TARGET} | 8/8 (0 uncovered)")),
        "{report}"
    );
    assert!(
        report.contains(&format!("| xtask | {TARGET} | 10/30 (20 uncovered)")),
        "{report}"
    );
    assert!(
        report.contains("Branch coverage: not available"),
        "{report}"
    );
    assert!(report.contains("Doc-tests: not measured"), "{report}");
    assert!(report.contains("cargo-llvm-cov 0.9.1"), "{report}");
    assert!(
        !report.contains("main.rs"),
        "binary code reported:\n{report}"
    );
    for line in report
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| Crate"))
    {
        assert!(line.contains(TARGET), "figure without its target: {line}");
        assert!(!line.contains('%'), "percentage in {line}");
    }
}

#[test]
fn uncovered_functions_are_listed_once_per_source_function_for_table_one_only() {
    let report = run(&clean_export(), &libraries(), &tables(), "").unwrap();

    assert_eq!(
        report.matches("crates/ironafs-xdr/src/lib.rs:9:1").count(),
        1,
        "{report}"
    );
    assert!(
        !report.contains("crates/ironafs-xdr/src/lib.rs:20:1"),
        "an instantiation ran:\n{report}"
    );
    assert!(
        !report.contains("crates/ironafs-xdr/src/lib.rs:3:1"),
        "{report}"
    );
    assert!(
        !report.contains("crates/xtask/src/lib.rs:4:1"),
        "table 4 listed:\n{report}"
    );
}

#[test]
fn a_test_file_in_the_export_fails() {
    for relative in [
        "crates/ironafs-xdr/src/decode/tests.rs",
        "crates/ironafs-xdr/tests/it/main.rs",
    ] {
        let mut export = clean_export();
        export.files.push(summary(relative, (4, 4)));

        let found = violations(&export, &libraries(), &tables(), "");

        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("test file") && found[0].contains(relative),
            "{found:?}"
        );
    }
}

#[test]
fn an_excluded_file_in_the_export_fails() {
    let found = violations(
        &clean_export(),
        &libraries(),
        &tables(),
        "crates/ironafs-xdr/src/decode.rs # planted\n",
    );

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("excluded") && found[0].contains("crates/ironafs-xdr/src/decode.rs"),
        "{found:?}"
    );
}

#[test]
fn a_missing_expected_crate_fails() {
    let mut export = clean_export();
    export
        .files
        .retain(|file| !file.path.contains("ironafs-client"));

    let found = violations(&export, &libraries(), &tables(), "");

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("ironafs-client") && found[0].contains("no file"),
        "{found:?}"
    );
}

#[test]
fn a_crate_not_built_on_the_target_is_not_measured() {
    let mut export = clean_export();
    export
        .files
        .retain(|file| !file.path.contains("ironafs-client"));
    let tables = Tables {
        not_measured: &[("ironafs-client", TARGET)],
        ..tables()
    };

    let report = run(&export, &libraries(), &tables, "").unwrap();

    assert!(report.contains(&format!(
        "| ironafs-client | {TARGET} | not measured | not measured | not measured | not available |"
    )), "{report}");
}

#[test]
fn a_crate_in_no_table_or_two_fails() {
    let mut libraries = libraries();
    libraries.push(Library {
        name: "ironafs-new".to_owned(),
        dir: "crates/ironafs-new/src".to_owned(),
    });
    let found = violations(&clean_export(), &libraries, &tables(), "");
    assert!(
        found
            .iter()
            .any(|v| v.contains("ironafs-new") && v.contains("no coverage table")),
        "{found:?}"
    );

    let twice = [
        ("ironafs-xdr", Table::One),
        ("ironafs-client", Table::Three),
        ("xtask", Table::Four),
        ("xtask", Table::One),
    ];
    let found = violations(
        &clean_export(),
        &self::libraries(),
        &Tables {
            crates: &twice,
            ..tables()
        },
        "",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("xtask") && found[0].contains("2 coverage tables"),
        "{found:?}"
    );
}

#[test]
fn an_export_from_another_tool_version_fails() {
    let mut export = clean_export();
    export.tool_version = "0.9.2".to_owned();

    let found = violations(&export, &libraries(), &tables(), "");

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("0.9.2") && found[0].contains("0.9.1"),
        "{found:?}"
    );
}

#[test]
fn every_architecture_crate_is_in_exactly_one_table() {
    for (name, _) in crate::deps::CRATES {
        let tables = TABLES
            .crates
            .iter()
            .filter(|(entry, _)| entry == name)
            .count();
        assert_eq!(tables, 1, "{name} is in {tables} coverage tables");
    }
    for (name, _) in TABLES.crates {
        assert!(
            crate::deps::CRATES.iter().any(|(known, _)| known == name),
            "{name} is not an architecture crate"
        );
    }
}

#[test]
fn table_one_is_the_specifications_list() {
    let mut one: Vec<&str> = TABLES
        .crates
        .iter()
        .filter(|(_, table)| *table == Table::One)
        .map(|(name, _)| *name)
        .collect();
    one.sort_unstable();
    assert_eq!(
        one,
        [
            "ironafs-auth",
            "ironafs-cache",
            "ironafs-config",
            "ironafs-events",
            "ironafs-ipc",
            "ironafs-proto",
            "ironafs-rx",
            "ironafs-rxgk",
            "ironafs-rxkad",
            "ironafs-vfs",
            "ironafs-xdr",
        ]
    );
}
