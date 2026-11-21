// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading `tests/mutants-accepted.txt`.

use super::parse_accepted;

const KEY: &str = "crates/ironafs-time/src/wall.rs: replace < with <= in WallTime::from_unix";

#[test]
fn an_entry_is_a_mutant_line_and_a_reason_line() {
    let text = format!("# header\n\nmutant: {KEY}\nreason: equivalent: both reject 1e9\n");
    let (entries, violations) = parse_accepted(&text);
    assert_eq!(violations, Vec::<String>::new());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, KEY);
    assert_eq!(entries[0].reason, "equivalent: both reject 1e9");
}

#[test]
fn an_empty_list_is_valid() {
    let (entries, violations) = parse_accepted("# nothing accepted yet\n");
    assert!(entries.is_empty());
    assert_eq!(violations, [] as [std::string::String; 0]);
}

#[test]
fn a_mutant_without_a_reason_is_refused() {
    for text in [
        format!("mutant: {KEY}\n"),
        format!("mutant: {KEY}\nreason:   \n"),
        format!("mutant: {KEY}\n\nreason: late\n"),
        format!("mutant: {KEY}\nmutant: {KEY}x\nreason: one\n"),
    ] {
        let (_, violations) = parse_accepted(&text);
        assert!(
            violations.iter().any(|found| found.contains("no reason")),
            "{text:?}: {violations:?}"
        );
    }
}

#[test]
fn a_reason_without_a_mutant_is_refused() {
    let (_, violations) = parse_accepted("reason: orphan\n");
    assert!(
        violations
            .iter()
            .any(|found| found.contains("no mutant line"))
    );
}

#[test]
fn a_duplicate_entry_is_refused() {
    let text = format!("mutant: {KEY}\nreason: a\nmutant: {KEY}\nreason: b\n");
    let (_, violations) = parse_accepted(&text);
    assert!(violations.iter().any(|found| found.contains("twice")));
}

#[test]
fn a_key_with_a_line_number_or_without_a_rust_file_is_refused() {
    for key in [
        "crates/ironafs-time/src/wall.rs:10:5: replace < with <=",
        "replace < with <=",
        "README.md: replace",
    ] {
        let (_, violations) = parse_accepted(&format!("mutant: {key}\nreason: r\n"));
        assert!(
            violations
                .iter()
                .any(|found| found.contains("`file.rs: change`")),
            "{key}: {violations:?}"
        );
    }
}

#[test]
fn an_unknown_line_is_refused() {
    let (_, violations) = parse_accepted("because I said so\n");
    assert!(violations.iter().any(|found| found.contains("line 1")));
}
