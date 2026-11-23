// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for comparing the fuzz crate's dependency policy with the root's.

use super::{NAMED, Named, Violation, compare};

const ROOT: &str = "# policy\n\
                    [licenses]\n\
                    allow = [\n  \"MIT\",\n]\n\
                    exceptions = []\n\
                    \n\
                    [bans.build]\n\
                    allow-build-scripts = []\n";

const TEST: &[Named] = &[
    Named {
        root: "exceptions = []\n",
        fuzz: "exceptions = [\n  { crate = \"engine@1.0.0\", allow = [\"NCSA\"] },\n]\n",
    },
    Named {
        root: "allow-build-scripts = []\n",
        fuzz: "allow-build-scripts = [\n  { crate = \"engine@1.0.0\" },\n]\n",
    },
];

/// The root text with every named entry applied.
fn fuzz() -> String {
    TEST.iter().fold(ROOT.to_owned(), |text, named| {
        text.replace(named.root, named.fuzz)
    })
}

#[test]
fn compare_passes_when_only_the_named_entries_differ() {
    assert_eq!(compare(ROOT, &fuzz(), TEST), []);
}

#[test]
fn compare_reports_a_fuzz_file_that_copies_the_root() {
    assert_eq!(
        compare(ROOT, ROOT, TEST),
        [Violation::Drift {
            line: 6,
            found: Some("exceptions = []".to_owned()),
            expected: Some("exceptions = [".to_owned()),
        }]
    );
}

#[test]
fn compare_reports_the_engine_licence_on_the_fuzz_global_allow_list() {
    let planted = fuzz().replace("  \"MIT\",\n", "  \"MIT\",\n  \"NCSA\",\n");
    assert_eq!(
        compare(ROOT, &planted, TEST),
        [Violation::Drift {
            line: 5,
            found: Some("  \"NCSA\",".to_owned()),
            expected: Some("]".to_owned()),
        }]
    );
}

#[test]
fn compare_reports_an_exception_widened_to_a_second_crate() {
    let planted = fuzz().replace(
        "  { crate = \"engine@1.0.0\", allow = [\"NCSA\"] },\n",
        "  { crate = \"engine@1.0.0\", allow = [\"NCSA\"] },\n  { crate = \"other@2.0.0\", allow = [\"NCSA\"] },\n",
    );
    assert_eq!(
        compare(ROOT, &planted, TEST),
        [Violation::Drift {
            line: 8,
            found: Some("  { crate = \"other@2.0.0\", allow = [\"NCSA\"] },".to_owned()),
            expected: Some("]".to_owned()),
        }]
    );
}

#[test]
fn compare_reports_a_fuzz_file_longer_or_shorter_than_expected() {
    let longer = format!("{}[extra]\n", fuzz());
    let found = compare(ROOT, &longer, TEST);
    assert_eq!(
        found,
        [Violation::Drift {
            line: 14,
            found: Some("[extra]".to_owned()),
            expected: None,
        }]
    );
    let expected = fuzz();
    let shorter = expected.trim_end_matches("]\n");
    let found = compare(ROOT, shorter, TEST);
    assert_eq!(
        found,
        [Violation::Drift {
            line: 13,
            found: None,
            expected: Some("]".to_owned()),
        }]
    );
}

#[test]
fn compare_reports_a_root_line_missing_or_repeated() {
    let missing = ROOT.replace("exceptions = []\n", "");
    assert_eq!(
        compare(&missing, &fuzz(), TEST),
        [Violation::Anchor {
            line: "exceptions = []\n",
            count: 0,
        }]
    );
    let repeated = format!("{ROOT}exceptions = []\n");
    assert_eq!(
        compare(&repeated, &fuzz(), TEST),
        [Violation::Anchor {
            line: "exceptions = []\n",
            count: 2,
        }]
    );
}

#[test]
fn compare_reports_the_engine_licence_in_the_root() {
    let root = ROOT.replace("  \"MIT\",\n", "  \"MIT\",\n  \"NCSA\",\n");
    let fuzz = fuzz().replace("  \"MIT\",\n", "  \"MIT\",\n  \"NCSA\",\n");
    assert_eq!(
        compare(&root, &fuzz, TEST),
        [Violation::EngineLicenceInRoot { line: 5 }]
    );
}

#[test]
fn compare_reads_windows_line_endings_as_unix_ones() {
    let fuzz = fuzz().replace('\n', "\r\n");
    assert_eq!(compare(ROOT, &fuzz, TEST), []);
}

#[test]
fn named_entries_scope_the_engine_licence_to_one_exact_crate() {
    let licence_lines: Vec<&str> = NAMED
        .iter()
        .flat_map(|named| named.fuzz.lines())
        .filter(|line| line.contains("NCSA"))
        .collect();
    assert_eq!(
        licence_lines,
        ["  { crate = \"libfuzzer-sys@0.4.13\", allow = [\"NCSA\"] },"]
    );
    assert!(NAMED.iter().all(|named| !named.root.contains("NCSA")));
}
