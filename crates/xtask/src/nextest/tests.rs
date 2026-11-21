// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the logged subprocess run shared by the mutation run and the
//! planted-defect probe.

use std::fs;

use super::{BUILD_FAILED, NO_TESTS_RUN, TESTS_FAILED, run_logged};
use crate::Command;

#[test]
fn exit_codes_are_nextests() {
    assert_eq!((NO_TESTS_RUN, TESTS_FAILED, BUILD_FAILED), (4, 100, 101));
}

#[test]
fn run_logged_appends_output_to_the_log_and_prints_none() {
    let dir = std::env::temp_dir().join(format!("xtask-run-logged-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let log = dir.join("run.log");
    fs::write(&log, "earlier\n").unwrap();
    let status = run_logged(
        Command::new("cargo").arg("--version"),
        &log,
        "cargo --version",
    )
    .unwrap();
    assert!(status.success());
    let text = fs::read_to_string(&log).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert!(text.starts_with("earlier\n"), "{text}");
    assert!(text.contains("cargo "), "{text}");
}
