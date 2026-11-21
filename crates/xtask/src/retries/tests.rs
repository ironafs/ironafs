// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the check that no test is ever retried.

use std::fs;
use std::path::{Path, PathBuf};

use super::{Violation, check, command_violations, profile_violations};

const PROFILES: &str = ".config/nextest.toml";
const WORKFLOW: &str = ".github/workflows/ci.yml";

fn profile(line: usize, value: &str) -> Violation {
    Violation::Profile {
        file: PathBuf::from(PROFILES),
        line,
        value: value.to_owned(),
    }
}

fn option(line: usize, value: &str) -> Violation {
    Violation::Option {
        file: PathBuf::from(WORKFLOW),
        line,
        value: value.to_owned(),
    }
}

fn environment(line: usize, value: &str) -> Violation {
    Violation::Environment {
        file: PathBuf::from(WORKFLOW),
        line,
        value: value.to_owned(),
    }
}

#[track_caller]
fn assert_profiles(text: &str, expected: &[Violation]) {
    assert_eq!(profile_violations(text, Path::new(PROFILES)), expected);
}

#[track_caller]
fn assert_commands(text: &str, expected: &[Violation]) {
    assert_eq!(command_violations(text, Path::new(WORKFLOW)), expected);
}

#[test]
fn retries_check_fails_on_nonzero_retries() {
    assert_profiles(
        "[profile.default]\nretries = 0\n\n[profile.ci]\nretries = 1\n",
        &[profile(5, "1")],
    );
}

#[test]
fn profile_violations_pass_zero_and_ignore_comments() {
    assert_profiles(
        "# retries = 2 would hide a bug; NEXTEST_RETRIES and --retries 3 too.\n\
         [profile.default]\n\
         retries = 0 # never 1\n\
         slow-timeout = { period = \"30s\", terminate-after = 2 }\n",
        &[],
    );
}

#[test]
fn profile_violations_catch_every_spelling_of_the_key() {
    for (text, line, value) in [
        ("[profile.ci]\n\"retries\" = 2\n", 2, "2"),
        ("[profile.ci]\n'retries' = 2\n", 2, "2"),
        ("profile.ci.retries = 3\n", 1, "3"),
        ("[profile.ci]\nretries=4\n", 2, "4"),
        (
            "[profile.ci]\nretries = { backoff = \"fixed\", count = 2 }\n",
            2,
            "{",
        ),
        ("[profile.ci]\nretries = \"1\"\n", 2, "\"1\""),
        ("[profile.ci]\nretries = 01\n", 2, "01"),
        ("[profile.ci]\nretries = 0.5\n", 2, "0"),
        ("[profile.ci]\nretries =\n", 2, ""),
    ] {
        assert_profiles(text, &[profile(line, value)]);
    }
}

#[test]
fn profile_violations_catch_per_test_overrides() {
    assert_profiles(
        "[[profile.default.overrides]]\nfilter = 'test(#slow)'\nretries = 3\n",
        &[profile(3, "3")],
    );
    assert_profiles(
        "[profile.ci]\noverrides = [{ filter = \"test(a#b)\", retries = 1 }]\n",
        &[profile(2, "1")],
    );
    assert_profiles(
        "[profile.ci]\noverrides = [{ filter = \"test(a)\", retries = 0 }]\n",
        &[],
    );
}

#[test]
fn profile_violations_read_past_multiline_strings() {
    assert_profiles(
        "[profile.ci]\nnote = \"\"\"\nretries = 5\n\"\"\"\nretries = 6\n",
        &[profile(5, "6")],
    );
}

#[test]
fn profile_violations_refuse_an_unterminated_string() {
    assert_eq!(
        profile_violations("[profile.ci]\nfilter = \"test(a)\n", Path::new(PROFILES)),
        [Violation::Unreadable {
            file: PathBuf::from(PROFILES),
            error: "line 2: unterminated string".to_owned(),
        }]
    );
}

fn table(line: usize) -> Violation {
    Violation::ProfileTable {
        file: PathBuf::from(PROFILES),
        line,
    }
}

#[test]
fn profile_violations_catch_retries_as_a_table_in_every_spelling() {
    for (text, line) in [
        ("[profile.ci.retries]\nbackoff = \"fixed\"\ncount = 2\n", 1),
        ("[profile.ci . \"retries\"]\ncount = 2\n", 1),
        ("[[profile.ci.overrides.retries]]\ncount = 2\n", 1),
        ("[profile.ci]\nretries.backoff = \"fixed\"\n", 2),
        ("profile.ci.retries.count = 2\n", 1),
        ("[profile.ci]\n'retries'.count = 2\n", 2),
        (
            "[profile.ci]\noverrides = [{ filter = \"all()\", retries.count = 2 }]\n",
            2,
        ),
        (
            "[[profile.ci.overrides]]\nfilter = 'all()'\nretries.backoff = \"fixed\"\n",
            3,
        ),
    ] {
        assert_profiles(text, &[table(line)]);
    }
}

