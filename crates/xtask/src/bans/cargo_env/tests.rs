// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading cargo's configuration for variables clippy reads.

use std::path::{Path, PathBuf};

use super::{candidates, violations};
use crate::bans::Violation;

/// Whether `found` is exactly one refusal of `name` on `line`.
#[track_caller]
fn assert_sets(text: &str, line: usize, name: &str) {
    let found = violations(text);
    assert_eq!(
        found,
        [Violation::CargoEnv {
            line,
            name: name.to_owned()
        }],
        "{text:?}"
    );
}

#[test]
fn violations_refuse_an_env_table_setting_a_variable_clippy_reads() {
    assert_sets("[env]\nCLIPPY_CONF_DIR = \"/x\"\n", 2, "CLIPPY_CONF_DIR");
    assert_sets("[env]\nCLIPPY_ARGS = \"x\"\n", 2, "CLIPPY_ARGS");
    assert_sets(
        "[env]\nCARGO_MANIFEST_DIR = \"/x\"\n",
        2,
        "CARGO_MANIFEST_DIR",
    );
    assert_sets(
        "[env]\nCLIPPY_CONF_DIR = { value = \"x\", relative = true }\n",
        2,
        "CLIPPY_CONF_DIR",
    );
}

#[test]
fn violations_refuse_every_spelling_of_the_key() {
    // Dotted, inline, quoted, escaped, in a header, and in another case.
    assert_sets("env.CLIPPY_CONF_DIR = \"/x\"\n", 1, "CLIPPY_CONF_DIR");
    assert_sets(
        "env = { clippy_conf_dir = { value = \"/x\", force = true } }\n",
        1,
        "clippy_conf_dir",
    );
    assert_sets(
        "[env]\n\"CLIPPY\\u005fCONF_DIR\" = \"/x\"\n",
        2,
        "CLIPPY_CONF_DIR",
    );
    assert_sets(
        "[env.'Clippy_Conf_Dir']\nvalue = \"/x\"\n",
        1,
        "Clippy_Conf_Dir",
    );
    assert_sets(
        "\"\\u0065nv\" . CLIPPY_CONF_DIR = \"/x\"\n",
        1,
        "CLIPPY_CONF_DIR",
    );
    assert_sets("[ENV]\nCLIPPY_CONF_DIR = \"/x\"\n", 2, "CLIPPY_CONF_DIR");
}

#[test]
fn violations_refuse_an_include() {
    for text in [
        "include = [\"more.toml\"]\n",
        "\"includ\\u0065\" = [{ path = \"more.toml\" }]\n",
    ] {
        assert_eq!(
            violations(text),
            [Violation::CargoInclude { line: 1 }],
            "{text:?}"
        );
    }
}

#[test]
fn violations_refuse_a_line_they_cannot_read() {
    for (text, line) in [
        ("[alias]\nx = \"\"\"\nrun\n\"\"\"\n", 2),
        ("x = '''\n'''\n", 1),
        ("\"a\\q\" = 1\n", 1),
        ("x = \"open\n", 1),
    ] {
        let found = violations(text);
        assert!(
            matches!(found.first(), Some(Violation::Unreadable { line: at, .. }) if *at == line),
            "{text:?}: {found:?}"
        );
    }
}

#[test]
fn violations_accept_other_configuration() {
    for text in [
        include_str!("../../../../../.cargo/config.toml"),
        "[env]\nOPENSSL_DIR = { value = \"vendor\", relative = true }\n",
        "[alias]\nclippy-all = \"clippy --workspace\" # not CLIPPY_CONF_DIR = x\n",
        "[build]\njobs = 4\n[target.'cfg(unix)']\nrunner = \"x = y\"\n",
    ] {
        assert_eq!(violations(text), [], "{text:?}");
    }
}

#[test]
fn candidates_are_both_names_in_each_ancestor_and_the_cargo_home() {
    let found = candidates(
        &[PathBuf::from("/w/a"), PathBuf::from("/w/t")],
        Path::new("/h/.cargo"),
    );
    let expected: Vec<PathBuf> = [
        "/.cargo/config",
        "/.cargo/config.toml",
        "/h/.cargo/config",
        "/h/.cargo/config.toml",
        "/w/.cargo/config",
        "/w/.cargo/config.toml",
        "/w/a/.cargo/config",
        "/w/a/.cargo/config.toml",
        "/w/t/.cargo/config",
        "/w/t/.cargo/config.toml",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn violations_read_past_a_byte_order_mark() {
    assert_sets(
        "\u{feff}[env]\nCLIPPY_CONF_DIR = \"/x\"\n",
        2,
        "CLIPPY_CONF_DIR",
    );
}
