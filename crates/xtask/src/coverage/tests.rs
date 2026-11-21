// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for `cargo xtask coverage`'s command layer.

use std::fs;
use std::path::PathBuf;

use super::{
    JOB_VARIABLE, host_target, job_violation, prepare_export_path, read_export, self_test,
    tool_version_violation,
};
use crate::Error;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-coverage-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

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
        "a file summarised twice",
        "function records that disagree with the file summaries",
        "a function record for a library file without a summary",
        "a measured crate with no instrumented line",
        "an export with no workspace library file",
        "a crate with only generated files",
        "a crate listed as not built that has files",
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

#[test]
fn the_export_path_is_prepared_on_a_fresh_runner_and_a_stale_export_removed() {
    let dir = scratch("prepare");
    let path = dir.join("xtask-coverage").join("export.json");

    prepare_export_path(&path).unwrap();
    assert!(path.parent().unwrap().is_dir());
    fs::write(&path, "stale").unwrap();
    prepare_export_path(&path).unwrap();
    assert!(!path.exists());

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_export_over_the_size_limit_is_refused_unread() {
    let dir = scratch("bounded");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.json");
    fs::write(&path, "x".repeat(17)).unwrap();

    assert_eq!(read_export(&path, 17).unwrap(), "x".repeat(17));
    let refused = read_export(&path, 16).unwrap_err();
    assert!(
        matches!(&refused, Error::Rules { violations, .. } if violations[0].contains("16 bytes")),
        "{refused}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_oversized_export_is_refused_even_when_the_limit_splits_a_character() {
    let dir = scratch("split");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.json");
    fs::write(&path, format!("{}é", "x".repeat(16))).unwrap();

    let refused = read_export(&path, 16).unwrap_err();

    assert!(
        matches!(&refused, Error::Rules { violations, .. } if violations[0].contains("16 bytes")),
        "{refused}"
    );
    fs::remove_dir_all(&dir).unwrap();
}