#[test]
fn profile_violations_ignore_retries_as_a_value() {
    assert_profiles(
        "[profile.ci]\nx = [\"retries\", \"y\"]\nz = [\n  \"retries\",\n]\nw = \"retries\"\n",
        &[],
    );
}

#[test]
fn profile_violations_decode_escapes_in_quoted_keys() {
    assert_profiles("[profile.ci]\n\"\\u0072etries\" = 2\n", &[profile(2, "2")]);
    assert_profiles(
        "[profile.ci]\n\"\\U00000072etries\".count = 2\n",
        &[table(2)],
    );
    assert_profiles("[profile.ci]\n\"\\x72etries\" = 2\n", &[profile(2, "2")]);
}

#[test]
fn profile_violations_count_lines_joined_by_a_line_ending_backslash() {
    assert_profiles(
        "[profile.ci]\nnote = \"\"\"a \\  \n\n   b\"\"\"\nretries = 1\n",
        &[profile(5, "1")],
    );
}

#[test]
fn profile_violations_refuse_an_invalid_escape() {
    for (text, error) in [
        (
            "[profile.ci]\nnote = \"a\\\nb\"\n",
            "line 2: invalid escape",
        ),
        ("[profile.ci]\nnote = \"\\q\"\n", "line 2: invalid escape"),
        ("[profile.ci]\nnote = \"\\u00\"\n", "line 2: invalid escape"),
    ] {
        assert_eq!(
            profile_violations(text, Path::new(PROFILES)),
            [Violation::Unreadable {
                file: PathBuf::from(PROFILES),
                error: error.to_owned(),
            }]
        );
    }
}

#[test]
fn command_violations_keep_quotes_attached_to_the_value() {
    for (text, expected) in [
        (
            "run: cargo nextest run --retries \"0\"1\n",
            option(1, "0\"1"),
        ),
        (
            "run: cargo nextest run --retries=0\"1\"\n",
            option(1, "0\"1\""),
        ),
        (
            "run: cargo nextest run --retries \"0\"$EXTRA\n",
            option(1, "0\"$EXTRA"),
        ),
        (
            "run: NEXTEST_RETRIES='0'2 cargo nextest run\n",
            environment(1, "0'2"),
        ),
        (
            "run: NEXTEST_RETRIES=0\"1\" cargo nextest run\n",
            environment(1, "0\"1\""),
        ),
    ] {
        assert_commands(text, &[expected]);
    }
}

fn escape(line: usize, spelled: char) -> Violation {
    Violation::Escape {
        file: PathBuf::from(WORKFLOW),
        line,
        spelled,
    }
}

#[test]
fn command_violations_catch_escapes_that_spell_a_name() {
    for (text, spelled) in [
        ("t = \"nextest run --\\u0072etries 2\"\n", 'r'),
        ("\"NEXTEST_\\u0052ETRIES\" = \"2\"\n", 'R'),
        ("run: cargo nextest run $'\\x2d'-retries 2\n", '-'),
        ("run: cargo nextest run --$'\\162'etries 2\n", 'r'),
        ("run: cargo nextest run --\\u{72}etries 2\n", 'r'),
        ("NEXTEST_RETRIES\\U0000003d2\n", '='),
    ] {
        let found = command_violations(text, Path::new(WORKFLOW));
        assert!(found.contains(&escape(1, spelled)), "{text}: {found:?}");
    }
}

#[test]
fn command_violations_pass_escapes_that_spell_no_name() {
    assert_commands(
        "run: printf '%s  %s\\n' a b\nx = \"\\u00e9 \\t \\u{1f} \\n\\\n y\"\n",
        &[],
    );
}

#[test]
fn command_violations_catch_the_option() {
    for (text, value) in [
        ("run: cargo nextest run --retries 2\n", "2"),
        ("run: cargo nextest run --retries=1\n", "1"),
        (
            "args = [\"nextest\", \"run\", \"--retries\", \"3\"]\n",
            "3\"",
        ),
        (
            "run: cargo nextest run --retries \"$RETRIES\"\n",
            "$RETRIES\"",
        ),
        ("run: cargo nextest run --retries\n", ""),
    ] {
        assert_commands(text, &[option(1, value)]);
    }
}

