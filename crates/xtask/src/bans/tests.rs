// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the bans' configuration and clippy's output on the
//! fixture.

use super::{
    BANS, Ban, Lint, TIME_CRATE_EXEMPT, Violation, crate_config_drift, fired_violations, listed,
    planted_source,
};

/// Clippy's short-format warning for `ban` fired on line `line`, spelt
/// here rather than through `Lint::word`, so the test does not share the
/// code it checks.
fn warning(line: usize, ban: &Ban) -> String {
    let word = match ban.lint {
        Lint::Method => "method",
        Lint::Type => "type",
    };
    format!(
        "src/lib.rs:{line}:9: warning: use of a disallowed {word} `{}`\n",
        ban.path
    )
}

/// The output of a fixture run in which every ban fired.
fn all_fired() -> String {
    BANS.iter()
        .enumerate()
        .map(|(index, ban)| warning(index + 1, ban))
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
    let output = all_fired().replace(&warning(2, &BANS[1]), &warning(2, &BANS[3]));
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
fn fired_violations_needs_the_lint_the_ban_is_listed_under() {
    // A type ban's path reported as a disallowed method does not count.
    let index = BANS.iter().position(|ban| ban.lint == Lint::Type).unwrap();
    let ban = &BANS[index];
    let method = Ban {
        lint: Lint::Method,
        ..*ban
    };
    let output = all_fired().replace(&warning(index + 1, ban), &warning(index + 1, &method));
    assert_eq!(
        fired_violations(&output),
        [Violation::NotFired {
            path: ban.path,
            planted: ban.planted
        }]
    );
}

#[test]
fn bans_cover_the_hashed_collections_their_hasher_and_subprocesses() {
    for path in [
        "std::hash::RandomState",
        "std::collections::HashMap",
        "std::collections::HashSet",
        "std::process::Command",
    ] {
        assert!(
            BANS.iter()
                .any(|ban| ban.path == path && ban.lint == Lint::Type),
            "{path}"
        );
    }
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

#[test]
fn bans_cover_the_deadline_constructors() {
    for path in [
        "ironafs_time::Deadline::boot_after",
        "ironafs_time::Deadline::awake_after",
    ] {
        assert!(
            BANS.iter()
                .any(|ban| ban.path == path && ban.lint == Lint::Method),
            "{path}"
        );
        assert!(TIME_CRATE_EXEMPT.contains(&path), "{path}");
    }
}

const ROOT: &str = "# SPDX header\n\n# Why.\ndisallowed-methods = [\n  \
                    { path = \"ironafs_time::Deadline::boot_after\", reason = \"r\" },\n  \
                    { path = \"ironafs_time::Deadline::awake_after\", reason = \"r\" },\n  \
                    { path = \"std::time::Instant::now\", reason = \"r\" },\n]\n";

#[test]
fn crate_config_matching_the_root_minus_the_constructor_bans_passes() {
    let own = "# Its own comment.\n\ndisallowed-methods = [\n  \
               { path = \"std::time::Instant::now\", reason = \"r\" },\n]\n";
    assert_eq!(crate_config_drift(ROOT, own), []);
}

#[test]
fn crate_config_missing_a_root_ban_is_drift() {
    let own = "disallowed-methods = [\n]\n";
    assert_eq!(
        crate_config_drift(ROOT, own),
        [Violation::CrateConfigDrift(
            "  { path = \"std::time::Instant::now\", reason = \"r\" },".to_owned()
        )]
    );
}

#[test]
fn crate_config_listing_a_constructor_ban_is_drift() {
    let found = crate_config_drift(ROOT, ROOT);
    assert_eq!(
        found,
        [Violation::CrateConfigDrift(
            "  { path = \"ironafs_time::Deadline::boot_after\", reason = \"r\" },".to_owned()
        )]
    );
}

#[test]
fn crate_config_drift_says_what_differs() {
    assert_eq!(
        Violation::CrateConfigDrift("x = 1".to_owned()).to_string(),
        "crates/ironafs-time/clippy.toml must be the root clippy.toml without the Deadline \
         constructor bans; first difference at: x = 1"
    );
}
