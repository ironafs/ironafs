// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the bans' configuration and clippy's output on the
//! fixture.

use std::path::{Path, PathBuf};

use std::ffi::OsStr;

use super::snapshot as snapshot_of;
use super::template;
use super::{
    BANS, Ban, CONF_DIR, Found, Lint, Snapshot, TIME_CRATE_EXEMPT, Violation, build_overrides,
    cargo_warnings, changed, config_files, crate_config_drift, fired_violations, line_breaks,
    listed, outside_root, overridden, plain_terminal, planted_source, root_config, uncovered,
    unreadable, workspace_clippy,
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

/// Cargo's last message after a build that succeeded.
const FINISHED: &str = "{\"reason\":\"build-finished\",\"success\":true}\n";

/// A compiler-artifact message for `package`'s build script.
fn script_built(package: &str) -> String {
    format!(
        "{{\"reason\":\"compiler-artifact\",\"package_id\":\"{package}\",\
         \"target\":{{\"kind\":[\"custom-build\"],\"name\":\"build-script-build\"}},\
         \"fresh\":true}}"
    )
}

#[test]
fn build_overrides_reports_a_variable_clippy_reads() {
    for name in ["CLIPPY_CONF_DIR", "CLIPPY_ARGS", "CARGO_MANIFEST_DIR"] {
        let messages = format!(
            "{}\n{}\n{FINISHED}",
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
fn build_overrides_reports_a_variable_clippy_reads_in_any_case() {
    // Windows reads environment names without regard to case.
    for name in ["clippy_conf_dir", "Clippy_Conf_Dir", "cargo_manifest_dir"] {
        let messages = format!(
            "{}\n{FINISHED}",
            executed("q#0.0.0", &format!("[[\"{name}\",\"/x\"]]"))
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
        "{}\n{}\n{{\"reason\":\"compiler-artifact\",\"target\":{{\"kind\":[\"lib\"]}},\"env\":[[\"CLIPPY_ARGS\",\"x\"]]}}\n\n\
         {FINISHED}",
        script_built("p#0.0.0"),
        executed("p#0.0.0", "[[\"OTHER\",\"1\"]]")
    );
    assert_eq!(build_overrides(&messages), []);
}

#[test]
fn build_overrides_refuses_a_report_it_cannot_read_whole() {
    for (case, messages) in [
        (
            "no env",
            "{\"reason\":\"build-script-executed\",\"package_id\":\"q#0.0.0\"}\n".to_owned(),
        ),
        ("env not an array", executed("q#0.0.0", "{}")),
        (
            "a pair not an array",
            executed("q#0.0.0", "[\"CLIPPY_ARGS\"]"),
        ),
        ("a pair of one", executed("q#0.0.0", "[[\"CLIPPY_ARGS\"]]")),
        (
            "a pair of three",
            executed("q#0.0.0", "[[\"A\",\"1\",\"2\"]]"),
        ),
        ("a name not a string", executed("q#0.0.0", "[[1,\"x\"]]")),
        ("a value not a string", executed("q#0.0.0", "[[\"A\",1]]")),
        (
            "no package",
            "{\"reason\":\"build-script-executed\",\"env\":[]}\n".to_owned(),
        ),
        ("a line that is not json", "not json".to_owned()),
    ] {
        let messages = format!("{messages}\n{FINISHED}");
        let found = build_overrides(&messages);
        assert!(
            matches!(found.as_slice(), [Violation::Unbuilt(_)]),
            "{case}: {found:?}"
        );
    }
}

#[test]
fn build_overrides_needs_cargo_to_say_the_build_finished() {
    for (case, end) in [
        ("no record", ""),
        (
            "a failed build",
            "{\"reason\":\"build-finished\",\"success\":false}\n",
        ),
        (
            "success not a bool",
            "{\"reason\":\"build-finished\",\"success\":\"true\"}\n",
        ),
    ] {
        let messages = format!("{}\n{end}", executed("p#0.0.0", "[]"));
        let found = build_overrides(&messages);
        assert!(
            matches!(found.as_slice(), [Violation::Unbuilt(_)]),
            "{case}: {found:?}"
        );
    }
}

#[test]
fn build_overrides_needs_a_report_from_every_build_script_built() {
    let messages = format!(
        "{}\n{}\n{}\n{FINISHED}",
        script_built("p#0.0.0"),
        script_built("q#0.0.0"),
        executed("p#0.0.0", "[]")
    );
    let found = build_overrides(&messages);
    assert!(
        matches!(found.as_slice(), [Violation::Unbuilt(error)] if error.contains("q#0.0.0")),
        "{found:?}"
    );
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
    let messages =
        format!("{{\"reason\":\"build-script-executed\",\"env\":[[\"CLIPPY_ARGS\"\n{FINISHED}");
    assert!(
        matches!(
            build_overrides(&messages).as_slice(),
            [Violation::Unbuilt(_)]
        ),
        "{:?}",
        build_overrides(&messages)
    );
}

#[test]
fn unreadable_reports_a_ban_key_spelt_another_way() {
    // TOML decodes each of these keys to a ban key clippy reads.
    for text in [
        "\"disallowed\\u002dmethods\" = [\n]\n",
        "\"disallowed\\U0000002Dtypes\" = [\n]\n",
        "\"disallowed\\u005fmethods\" = [\n]\n",
        "[[\"disallowed\\u002dtypes\"]]\npath = \"a::B\"\n",
        "'disallowed-methods' = [\n]\n",
        "disallowed-method = [\n]\n",
    ] {
        let found = unreadable(text);
        assert!(
            matches!(found.as_slice(), [Violation::Unreadable { line: 1, .. }]),
            "{text:?}: {found:?}"
        );
    }
}

#[test]
fn unreadable_reports_a_quoted_key_it_cannot_decode() {
    for text in ["\"a\\q\" = 1\n", "\"a\\u00\" = 1\n", "\"a = 1\n"] {
        let found = unreadable(text);
        assert!(
            matches!(found.as_slice(), [Violation::Unreadable { line: 1, .. }]),
            "{text:?}: {found:?}"
        );
    }
}

#[test]
fn unreadable_accepts_other_quoted_keys() {
    assert_eq!(
        unreadable("\"doc\\u002dvalid-idents\" = [\"a\"]\n'x' = 1\n"),
        []
    );
}

#[test]
fn unreadable_reports_an_entry_line_holding_more_than_one_ban() {
    for text in [
        "disallowed-methods = [\n  { path = \"a::b\" }, { \"path\" = \"c::d\" },\n]\n",
        "disallowed-methods = [\n  { path = \"a::b\", reason = \"r\" }, { 'path' = \"c::d\" },\n]\n",
        "disallowed-methods = [\n  { path = \"a\\u003a:b\", reason = \"r\" },\n]\n",
        "disallowed-methods = [\n  { path = \"a::b\", reason = \"r\\\" }, {\" },\n]\n",
    ] {
        let found = unreadable(text);
        assert!(
            matches!(found.as_slice(), [Violation::Unreadable { line: 2, .. }]),
            "{text:?}: {found:?}"
        );
    }
}

/// A snapshot holding each of `files` with its text.
fn snapshot(files: &[(&str, &str)]) -> Snapshot {
    files
        .iter()
        .map(|(file, text)| (PathBuf::from(file), text.as_bytes().to_vec()))
        .collect()
}

/// A configuration `changed` names for `file`.
fn changed_file(file: &str) -> Found {
    Found {
        file: file.to_owned(),
        violation: Violation::Changed,
    }
}

#[test]
fn changed_accepts_the_same_configurations() {
    let files = snapshot(&[("clippy.toml", "a"), ("crates/b/clippy.toml", "b")]);
    assert_eq!(changed(&files, &files.clone()), []);
}

#[test]
fn changed_reports_a_configuration_rewritten_added_or_removed() {
    let before = snapshot(&[("clippy.toml", "a"), ("crates/b/clippy.toml", "b")]);
    for (after, file) in [
        (
            snapshot(&[("clippy.toml", "a "), ("crates/b/clippy.toml", "b")]),
            "clippy.toml",
        ),
        (
            snapshot(&[
                ("clippy.toml", "a"),
                ("crates/b/clippy.toml", "b"),
                ("crates/c/.clippy.toml", "a"),
            ]),
            "crates/c/.clippy.toml",
        ),
        (snapshot(&[("clippy.toml", "a")]), "crates/b/clippy.toml"),
    ] {
        assert_eq!(changed(&before, &after), [changed_file(file)], "{file}");
    }
}

#[test]
fn changed_violation_says_what_failed() {
    assert_eq!(
        changed_file("crates/b/clippy.toml").to_string(),
        "crates/b/clippy.toml: added, removed or rewritten while the workspace was built, so \
         clippy may have linted with a configuration this check did not prove: a build script \
         writes it; remove that"
    );
}

#[test]
fn build_overrides_refuses_a_message_with_no_reason_it_knows() {
    for line in [
        "{\"reason\":\"something-new\"}",
        "{\"no\":\"reason\"}",
        "{\"reason\":1}",
        "[]",
    ] {
        let found = build_overrides(&format!("{line}\n{FINISHED}"));
        assert!(
            matches!(found.as_slice(), [Violation::Unbuilt(_)]),
            "{line}: {found:?}"
        );
    }
}

#[test]
fn build_overrides_needs_build_finished_last() {
    let messages = format!("{FINISHED}{}\n", executed("p#0.0.0", "[]"));
    let found = build_overrides(&messages);
    assert!(
        matches!(found.as_slice(), [Violation::Unbuilt(_)]),
        "{found:?}"
    );
}

#[test]
fn build_overrides_needs_a_report_for_each_build_script_unit() {
    let messages = format!(
        "{}\n{}\n{}\n{FINISHED}",
        script_built("p#0.0.0"),
        script_built("p#0.0.0"),
        executed("p#0.0.0", "[]")
    );
    let found = build_overrides(&messages);
    assert!(
        matches!(found.as_slice(), [Violation::Unbuilt(_)]),
        "{found:?}"
    );
}

#[test]
fn build_overrides_refuses_an_artifact_with_no_target_kinds() {
    for artifact in [
        "{\"reason\":\"compiler-artifact\",\"package_id\":\"p#0.0.0\"}",
        "{\"reason\":\"compiler-artifact\",\"package_id\":\"p#0.0.0\",\"target\":{\"kind\":\"custom-build\"}}",
        "{\"reason\":\"compiler-artifact\",\"package_id\":\"p#0.0.0\",\"target\":{\"kind\":[1]}}",
    ] {
        let found = build_overrides(&format!("{artifact}\n{FINISHED}"));
        assert!(
            matches!(found.as_slice(), [Violation::Unbuilt(_)]),
            "{artifact}: {found:?}"
        );
    }
}

#[test]
fn uncovered_reads_past_a_byte_order_mark() {
    // TOML skips a leading byte order mark, and so does clippy.
    let text = "\u{feff}\"disallowed\\u002dmethods\" = [\n  { path = \"std::thread::sleep\", reason = \"r\" },\n]\n";
    assert!(
        matches!(
            unreadable(text).as_slice(),
            [Violation::Unreadable { line: 1, .. }]
        ),
        "{:?}",
        unreadable(text)
    );
    let text =
        "\u{feff}disallowed-methods = [\n  { path = \"std::thread::sleep\", reason = \"r\" },\n]\n";
    assert_eq!(unreadable(text), []);
    assert_eq!(
        uncovered(text),
        [Violation::Uncovered {
            lint: Lint::Method,
            path: "std::thread::sleep".to_owned()
        }]
    );
}

#[test]
fn cargo_warnings_refuses_each_warning_cargo_prints() {
    for line in [
        "warning: unused manifest key: package.bogus",
        "warning: planted@0.0.0: planted",
        "error: something cargo said",
        "warning[E0001]: a coded warning",
    ] {
        let stderr = format!(
            "    Checking planted v0.0.0 (/w/planted)\n{line}\n    Finished `dev` profile in 0.2s\n"
        );
        assert_eq!(
            cargo_warnings(&stderr),
            [Violation::Lint(line.to_owned())],
            "{line}"
        );
    }
}

#[test]
fn cargo_warnings_accepts_cargo_progress() {
    let stderr = "   Compiling planted v0.0.0 (/w/warning)\n    Checking x v0.0.0\n    \
                  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s\n";
    assert_eq!(cargo_warnings(stderr), []);
}

#[test]
fn cargo_warnings_reads_a_warning_after_a_progress_bar() {
    let stderr = "    Building [=====>   ] 3/9: planted\rwarning: planted@0.0.0: x\n";
    assert_eq!(
        cargo_warnings(stderr),
        [Violation::Lint("warning: planted@0.0.0: x".to_owned())]
    );
}

#[test]
fn plain_terminal_sets_every_terminal_setting_over_the_caller() {
    let mut command = crate::Command::new("cargo");
    command
        .env("CARGO_TERM_QUIET", "true")
        .env("CARGO_TERM_PROGRESS_WHEN", "always");
    plain_terminal(&mut command);
    let set = |name: &str| {
        command
            .get_envs()
            .find(|(key, _)| *key == std::ffi::OsStr::new(name))
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned())
    };
    for (name, value) in [
        ("CARGO_TERM_QUIET", "false"),
        ("CARGO_TERM_VERBOSE", "false"),
        ("CARGO_TERM_PROGRESS_WHEN", "never"),
        ("CARGO_TERM_COLOR", "never"),
    ] {
        assert_eq!(set(name).as_deref(), Some(value), "{name}");
    }
}

#[test]
fn line_breaks_refuses_a_path_holding_one() {
    let paths = [
        Path::new("/w/ok"),
        Path::new("/w/a\nwarning: x"),
        Path::new("/w/b\rc"),
    ];
    let found = line_breaks(&paths);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .all(|violation| matches!(violation, Violation::Unbuilt(_))),
        "{found:?}"
    );
}

#[test]
fn workspace_clippy_lints_every_feature() {
    // Code behind a feature no build enables would otherwise never be
    // compiled, so no ban could fire on it.
    let command = workspace_clippy(Path::new("clippy-driver"));
    assert!(
        command.get_args().any(|arg| arg == "--all-features"),
        "{:?}",
        command.get_args().collect::<Vec<_>>()
    );
}

#[test]
fn outside_root_refuses_a_member_outside_the_root() {
    let root = Path::new("/w");
    let members = [
        PathBuf::from("/w/crates/a"),
        PathBuf::from("/elsewhere/b"),
        PathBuf::from("/w2/c"),
    ];
    let found = outside_root(root, &members);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().all(
            |violation| matches!(violation, Violation::Unbuilt(text) if text.contains("outside"))
        ),
        "{found:?}"
    );
}

#[test]
fn snapshot_covers_the_cargo_configurations_of_every_directory_it_is_given() {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-snapshot-{}", std::process::id()));
    let fixtures = root.join("target").join("fixtures");
    let config = fixtures.join(".cargo").join("config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "").unwrap();
    let held = |dirs: &[PathBuf]| {
        snapshot_of(&root, &[], dirs)
            .unwrap()
            .iter()
            .any(|(file, _)| *file == config)
    };
    let with = held(&[root.clone(), fixtures.clone()]);
    let without = held(std::slice::from_ref(&root));
    std::fs::remove_dir_all(&root).unwrap();
    assert!(with, "the fixtures' chain is snapshot");
    assert!(!without, "only the directories given are");
}

#[test]
fn template_refuses_a_build_directory_with_template_syntax() {
    assert_eq!(template(Path::new("/w/target/check-bans-lint-1")), None);
    for path in ["/w/{workspace-root}/target", "/w/a}b/target"] {
        assert!(
            matches!(template(Path::new(path)), Some(Violation::Unbuilt(text)) if text.contains("template")),
            "{path}"
        );
    }
}

#[test]
fn workspace_clippy_is_cargo_check_with_clippy_s_driver() {
    let command = workspace_clippy(Path::new("/t/bin/clippy-driver"));
    assert_eq!(command.get_args().next(), Some(OsStr::new("check")));
    let set = |name: &str| {
        command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(name))
            .and_then(|(_, value)| value)
    };
    assert_eq!(
        set("RUSTC_WORKSPACE_WRAPPER"),
        Some(OsStr::new("/t/bin/clippy-driver"))
    );
    assert_eq!(set("CLIPPY_ARGS"), Some(OsStr::new("")));
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

#[test]
fn isolated_build_refuses_a_directory_cargo_would_read_as_a_template() {
    let mut command = crate::Command::new("cargo");
    assert!(super::isolated_build(&mut command, Path::new("/w/target/lint")).is_ok());
    for dir in ["/w/{workspace-root}/lint", "/w/a{b/lint", "/w/a}b/lint"] {
        let mut command = crate::Command::new("cargo");
        let error = super::isolated_build(&mut command, Path::new(dir))
            .unwrap_err()
            .to_string();
        assert!(error.contains(dir) && error.contains("template"), "{error}");
        assert_eq!(command.get_envs().count(), 0, "{dir}");
    }
}