#[test]
fn command_violations_catch_the_environment() {
    for (text, value) in [
        ("    NEXTEST_RETRIES: 2\n", "2"),
        ("run: NEXTEST_RETRIES=1 cargo nextest run\n", "1"),
        (
            "run: echo \"NEXTEST_RETRIES=4\" >> \"$GITHUB_ENV\"\n",
            "4\"",
        ),
        ("NEXTEST_RETRIES = { value = \"2\", force = true }\n", "{"),
    ] {
        assert_commands(text, &[environment(1, value)]);
    }
}

#[test]
fn command_violations_catch_another_configuration_file() {
    for (text, option) in [
        (
            "run: cargo nextest run --config-file other.toml\n",
            "--config-file",
        ),
        (
            "run: cargo nextest run --tool-config-file tool:x.toml\n",
            "--tool-config-file",
        ),
    ] {
        assert_commands(
            text,
            &[Violation::ConfigFile {
                file: PathBuf::from(WORKFLOW),
                line: 1,
                option,
            }],
        );
    }
}

#[test]
fn command_violations_pass_zero_and_other_names() {
    assert_commands(
        "run: cargo nextest run --workspace --profile ci --retries 0 --no-tests fail\n\
         env:\n  NEXTEST_RETRIES: 0\n  MY_NEXTEST_RETRIES: 3\n\
         run: cargo nextest run --retries=0\n\
         args = [\"--retries\", \"0\"]\n",
        &[],
    );
}

#[test]
fn command_violations_number_lines_from_one() {
    assert_commands(
        "jobs:\n  test:\n    steps:\n      - run: cargo nextest run --retries 9\n",
        &[option(4, "9")],
    );
}

#[test]
fn xtask_check_passes_retries_zero_to_nextest() {
    let command = crate::check::NEXTEST_RUN;
    assert!(
        command.windows(2).any(|pair| pair == ["--retries", "0"]),
        "{command:?}"
    );
    assert_eq!(
        command_violations(&command.join(" "), Path::new("crates/xtask/src/check.rs")),
        []
    );
}

/// A fresh, empty directory for one test.
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ironafs-xtask-retries-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create scratch directory");
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("create directory");
    fs::write(path, text).expect("write file");
}

#[test]
fn check_reads_profiles_workflows_actions_cargo_configuration_and_xtask() {
    let root = scratch("reads");
    write(&root, PROFILES, "[profile.default]\nretries = 1\n");
    write(&root, WORKFLOW, "run: cargo nextest run --retries 2\n");
    write(
        &root,
        ".github/actions/test/action.yml",
        "env:\n  NEXTEST_RETRIES: 3\n",
    );
    write(
        &root,
        ".cargo/config.toml",
        "[alias]\nt = \"nextest run --retries 4\"\n",
    );
    write(
        &root,
        "crates/a/.cargo/config",
        "[env]\nNEXTEST_RETRIES = \"5\"\n",
    );
    write(
        &root,
        "crates/xtask/src/mutants.rs",
        "fn f() { run([\"nextest\", \"run\", \"--retries\", \"8\"]); }\n",
    );
    write(
        &root,
        "crates/xtask/src/retries.rs",
        "const O: &str = \"--retries\";\n",
    );
    write(&root, "crates/xtask/src/retries/x.rs", "--retries 9\n");
    write(&root, "target/.github/x.yml", "--retries 6\n");
    write(&root, "docs/notes.md", "--retries 7\n");
    let (found, read) = check(&root, &root.join("target")).expect("check runs");
    let _ = fs::remove_dir_all(&root);
    assert_eq!(read, 6);
    assert_eq!(
        found,
        [
            Violation::Option {
                file: PathBuf::from(".cargo/config.toml"),
                line: 2,
                value: "4\"".to_owned(),
            },
            profile(2, "1"),
            Violation::Environment {
                file: PathBuf::from(".github/actions/test/action.yml"),
                line: 2,
                value: "3".to_owned(),
            },
            option(1, "2"),
            Violation::Environment {
                file: PathBuf::from("crates/a/.cargo/config"),
                line: 2,
                value: "5\"".to_owned(),
            },
            Violation::Option {
                file: PathBuf::from("crates/xtask/src/mutants.rs"),
                line: 1,
                value: "8\"".to_owned(),
            },
        ]
    );
}

#[test]
fn check_fails_without_the_nextest_configuration_or_xtask() {
    let root = scratch("missing");
    write(&root, WORKFLOW, "run: cargo nextest run --retries 0\n");
    let (found, read) = check(&root, &root.join("target")).expect("check runs");
    let _ = fs::remove_dir_all(&root);
    assert_eq!(read, 1);
    assert_eq!(
        found,
        [
            Violation::Missing(PathBuf::from(PROFILES)),
            Violation::Missing(PathBuf::from("crates/xtask")),
        ]
    );
}

