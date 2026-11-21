// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads `cargo nextest list --message-format json`.

use std::path::PathBuf;

use super::Listed;
use crate::Error;
use crate::json::{self, Value};

#[cfg(test)]
mod tests;

const COMMAND: &str = "cargo nextest list";

/// Reads the run's one target triple, its target directory and every test of every binary,
/// ignored and filtered-out tests included, in the order listed. The
/// list's own count must equal the tests read.
///
/// # Errors
///
/// Returns an error if the text is not such a list, names no single
/// target, or its count disagrees.
pub(super) fn parse(text: &str) -> Result<(String, PathBuf, Vec<Listed>), Error> {
    let unexpected = || Error::UnexpectedOutput(COMMAND);
    let document = json::parse(text).map_err(|_| unexpected())?;
    let target = match document
        .get("rust-build-meta")
        .and_then(|meta| meta.get("target-platforms"))
        .and_then(Value::as_array)
    {
        Some([platform]) => platform
            .get("triple")
            .and_then(Value::as_str)
            .ok_or_else(unexpected)?
            .to_owned(),
        _ => return Err(unexpected()),
    };
    let target_dir = document
        .get("rust-build-meta")
        .and_then(|meta| meta.get("target-directory"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(unexpected)?;
    let Some(Value::Object(suites)) = document.get("rust-suites") else {
        return Err(unexpected());
    };
    let mut listed = Vec::new();
    for (_, suite) in suites {
        let field = |name| {
            suite
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(unexpected)
        };
        let package = field("package-name")?;
        let binary = field("binary-id")?;
        let Some(Value::Object(cases)) = suite.get("testcases") else {
            return Err(unexpected());
        };
        for (name, _) in cases {
            listed.push(Listed {
                package: package.to_owned(),
                binary: binary.to_owned(),
                name: name.clone(),
            });
        }
    }
    let count = match document.get("test-count") {
        Some(Value::Number(count)) => count.parse::<usize>().map_err(|_| unexpected())?,
        _ => return Err(unexpected()),
    };
    if count != listed.len() {
        return Err(unexpected());
    }
    Ok((target, target_dir, listed))
}
