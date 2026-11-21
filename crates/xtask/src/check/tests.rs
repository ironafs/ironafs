// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for optional tool detection and diagnostics.

use std::io;

use std::ffi::OsStr;

use super::{check_reuse_result, clippy, lists_deny, report_skipped};
use crate::Error;

#[test]
fn check_reports_each_missing_optional_tool_by_name() {
    let mut output = Vec::new();
    check_reuse_result(Err(io::ErrorKind::NotFound.into()), || false, &mut output).unwrap();
    report_skipped("cargo-deny", &mut output).unwrap();

    assert_eq!(
        output,
        b"skipped reuse (not installed)\nskipped cargo-deny (not installed)\n"
    );
}

#[test]
fn check_reports_failure_when_reuse_cannot_start() {
    for (kind, in_path) in [
        (io::ErrorKind::PermissionDenied, false),
        (io::ErrorKind::NotFound, true),
    ] {
        let mut output = Vec::new();
        let Err(Error::Io { operation, source }) =
            check_reuse_result(Err(kind.into()), || in_path, &mut output)
        else {
            panic!("expected a reuse start error");
        };

        assert_eq!(operation, "run reuse lint");
        assert_eq!(source.kind(), kind);
        assert_eq!(output, [] as [u8; 0]);
    }
}

#[test]
fn check_detects_deny_as_an_exact_cargo_command() {
    assert!(lists_deny(b"Installed Commands:\n    check   Check a package\n    deny\n").unwrap());
    assert!(
        !lists_deny(b"Installed Commands:\n    check   Check a package\n    deny-all\n").unwrap()
    );
    assert!(matches!(
        lists_deny(b"Installed Commands:\n"),
        Err(Error::UnexpectedOutput("cargo --list"))
    ));
}

#[test]
fn check_lints_without_a_clippy_configuration_override() {
    // The variable would replace the configuration check-bans proves.
    let command = clippy();
    let removed = command
        .get_envs()
        .any(|(name, value)| name == OsStr::new(crate::bans::CONF_DIR) && value.is_none());
    assert!(
        removed,
        "{CONF_DIR} is not removed",
        CONF_DIR = crate::bans::CONF_DIR
    );
}
