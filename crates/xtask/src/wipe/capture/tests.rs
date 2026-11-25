// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive and negative evidence from the binary stop and capture records.

use std::ffi::OsString;
use std::path::Path;

use super::{Session, Stage, cargo_messages, forward, require_binary_stop};
use crate::Error;

const TARGET: &str = "x86_64-unknown-linux-gnu";
const MARKER: &str = "error: ironafs-wipe-stop-capture-1-0";
const TERMINAL: &str = "error: could not compile `ironafs-cli` (bin \"afs\")\n\nCaused by:\n  process didn't exit successfully: (exit status: 86)\n";

#[test]
fn binary_marker_must_be_the_sole_cargo_failure() {
    let stderr = format!("{MARKER}\n{TERMINAL}");
    require_binary_stop(Some(101), stderr.as_bytes(), MARKER).unwrap();
    let with_dependency_failure =
        format!("{MARKER}\nerror: could not compile `fixture-dep` (lib)\n{TERMINAL}");
    for (status, stderr, marker) in [
        (Some(0), stderr.as_str(), MARKER),
        (
            Some(101),
            stderr.as_str(),
            "error: ironafs-wipe-stop-capture-1-1",
        ),
        (Some(101), with_dependency_failure.as_str(), MARKER),
        (Some(101), TERMINAL, MARKER),
    ] {
        let Error::Rules { violations, .. } =
            require_binary_stop(status, stderr.as_bytes(), marker).unwrap_err()
        else {
            panic!("expected binary stop refusal")
        };
        assert_eq!(
            violations,
            ["Cargo did not end solely at the expected afs binary marker"]
        );
    }
}

#[test]
fn binary_stop_does_not_hide_a_json_dependency_error() {
    let stdout = br#"{"reason":"compiler-message","message":{"level":"error"}}
{"reason":"build-finished","success":false}
"#;
    let Error::Rules { violations, .. } = cargo_messages(stdout, true, TARGET, &[]).unwrap_err()
    else {
        panic!("expected Cargo diagnostic refusal")
    };
    assert_eq!(
        violations,
        ["Cargo reported a compiler error during wipe capture"]
    );
}

#[test]
fn cargo_completion_must_match_the_stage_and_be_terminal() {
    let stdout = br#"{"reason":"build-finished","success":false}"#;
    assert!(
        cargo_messages(stdout, true, TARGET, &[])
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        cargo_messages(stdout, false, TARGET, &[]),
        Err(Error::UnexpectedOutput("Cargo build message sequence"))
    ));
    let duplicate = br#"{"reason":"build-finished","success":false}
{"reason":"build-finished","success":false}"#;
    assert!(matches!(
        cargo_messages(duplicate, true, TARGET, &[]),
        Err(Error::UnexpectedOutput("Cargo build message sequence"))
    ));
}

#[test]
fn capture_preserves_argument_boundaries_and_cannot_replace_a_record() {
    let dir = crate::fixture::fresh_dir(&std::env::temp_dir(), "wipe-capture-record").unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let session = Session::create(
        &dir,
        Path::new("/compiler/bin/rustc"),
        Stage::Library,
        TARGET,
    )
    .unwrap();
    let argv = [
        "a path with spaces",
        "cfg=\"value with spaces\"",
        "line\nbreak",
        "",
    ];
    let arguments = argv.map(OsString::from);
    session.record(&arguments).unwrap();
    assert!(matches!(
        session.record(&[OsString::from("replacement")]),
        Err(Error::PathIo {
            operation: "publish wipe capture",
            ..
        })
    ));
    assert_eq!(session.records().unwrap(), [argv]);
    crate::fixture::remove_dir(&dir).unwrap();
}

#[test]
fn internal_wrapper_requires_a_scoped_capture_session() {
    let mut diagnostics = Vec::new();
    assert!(matches!(
        forward(
            Path::new("outside-session"),
            [OsString::from("rustc")].into_iter(),
            &mut diagnostics
        ),
        Err(Error::UnexpectedOutput("internal wipe capture session"))
    ));
    assert_eq!(diagnostics, [] as [u8; 0]);
}
