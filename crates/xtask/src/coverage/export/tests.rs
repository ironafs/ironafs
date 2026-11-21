// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the reader of the tool's JSON export.

use super::{Counts, ExportError, parse};

fn counts(count: u64, covered: u64) -> String {
    format!(r#"{{"count": {count}, "covered": {covered}, "percent": 0.0}}"#)
}

fn file(path: &str, lines: (u64, u64)) -> String {
    format!(
        r#"{{"filename": "{path}", "segments": [], "branches": [], "expansions": [],
            "summary": {{"branches": {b}, "functions": {f}, "instantiations": {f},
                         "lines": {l}, "regions": {r}}}}}"#,
        b = counts(0, 0),
        f = counts(2, 1),
        l = counts(lines.0, lines.1),
        r = counts(5, 4),
    )
}

fn function(name: &str, path: &str, line: u64, column: u64, count: u64) -> String {
    format!(
        r#"{{"branches": [], "count": {count}, "filenames": ["{path}"], "name": "{name}",
            "regions": [[{line}, {column}, {end}, 2, {count}, 0, 0, 0]]}}"#,
        end = line + 3,
    )
}

fn export(files: &[String], functions: Option<&[String]>, version: &str) -> String {
    let functions = functions.map_or_else(String::new, |list| {
        format!(r#", "functions": [{}]"#, list.join(","))
    });
    format!(
        r#"{{"data": [{{"files": [{}]{functions}, "totals": {{}}}}],
            "type": "llvm.coverage.json.export", "version": "3.1.0",
            "cargo_llvm_cov": {{"version": "{version}", "manifest_path": "/w/Cargo.toml"}}}}"#,
        files.join(","),
    )
}

#[test]
fn parse_reads_files_functions_and_tool_version() {
    let text = export(
        &[file("/w/crates/a/src/lib.rs", (10, 7))],
        Some(&[function("_RNvCs1_1a1f", "/w/crates/a/src/lib.rs", 3, 1, 4)]),
        "0.9.1",
    );

    let parsed = parse(&text).unwrap();

    assert_eq!(parsed.tool_version, "0.9.1");
    assert_eq!(parsed.files.len(), 1);
    assert_eq!(parsed.files[0].path, "/w/crates/a/src/lib.rs");
    assert_eq!(
        parsed.files[0].lines,
        Counts {
            count: 10,
            covered: 7
        }
    );
    assert_eq!(
        parsed.files[0].regions,
        Counts {
            count: 5,
            covered: 4
        }
    );
    assert_eq!(
        parsed.files[0].functions,
        Counts {
            count: 2,
            covered: 1
        }
    );
    assert_eq!(parsed.functions.len(), 1);
    let found = &parsed.functions[0];
    assert_eq!(
        (
            found.file.as_str(),
            found.line,
            found.column,
            found.executed
        ),
        ("/w/crates/a/src/lib.rs", 3, 1, true)
    );
}

#[test]
fn parse_marks_an_unexecuted_function() {
    let text = export(
        &[file("/w/a.rs", (1, 0))],
        Some(&[function("f", "/w/a.rs", 3, 1, 0)]),
        "0.9.1",
    );

    assert!(!parse(&text).unwrap().functions[0].executed);
}

#[test]
fn parse_reports_an_empty_export() {
    assert_eq!(parse("").unwrap_err(), ExportError::Empty);
    assert_eq!(parse("  \n").unwrap_err(), ExportError::Empty);
    assert_eq!(
        parse(&export(&[], Some(&[]), "0.9.1")).unwrap_err(),
        ExportError::Empty
    );
}

#[test]
fn parse_reports_a_malformed_export() {
    let good_file = file("/w/a.rs", (1, 1));
    let cases = [
        ("not JSON", "{".to_owned()),
        (
            "summary only",
            export(std::slice::from_ref(&good_file), None, "0.9.1"),
        ),
        (
            "wrong type",
            export(std::slice::from_ref(&good_file), Some(&[]), "0.9.1")
                .replace("llvm.coverage.json.export", "something.else"),
        ),
        (
            "no tool version",
            export(std::slice::from_ref(&good_file), Some(&[]), "0.9.1").replace(
                r#""cargo_llvm_cov": {"version": "0.9.1", "manifest_path": "/w/Cargo.toml"}"#,
                r#""other": 1"#,
            ),
        ),
        (
            "two data entries",
            export(std::slice::from_ref(&good_file), Some(&[]), "0.9.1").replace(
                r#""data": [{"#,
                r#""data": [{"files": [], "totals": {}}, {"#,
            ),
        ),
        (
            "negative count",
            export(
                &[good_file.replace(r#""count": 1,"#, r#""count": -1,"#)],
                Some(&[]),
                "0.9.1",
            ),
        ),
        (
            "covered above count",
            export(&[file("/w/a.rs", (1, 2))], Some(&[]), "0.9.1"),
        ),
        (
            "function without regions",
            export(
                std::slice::from_ref(&good_file),
                Some(&[
                    r#"{"count": 1, "filenames": ["/w/a.rs"], "name": "f", "regions": []}"#
                        .to_owned(),
                ]),
                "0.9.1",
            ),
        ),
        (
            "short region",
            export(
                std::slice::from_ref(&good_file),
                Some(&[
                    r#"{"count": 1, "filenames": ["/w/a.rs"], "name": "f", "regions": [[1, 2]]}"#
                        .to_owned(),
                ]),
                "0.9.1",
            ),
        ),
    ];
    for (name, text) in cases {
        assert!(
            matches!(parse(&text), Err(ExportError::Malformed(_))),
            "{name}: {:?}",
            parse(&text).map(|export| export.files.len())
        );
    }
}

#[test]
fn parse_takes_the_first_region_in_the_functions_own_file() {
    let text = export(
        &[file("/w/a.rs", (1, 1))],
        Some(&[
            r#"{"count": 2, "filenames": ["/w/a.rs", "/w/macro.rs"], "name": "f",
                  "regions": [[40, 9, 41, 1, 2, 1, 0, 0], [7, 5, 9, 2, 2, 0, 0, 0]]}"#
                .to_owned(),
        ]),
        "0.9.1",
    );

    let found = &parse(&text).unwrap().functions[0];

    assert_eq!((found.line, found.column), (7, 5));
}

#[test]
fn parse_takes_the_earliest_start_of_the_functions_own_file() {
    let text = export(
        &[file("/w/a.rs", (1, 1))],
        Some(&[r#"{"count": 2, "filenames": ["/w/a.rs"], "name": "f",
                  "regions": [[9, 1, 9, 8, 2, 0, 0, 0], [7, 5, 9, 2, 2, 0, 0, 0], [7, 2, 7, 4, 2, 0, 0, 0]]}"#
            .to_owned()]),
        "0.9.1",
    );

    let found = &parse(&text).unwrap().functions[0];

    assert_eq!((found.line, found.column), (7, 2));
}
