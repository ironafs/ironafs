// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the bans' configuration and clippy's output on the
//! fixture.

use super::{BANS, Violation, fired_violations, listed, planted_source};

/// Clippy's short-format warning for a ban fired on line `line`.
fn warning(line: usize, path: &str) -> String {
    format!("src/lib.rs:{line}:9: warning: use of a disallowed method `{path}`\n")
}

/// The output of a fixture run in which every ban fired.
fn all_fired() -> String {
    BANS.iter()
        .enumerate()
        .map(|(index, ban)| warning(index + 1, ban.path))
        .collect()
}

#[test]
fn listed_finds_a_path_entry() {
    let text =
        "disallowed-methods = [\n  { path = \"std::time::Instant::now\", reason = \"r\" },\n]\n";
    assert!(listed(text, "std::time::Instant::now"));
}

#[test]
fn listed_needs_the_exact_path() {
    let text =
        "disallowed-methods = [\n  { path = \"std::time::Instant::nowx\", reason = \"r\" },\n]\n";
    assert!(!listed(text, "std::time::Instant::now"));
    assert!(!listed(text, "std::time::Instant::no"));
}

#[test]
fn listed_ignores_a_path_named_only_in_a_comment_or_reason() {
    let text = "# path = \"std::time::Instant::now\"\n\
                disallowed-methods = [\n  { path = \"a::b\", reason = \"not std::time::Instant::now\" },\n]\n";
    assert!(!listed(text, "std::time::Instant::now"));
}

#[test]
fn planted_source_puts_each_ban_on_its_own_line() {
    let source = planted_source();
    let lines: Vec<&str> = source.lines().collect();
    assert_eq!(lines.len(), BANS.len());
    for (line, ban) in lines.iter().zip(BANS) {
        assert_eq!(*line, ban.planted);
    }
}

#[test]
fn fired_violations_accepts_every_ban_firing() {
    assert_eq!(fired_violations(&all_fired()), []);
}

#[test]
fn fired_violations_reports_a_planted_line_with_no_warning() {
    let output: String = all_fired()
        .lines()
        .filter(|line| !line.starts_with("src/lib.rs:3:"))
        .flat_map(|line| [line, "\n"])
        .collect();
    let ban = &BANS[2];
    assert_eq!(
        fired_violations(&output),
        [Violation::NotFired {
            path: ban.path,
            planted: ban.planted
        }]
    );
}

#[test]
fn fired_violations_needs_the_warning_for_the_line_own_path() {
    // A warning on line 2 for another ban's path does not count for line 2.
    let output = all_fired().replace(&warning(2, BANS[1].path), &warning(2, BANS[3].path));
    assert_ne!(BANS[1].path, BANS[3].path);
    let ban = &BANS[1];
    assert_eq!(
        fired_violations(&output),
        [Violation::NotFired {
            path: ban.path,
            planted: ban.planted
        }]
    );
}

#[test]
fn fired_violations_reports_an_unresolved_path() {
    let line = "/x/clippy.toml:5:12: warning: `std::time::Instant::nowx` does not refer to a \
                reachable function";
    let output = format!("{line}\n{}", all_fired());
    assert_eq!(
        fired_violations(&output),
        [Violation::Unresolved(line.to_owned())]
    );
}

#[test]
fn violations_say_what_failed() {
    assert_eq!(
        Violation::NotListed {
            path: "std::time::Instant::now"
        }
        .to_string(),
        "clippy.toml does not ban std::time::Instant::now (coding standards rule 7)"
    );
    assert_eq!(
        Violation::NotFired {
            path: "a::b",
            planted: "pub fn f() { a::b() }"
        }
        .to_string(),
        "the ban on a::b did not fire on `pub fn f() { a::b() }`: it is missing, misspelt or \
         no longer resolves (coding standards rule 7)"
    );
    assert_eq!(
        Violation::Unresolved("clippy.toml:1:1: warning: x".to_owned()).to_string(),
        "clippy cannot resolve a ban, so it bans nothing: clippy.toml:1:1: warning: x"
    );
    assert_eq!(
        Violation::Clippy("error: oops".to_owned()).to_string(),
        "clippy did not finish on the planted calls: error: oops"
    );
}
