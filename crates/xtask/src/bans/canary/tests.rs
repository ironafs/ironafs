// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the canary's text and for reading the lint run it is in.

use super::{depended_on, line, manifest, source, tampered, verdict};
use crate::bans::{BANS, Lint, Violation};
use crate::metadata::Package;

const CANARY: &str = "path+file:///w/crates/ironafs-bans-canary#0.0.0";
const OTHER: &str = "path+file:///w/crates/ironafs-time#0.0.0";
const FINISHED: &str = "{\"reason\":\"build-finished\",\"success\":true}";

fn members() -> Vec<String> {
    vec![CANARY.to_owned(), OTHER.to_owned()]
}

/// A compiler-artifact message for `package`'s library.
fn artifact(package: &str) -> String {
    format!(
        "{{\"reason\":\"compiler-artifact\",\"package_id\":\"{package}\",\
         \"target\":{{\"kind\":[\"lib\"],\"name\":\"x\"}},\"fresh\":true}}"
    )
}

/// A compiler-message from `package` at `file`:`line`, with `code`, of
/// `level`, saying `text`.
fn diagnostic(package: &str, file: &str, line: usize, code: &str, text: &str) -> String {
    format!(
        "{{\"reason\":\"compiler-message\",\"package_id\":\"{package}\",\
         \"target\":{{\"kind\":[\"lib\"],\"name\":\"x\"}},\"message\":{{\
         \"rendered\":\"warning: {text}\",\"code\":{{\"code\":\"{code}\",\"explanation\":null}},\
         \"level\":\"warning\",\"message\":\"{text}\",\"children\":[],\"spans\":[{{\
         \"file_name\":\"{file}\",\"line_start\":{line},\"is_primary\":true}}]}}}}"
    )
}

/// The diagnostic clippy gives for `BANS[index]` on its canary line.
fn fired(index: usize) -> String {
    let ban = &BANS[index];
    let (code, word) = match ban.lint {
        Lint::Method => ("clippy::disallowed_methods", "method"),
        Lint::Type => ("clippy::disallowed_types", "type"),
    };
    diagnostic(
        CANARY,
        "crates/ironafs-bans-canary/src/lib.rs",
        line(index),
        code,
        &format!("use of a disallowed {word} `{}`", ban.path),
    )
}

/// A whole run in which every canary line fired, with `except` left out.
fn run(except: Option<usize>) -> String {
    let mut lines = vec![artifact(OTHER), artifact(CANARY)];
    lines.extend(
        (0..BANS.len())
            .filter(|index| Some(*index) != except)
            .map(fired),
    );
    lines.push(FINISHED.to_owned());
    lines
        .iter()
        .flat_map(|line| [line.as_str(), "\n"])
        .collect()
}

#[test]
fn canary_files_in_the_tree_are_the_generated_text() {
    assert_eq!(
        include_str!("../../../../ironafs-bans-canary/src/lib.rs"),
        source()
    );
    assert_eq!(
        include_str!("../../../../ironafs-bans-canary/Cargo.toml"),
        manifest()
    );
}

#[test]
fn canary_source_plants_each_ban_on_its_line() {
    let source = source();
    let lines: Vec<&str> = source.lines().collect();
    for (index, ban) in BANS.iter().enumerate() {
        let text = lines[line(index) - 1];
        assert!(text.ends_with(ban.planted), "{index}: {text}");
    }
    assert!(source.contains("#![cfg(feature = \"planted\")]"));
}

#[test]
fn canary_source_silences_no_ban() {
    let source = source();
    for word in ["allow", "disallowed", "clippy::style", "clippy::all"] {
        assert!(
            !source.lines().any(|line| line.contains("#[")
                && line.contains(word)
                && !line.contains("default_trait_access")),
            "{word}"
        );
    }
}

#[test]
fn verdict_accepts_a_run_in_which_every_canary_line_fired() {
    assert_eq!(verdict(&run(None), CANARY, &members()), []);
}

#[test]
fn verdict_reports_a_canary_line_that_did_not_fire() {
    let found = verdict(&run(Some(3)), CANARY, &members());
    assert_eq!(
        found,
        [Violation::CanaryMissing {
            path: BANS[3].path,
            line: line(3)
        }]
    );
}