#[test]
fn check_does_not_take_another_file_for_the_profiles() {
    let root = scratch("lookalike");
    write(
        &root,
        ".github/x/.config/nextest.toml",
        "run: --retries 3\n",
    );
    write(&root, "crates/xtask/src/lib.rs", "\n");
    let (found, _) = check(&root, &root.join("target")).expect("check runs");
    let _ = fs::remove_dir_all(&root);
    assert_eq!(
        found,
        [
            Violation::Missing(PathBuf::from(PROFILES)),
            Violation::Option {
                file: PathBuf::from(".github/x/.config/nextest.toml"),
                line: 1,
                value: "3".to_owned(),
            },
        ]
    );
}

#[test]
fn check_passes_this_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().expect("repository root");
    let (found, read) = check(&root, &root.join("target")).expect("check runs");
    assert_eq!(found, []);
    assert!(read >= 3, "read {read} files");
}

#[test]
fn command_violations_catch_escapes_behind_doubled_backslashes() {
    for (text, spelled) in [
        ("run: \"cargo nextest run --$'\\\\162'etries 2\"\n", 'r'),
        ("run: \"cargo nextest run $'\\\\x2d'-retries 2\"\n", '-'),
        ("run: \"cargo nextest run --$'\\\\\\\\x72'etries 2\"\n", 'r'),
        ("x = \"--\\u{7_2}etries\"\n", 'r'),
        ("x = \"\\\\u0072\"\n", 'r'),
    ] {
        let found = command_violations(text, Path::new(WORKFLOW));
        assert!(found.contains(&escape(1, spelled)), "{text}: {found:?}");
    }
}

#[test]
fn command_violations_catch_a_name_split_by_a_line_ending_backslash() {
    for text in [
        "t = \"\"\"nextest run --re\\\n tries 2\"\"\"\n",
        "\"NEXTEST_\\\n RETRIES\": \"2\"\n",
        "run: cargo nextest run --re\\\r\ntries 2\n",
    ] {
        let found = command_violations(text, Path::new(WORKFLOW));
        assert!(
            found.contains(&Violation::Joined {
                file: PathBuf::from(WORKFLOW),
                line: 1,
            }),
            "{text}: {found:?}"
        );
    }
    assert_commands(
        "run: |\n  cargo nextest run \\\n    --retries 0 \\\n    --workspace\n",
        &[],
    );
}

#[test]
fn command_violations_refuse_a_backtick_that_opens_a_substitution() {
    for (text, expected) in [
        (
            "run: cargo nextest run --retries 0` echo 2`\n",
            option(1, "0`"),
        ),
        (
            "run: cargo nextest run --retries \"0` echo 2`\"\n",
            option(1, "0`"),
        ),
        (
            "run: NEXTEST_RETRIES=0` echo 2` cargo nextest run\n",
            environment(1, "0`"),
        ),
        ("run: cargo nextest run --retries 0.5\n", option(1, "0.5")),
    ] {
        assert_commands(text, &[expected]);
    }
}

#[test]
fn command_violations_pass_a_zero_ending_prose_or_a_code_span() {
    assert_commands(
        "# Pass `--retries 0`. Always --retries 0. Or --retries 0: so.\n\
         # The run passes `--retries 0`, and `NEXTEST_RETRIES=0`.\n",
        &[],
    );
}

#[test]
fn profile_violations_read_quotes_before_a_multiline_closing_delimiter() {
    for text in [
        "[profile.ci]\nnote = \"\"\"a\"\"\"\"\nretries = 0\n",
        "[profile.ci]\nnote = '''a''''\nretries = 0\n",
        "[profile.ci]\nnote = \"\"\"a\"\"\"\"\"\nretries = 0\n",
    ] {
        assert_profiles(text, &[]);
    }
    assert_profiles(
        "[profile.ci]\njunit = { path = \"\"\"a\"\"\"\", report-name = \"\"\"b\"\"\"\"\" }\n\
         retries = 2\n# \"\"\"\n",
        &[profile(3, "2")],
    );
    assert_eq!(
        profile_violations(
            "[profile.ci]\nnote = \"\"\"a\"\"\"\"\"\"\n",
            Path::new(PROFILES)
        ),
        [Violation::Unreadable {
            file: PathBuf::from(PROFILES),
            error: "line 2: too many quotes before a closing delimiter".to_owned(),
        }]
    );
}
