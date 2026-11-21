// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the bans' configuration and clippy's output on the
//! fixture.

use std::path::{Path, PathBuf};

use std::ffi::OsStr;

use super::{
    BANS, Ban, CONF_DIR, Found, Lint, Violation, build_overrides, config_files, fired_violations,
    listed, overridden, planted_source, root_config, uncovered, unreadable,
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

/// The configurations found under `/w` for members in `members`, when
/// exactly the files in `existing` exist.
fn configs(members: &[&str], existing: &[&str]) -> Vec<PathBuf> {
    let root = Path::new("/w");
    let members: Vec<PathBuf> = members.iter().map(|member| root.join(member)).collect();
    config_files(root, &members, |path| {
        existing.iter().any(|file| root.join(file) == path)
    })
}

#[test]
fn config_files_finds_the_root_file() {
    assert_eq!(
        configs(&["crates/a"], &["clippy.toml"]),
        [PathBuf::from("clippy.toml")]
    );
}

#[test]
fn config_files_finds_a_member_own_file_and_one_between_it_and_the_root() {
    assert_eq!(
        configs(
            &["crates/a", "crates/b"],
            &["clippy.toml", "crates/a/clippy.toml", "crates/.clippy.toml"]
        ),
        [
            PathBuf::from("clippy.toml"),
            PathBuf::from("crates/.clippy.toml"),
            PathBuf::from("crates/a/clippy.toml"),
        ]
    );
}

#[test]
fn config_files_finds_a_dotted_root_file_and_names_each_file_once() {
    assert_eq!(
        configs(&["crates/a", "crates/b"], &[".clippy.toml", "clippy.toml"]),
        [PathBuf::from(".clippy.toml"), PathBuf::from("clippy.toml")]
    );
}

#[test]
fn config_files_ignores_files_outside_the_workspace_and_beside_no_member() {
    assert_eq!(
        configs(
            &["crates/a", "/elsewhere/c"],
            &[
                "clippy.toml",
                "crates/b/clippy.toml",
                "/elsewhere/c/clippy.toml"
            ]
        ),
        [PathBuf::from("clippy.toml")]
    );
}

/// A root configuration listing exactly `methods` and `types`, with a
/// macro ban and a comment that names a path, which never count.
fn root_with(methods: &[&str], types: &[&str]) -> String {
    let entries = |paths: &[&str]| -> String {
        paths
            .iter()
            .flat_map(|path| ["  { path = \"", path, "\", reason = \"r\" },\n"])
            .collect()
    };
    format!(
        "# path = \"std::thread::sleep\"\ndisallowed-methods = [\n{}]\ndisallowed-types = [\n{}]\n\
         disallowed-macros = [\n  {{ path = \"std::println\", reason = \"r\" }},\n]\n",
        entries(methods),
        entries(types)
    )
}

/// The paths of `BANS` under `lint`, each once.
fn ban_paths(lint: Lint) -> Vec<&'static str> {
    let mut paths: Vec<&str> = BANS
        .iter()
        .filter(|ban| ban.lint == lint)
        .map(|ban| ban.path)
        .collect();
    paths.dedup();
    paths
}

#[test]
fn uncovered_accepts_a_root_whose_every_ban_is_planted() {
    let text = root_with(&ban_paths(Lint::Method), &ban_paths(Lint::Type));
    assert_eq!(uncovered(&text), []);
}

#[test]
fn uncovered_accepts_the_repository_root_configuration() {
    let text = include_str!("../../../../clippy.toml");
    assert_eq!(uncovered(text), []);
}

#[test]
fn uncovered_reports_a_root_ban_with_no_planted_line() {
    let mut methods = ban_paths(Lint::Method);
    methods.push("std::thread::sleep");
    let text = root_with(&methods, &ban_paths(Lint::Type));
    assert_eq!(
        uncovered(&text),
        [Violation::Uncovered {
            lint: Lint::Method,
            path: "std::thread::sleep".to_owned()
        }]
    );
}

#[test]
fn uncovered_needs_a_planted_line_under_the_same_lint() {
    // A type ban listed as a method is not what the planted lines prove.
    let mut methods = ban_paths(Lint::Method);
    methods.push("std::process::Command");
    let text = root_with(&methods, &ban_paths(Lint::Type));
    assert_eq!(
        uncovered(&text),
        [Violation::Uncovered {
            lint: Lint::Method,
            path: "std::process::Command".to_owned()
        }]
    );
}

#[test]
fn overridden_accepts_an_unset_variable() {
    assert_eq!(overridden(None), None);
}

#[test]
fn overridden_refuses_any_set_value() {
    for value in ["/elsewhere", ""] {
        assert_eq!(
            overridden(Some(OsStr::new(value))),
            Some(Found {
                file: CONF_DIR.to_owned(),
                violation: Violation::Overridden(value.to_owned())
            }),
            "{value:?}"
        );
    }
}

#[test]
fn new_violations_say_what_failed() {
    assert_eq!(
        Violation::Uncovered {
            lint: Lint::Method,
            path: "std::thread::sleep".to_owned()
        }
        .to_string(),
        "clippy.toml bans the method std::thread::sleep, but check-bans plants no line that \
         reaches it, so nothing proves the ban fires: add one to BANS (coding standards rule 7)"
    );
    assert_eq!(
        Violation::Overridden("/elsewhere".to_owned()).to_string(),
        "CLIPPY_CONF_DIR is set (to \"/elsewhere\"), so clippy would lint every crate with \
         that directory's configuration instead of the files this check proves: unset it"
    );
}