#[test]
fn verdict_needs_the_diagnostic_on_the_canary_line_and_file() {
    let index = 2;
    for moved in [
        fired(index).replace(
            &format!("\"line_start\":{}", line(index)),
            &format!("\"line_start\":{}", line(index) + 1),
        ),
        fired(index).replace(
            "ironafs-bans-canary/src/lib.rs",
            "ironafs-bans-canary/src/other.rs",
        ),
        fired(index).replace(CANARY, OTHER),
    ] {
        let messages = run(Some(index)).replace(FINISHED, &format!("{moved}\n{FINISHED}"));
        let found = verdict(&messages, CANARY, &members());
        assert!(
            found.contains(&Violation::CanaryMissing {
                path: BANS[index].path,
                line: line(index)
            }),
            "{moved}: {found:?}"
        );
    }
}

#[test]
fn verdict_reports_any_other_warning() {
    let other = diagnostic(
        OTHER,
        "crates/ironafs-time/src/lib.rs",
        3,
        "unused",
        "unused x",
    );
    let messages = run(None).replace(FINISHED, &format!("{other}\n{FINISHED}"));
    let found = verdict(&messages, CANARY, &members());
    assert!(
        matches!(found.as_slice(), [Violation::Lint(text)] if text.contains("unused x")),
        "{found:?}"
    );
}

#[test]
fn verdict_refuses_a_run_with_only_a_build_finished_record() {
    let found = verdict(&format!("{FINISHED}\n"), CANARY, &members());
    for (index, ban) in BANS.iter().enumerate() {
        assert!(
            found.contains(&Violation::CanaryMissing {
                path: ban.path,
                line: line(index)
            }),
            "{index}: {found:?}"
        );
    }
    assert!(found.contains(&Violation::Unbuilt(format!(
        "cargo reported no artifact of the member {OTHER}"
    ))));
}

#[test]
fn verdict_accepts_a_windows_path() {
    let messages = run(None).replace(
        "crates/ironafs-bans-canary/src/lib.rs",
        "crates\\\\ironafs-bans-canary\\\\src\\\\lib.rs",
    );
    assert_eq!(verdict(&messages, CANARY, &members()), []);
}

/// The canary's own files, as `tampered` reads them.
fn canary_files() -> Vec<(String, Vec<u8>)> {
    vec![
        ("Cargo.toml".to_owned(), manifest().into_bytes()),
        ("src/lib.rs".to_owned(), source().into_bytes()),
    ]
}

#[test]
fn tampered_accepts_the_generated_files() {
    assert_eq!(tampered(&canary_files()), []);
}

#[test]
fn tampered_refuses_a_silencing_attribute_an_extra_file_or_a_missing_one() {
    let mut expect = canary_files();
    expect[1].1 = source()
        .replace("#![cfg(", "#![expect(clippy::disallowed_methods)]\n#![cfg(")
        .into_bytes();
    let mut config = canary_files();
    config.push(("clippy.toml".to_owned(), Vec::new()));
    let mut build = canary_files();
    build.push(("build.rs".to_owned(), b"fn main() {}\n".to_vec()));
    let mut manifest_changed = canary_files();
    manifest_changed[0]
        .1
        .extend_from_slice(b"build = \"b.rs\"\n");
    let missing = vec![canary_files().remove(0)];
    for (case, files) in [
        ("expect", expect),
        ("clippy.toml", config),
        ("build.rs", build),
        ("manifest", manifest_changed),
        ("missing", missing),
    ] {
        let found = tampered(&files);
        assert!(
            !found.is_empty()
                && found
                    .iter()
                    .all(|violation| matches!(violation, Violation::Canary(_))),
            "{case}: {found:?}"
        );
    }
}

fn package(id: &str, dependencies: &[&str]) -> Package {
    Package {
        id: id.to_owned(),
        name: id.to_owned(),
        path: None,
        member: true,
        dependencies: dependencies.iter().map(|id| (*id).to_owned()).collect(),
        target_kinds: vec!["lib".to_owned()],
        links: None,
        features: Vec::new(),
    }
}

#[test]
fn depended_on_reports_a_package_that_depends_on_the_canary() {
    let packages = [package(CANARY, &[]), package(OTHER, &[CANARY])];
    assert_eq!(
        depended_on(&packages, CANARY),
        [Violation::Canary(format!("{OTHER} depends on the canary"))]
    );
    assert_eq!(
        depended_on(&[package(CANARY, &[]), package(OTHER, &[])], CANARY),
        []
    );
}
