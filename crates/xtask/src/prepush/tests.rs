// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading Git's pre-push updates, refusing `main`, choosing the
//! commits to scan and reading the scanner's result.

use std::io;

use super::{Scanner, plan, scanner_result};
use crate::Error;

const LOCAL: &str = "1111111111111111111111111111111111111111";
const REMOTE: &str = "2222222222222222222222222222222222222222";
const ZERO: &str = "0000000000000000000000000000000000000000";

#[test]
fn plan_scans_only_commits_the_remote_lacks() {
    let updates = format!(
        "refs/heads/topic {LOCAL} refs/heads/topic {REMOTE}\nrefs/heads/new {LOCAL} refs/heads/new {ZERO}\n"
    );
    assert_eq!(
        plan("origin", &updates),
        Ok(vec![
            vec![format!("{REMOTE}..{LOCAL}")],
            vec![
                LOCAL.to_owned(),
                "--not".to_owned(),
                "--remotes=origin".to_owned()
            ],
        ])
    );
}

#[test]
fn plan_skips_deletions_and_accepts_sha256_ids() {
    let sha256 = "a".repeat(64);
    let zero256 = "0".repeat(64);
    assert_eq!(
        plan(
            "origin",
            &format!(
                "(delete) {ZERO} refs/heads/old {REMOTE}\nrefs/heads/t {sha256} refs/heads/t {zero256}\n"
            )
        ),
        Ok(vec![vec![
            sha256.clone(),
            "--not".to_owned(),
            "--remotes=origin".to_owned()
        ]])
    );
}

#[test]
fn plan_refuses_any_update_of_main() {
    for line in [
        format!("refs/heads/topic {LOCAL} refs/heads/main {REMOTE}"),
        format!("(delete) {ZERO} refs/heads/main {REMOTE}"),
        format!("HEAD {LOCAL} refs/heads/main {ZERO}"),
    ] {
        assert_eq!(
            plan("origin", &line),
            Err(vec![
                "line 1 updates refs/heads/main: push your task branch; main changes only through an approved squash merge (ADR 0002, ADR 0003)"
                    .to_owned()
            ]),
            "{line}"
        );
    }
}

#[test]
fn plan_rejects_malformed_updates() {
    for line in [
        "refs/heads/t".to_owned(),
        format!("refs/heads/t {LOCAL} refs/heads/t"),
        format!("refs/heads/t xyz refs/heads/t {REMOTE}"),
        format!("refs/heads/t {LOCAL} refs/heads/t {REMOTE} extra"),
    ] {
        assert_eq!(
            plan("origin", &line),
            Err(vec![
                "line 1 is not `<local ref> <local id> <remote ref> <remote id>`".to_owned()
            ]),
            "{line}"
        );
    }
}

#[test]
fn scanner_result_passes_only_on_a_clean_exit() {
    assert!(matches!(
        scanner_result(Scanner::Commits, Ok(Some(0))),
        Ok(())
    ));
    assert!(matches!(
        scanner_result(Scanner::Commits, Ok(Some(1))),
        Err(Error::Rules { check: "pre-push", violations }) if violations
            == ["gitleaks found a secret in the commits to push: remove it (the land-change skill, section 0)"]
    ));
    assert!(matches!(
        scanner_result(Scanner::Messages, Ok(Some(1))),
        Err(Error::Rules { check: "pre-push", violations }) if violations
            == ["gitleaks found a secret in the messages of the commits to push: remove it (the land-change skill, section 0)"]
    ));
    assert!(matches!(
        scanner_result(Scanner::Commits, Ok(None)),
        Err(Error::Rules { check: "pre-push", violations }) if violations
            == ["gitleaks did not complete, so the push is refused"]
    ));
    assert!(matches!(
        scanner_result(Scanner::Commits, Ok(Some(2))),
        Err(Error::Rules { check: "pre-push", violations }) if violations
            == ["gitleaks did not complete, so the push is refused"]
    ));
    assert!(matches!(
        scanner_result(Scanner::Commits, Err(io::ErrorKind::NotFound)),
        Err(Error::Rules { check: "pre-push", violations }) if violations
            == ["gitleaks is not installed: the pre-push hook needs it to scan outgoing commits"]
    ));
}