#[test]
fn uncovered_reads_an_indented_key() {
    // Indenting a key is valid TOML; the array must still be read.
    let mut methods = ban_paths(Lint::Method);
    methods.push("std::thread::sleep");
    let text = root_with(&methods, &ban_paths(Lint::Type))
        .replace("disallowed-methods = [", "  disallowed-methods = [")
        .replace("]\ndisallowed-types", "  ]\ndisallowed-types");
    assert_eq!(unreadable(&text), []);
    assert_eq!(
        uncovered(&text),
        [Violation::Uncovered {
            lint: Lint::Method,
            path: "std::thread::sleep".to_owned()
        }]
    );
}

#[test]
fn unreadable_accepts_the_repository_configurations() {
    assert_eq!(unreadable(include_str!("../../../../clippy.toml")), []);
    assert_eq!(
        unreadable(include_str!("../../../ironafs-xdr/clippy.toml")),
        []
    );
    assert_eq!(
        unreadable(&root_with(&ban_paths(Lint::Method), &ban_paths(Lint::Type))),
        []
    );
}

#[test]
fn unreadable_reports_each_ban_format_the_reader_cannot_read() {
    for (text, line) in [
        ("disallowed-methods = [{ path = \"a::b\" }]\n", 1),
        ("disallowed-methods = [\n  \"a::b\",\n]\n", 2),
        (
            "disallowed-types = [\n  { path = \"a::B\" }, { path = \"c::D\" },\n]\n",
            2,
        ),
        (
            "disallowed-methods = [\n  { reason = \"r\", path = \"a::b\" },\n]\n",
            2,
        ),
        ("[[disallowed-methods]]\npath = \"a::b\"\n", 1),
        ("\"disallowed-methods\" = [\n]\n", 1),
        ("disallowed-methods = [ # bans\n]\n", 1),
        ("x = 1\ndisallowed_types = [\n]\n", 2),
    ] {
        let found = unreadable(text);
        assert!(
            matches!(found.as_slice(), [Violation::Unreadable { line: at, .. }] if *at == line),
            "{text:?}: {found:?}"
        );
    }
}

#[test]
fn root_config_is_either_name_in_the_root() {
    assert!(root_config(Path::new("clippy.toml")));
    assert!(root_config(Path::new(".clippy.toml")));
    assert!(!root_config(Path::new("crates/a/clippy.toml")));
    assert!(!root_config(Path::new("crates/a/.clippy.toml")));
}

/// A build-script-executed message from cargo for `package` with `env`.
fn executed(package: &str, env: &str) -> String {
    format!(
        "{{\"reason\":\"build-script-executed\",\"package_id\":\"{package}\",\
         \"linked_libs\":[],\"linked_paths\":[],\"cfgs\":[],\"env\":{env},\
         \"out_dir\":\"/t\"}}"
    )
}

#[test]
fn build_overrides_reports_a_variable_clippy_reads() {
    for name in ["CLIPPY_CONF_DIR", "CLIPPY_ARGS", "CARGO_MANIFEST_DIR"] {
        let messages = format!(
            "{}\n{}\n",
            executed("p#0.0.0", "[]"),
            executed(
                "q#0.0.0",
                &format!("[[\"OTHER\",\"1\"],[\"{name}\",\"/x\"]]")
            )
        );
        assert_eq!(
            build_overrides(&messages),
            [Violation::BuildOverride {
                package: "q#0.0.0".to_owned(),
                name: name.to_owned()
            }],
            "{name}"
        );
    }
}

#[test]
fn build_overrides_ignores_other_variables_and_messages() {
    let messages = format!(
        "{}\n{{\"reason\":\"compiler-artifact\",\"env\":[[\"CLIPPY_ARGS\",\"x\"]]}}\n\
         not json\n",
        executed("p#0.0.0", "[[\"OTHER\",\"1\"]]")
    );
    assert_eq!(build_overrides(&messages), []);
}

#[test]
fn read_and_build_violations_say_what_failed() {
    assert_eq!(
        Violation::Unreadable {
            line: 3,
            text: "  \"a::b\",".to_owned()
        }
        .to_string(),
        "line 3 is in a ban format check-bans cannot read, so its bans would go unchecked: \
         `  \"a::b\",`; write each ban as `{ path = \"...\", reason = \"...\" },` on its own \
         line under `disallowed-methods = [` or `disallowed-types = [`"
    );
    assert_eq!(
        Violation::BuildOverride {
            package: "q#0.0.0".to_owned(),
            name: "CLIPPY_CONF_DIR".to_owned()
        }
        .to_string(),
        "the build script of q#0.0.0 sets CLIPPY_CONF_DIR for its crate's compiler, which \
         changes the configuration or lints clippy runs with there: remove it"
    );
    assert_eq!(
        Violation::Unbuilt("error: e".to_owned()).to_string(),
        "the workspace's clippy did not finish, so not every build script's output was read: \
         error: e"
    );
}

#[test]
fn build_overrides_refuses_a_build_script_message_it_cannot_parse() {
    let messages = "{\"reason\":\"build-script-executed\",\"env\":[[\"CLIPPY_ARGS\"\n";
    assert!(
        matches!(
            build_overrides(messages).as_slice(),
            [Violation::Unbuilt(_)]
        ),
        "{:?}",
        build_overrides(messages)
    );
}
