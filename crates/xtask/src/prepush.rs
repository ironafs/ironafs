// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pre-push hook: refuses any update of `main`, scans the outgoing
//! commits and their messages for secrets with gitleaks (the `land-change`
//! skill, section 0), then runs the local checks.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::process::Stdio;

use crate::{Command, Error};

#[cfg(test)]
mod tests;

const CHECK: &str = "pre-push";

/// What a gitleaks run reads.
#[derive(Clone, Copy, Debug)]
enum Scanner {
    /// The changes of the outgoing commits.
    Commits,
    /// The outgoing commits' messages, which a commit scan does not read.
    Messages,
}

/// Runs the pre-push hook for the updates Git writes to `input`.
///
/// # Errors
///
/// Returns an error if an update touches `main`, the input is malformed,
/// gitleaks finds a secret or cannot complete, or a local check fails.
pub(super) fn run(
    remote: &OsStr,
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut updates = String::new();
    input
        .read_to_string(&mut updates)
        .map_err(Error::io("read pre-push updates"))?;
    let remote = remote
        .to_str()
        .ok_or(Error::UnexpectedOutput("pre-push remote name"))?;
    let ranges = plan(remote, &updates).map_err(|violations| Error::Rules {
        check: CHECK,
        violations,
    })?;
    for range in &ranges {
        scan_commits(range)?;
        scan_messages(range)?;
    }
    crate::check::run(output)
}

/// The `git log` revisions to scan for each update; refuses `main`.
fn plan(remote: &str, updates: &str) -> Result<Vec<Vec<String>>, Vec<String>> {
    let mut ranges = Vec::new();
    let mut violations = Vec::new();
    for (index, line) in updates.lines().enumerate() {
        let line_number = index + 1;
        let fields: Vec<&str> = line.split(' ').collect();
        let [_, local, remote_ref, remote_id] = fields.as_slice() else {
            violations.push(malformed(line_number));
            continue;
        };
        if !is_object_id(local) || !is_object_id(remote_id) {
            violations.push(malformed(line_number));
            continue;
        }
        if *remote_ref == "refs/heads/main" {
            violations.push(format!(
                "line {line_number} updates refs/heads/main: push your task branch; main changes only through an approved squash merge (ADR 0002, ADR 0003)"
            ));
            continue;
        }
        if is_zero(local) {
            // A deletion sends no commits.
            continue;
        }
        ranges.push(if is_zero(remote_id) {
            vec![
                (*local).to_owned(),
                "--not".to_owned(),
                format!("--remotes={remote}"),
            ]
        } else {
            vec![format!("{remote_id}..{local}")]
        });
    }
    if violations.is_empty() {
        Ok(ranges)
    } else {
        Err(violations)
    }
}

fn malformed(line: usize) -> String {
    format!("line {line} is not `<local ref> <local id> <remote ref> <remote id>`")
}

fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_zero(id: &str) -> bool {
    id.bytes().all(|byte| byte == b'0')
}

fn scan_commits(range: &[String]) -> Result<(), Error> {
    let status = Command::new("gitleaks")
        .args(["git", "--no-banner", "--redact", "--log-opts"])
        .arg(range.join(" "))
        .arg(".")
        .stdin(Stdio::null())
        .status();
    scanner_result(
        Scanner::Commits,
        status
            .map(|status| status.code())
            .map_err(|error| error.kind()),
    )
}

fn scan_messages(range: &[String]) -> Result<(), Error> {
    const COMMAND: &str = "git log --format=%B";
    let log = Command::new("git")
        .args(["log", "--format=%B"])
        .args(range)
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, log.status)?;
    let result = Command::new("gitleaks")
        .args(["stdin", "--no-banner", "--redact"])
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&log.stdout)?;
            }
            child.wait()
        });
    scanner_result(
        Scanner::Messages,
        result
            .map(|status| status.code())
            .map_err(|error| error.kind()),
    )
}

/// Reads gitleaks' exit: 0 is clean, 1 found a secret, anything else, or
/// no exit code at all, means the scan did not complete.
fn scanner_result(
    scanner: Scanner,
    result: Result<Option<i32>, io::ErrorKind>,
) -> Result<(), Error> {
    let violation = match result {
        Ok(Some(0)) => return Ok(()),
        Ok(Some(1)) => match scanner {
            Scanner::Commits => {
                "gitleaks found a secret in the commits to push: remove it (the land-change skill, section 0)"
            }
            Scanner::Messages => {
                "gitleaks found a secret in the messages of the commits to push: remove it (the land-change skill, section 0)"
            }
        },
        Err(io::ErrorKind::NotFound) => {
            "gitleaks is not installed: the pre-push hook needs it to scan outgoing commits"
        }
        Ok(_) | Err(_) => "gitleaks did not complete, so the push is refused",
    };
    Err(Error::Rules {
        check: CHECK,
        violations: vec![violation.to_owned()],
    })
}
