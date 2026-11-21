// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for `cargo xtask coverage`'s command layer.

use super::{JOB_VARIABLE, host_target, job_violation, self_test, tool_version_violation};

#[test]
fn only_the_pinned_tool_version_is_accepted() {
    assert_eq!(tool_version_violation("cargo-llvm-cov 0.9.1\n"), None);
    for line in [
        "cargo-llvm-cov 0.9.2\n",
        "cargo-llvm-cov 0.9.10\n",
        "cargo-llvm-cov 0.9.1-beta\n",
        "",
        "something else 0.9.1\n",
    ] {
        let violation = tool_version_violation(line);
        assert!(
            violation
                .as_deref()
                .is_some_and(|text| text.contains("0.9.1")),
            "{line:?}: {violation:?}"
        );
    }
}

#[test]
fn host_target_is_read_from_rustc_verbose_version() {
    let text = "rustc 1.100.0 (abc 2026-11-12)\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\nrelease: 1.100.0\n";

    assert_eq!(host_target(text), Some("x86_64-unknown-linux-gnu"));
    assert_eq!(host_target("rustc 1.100.0\n"), None);
    assert_eq!(host_target("host: \n"), None);
}

#[test]
fn self_test_catches_every_planted_case_and_passes_the_clean_one() {
    let mut output = Vec::new();

    self_test::run(&mut output).unwrap();

    let text = String::from_utf8(output).unwrap();
    for case in [
        "an empty export",
        "a malformed export",
        "a summary-only export",
        "an export missing an expected crate",
        "an export counting a planted test file",
        "an excluded file in the export",
        "an export from another tool version",
        "a crate in no table",
        "a crate in two tables",
        "an exclusion without a reason",
    ] {
        assert!(
            text.contains(&format!("caught: {case}: ")),
            "{case} missing:\n{text}"
        );
    }
    assert!(text.contains("passed: the clean export"), "{text}");
}

#[test]
fn the_real_run_refuses_outside_the_coverage_job() {
    assert_eq!(job_violation(Some("1")), None);
    for value in [None, Some(""), Some("0"), Some("true")] {
        let violation = job_violation(value);
        assert!(
            violation
                .as_deref()
                .is_some_and(|text| text.contains(JOB_VARIABLE)),
            "{value:?}: {violation:?}"
        );
    }
}
