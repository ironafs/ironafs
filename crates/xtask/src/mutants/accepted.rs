// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads `tests/mutants-accepted.txt`: the surviving or timed-out mutants
//! that are accepted, each with a one-line reason (testing specification
//! 5.4, point 4).
//!
//! An entry is two lines, `mutant: <file>: <change>` and, directly under
//! it, `reason: <text>`. The mutant is named as the tool names it without
//! the line and column, so an entry survives unrelated edits above it.
//! Lines starting with `#` and blank lines separate entries.

use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

/// One accepted mutant.
#[derive(Debug)]
pub(super) struct Accepted {
    /// `file: change`, the tool's name without `:line:column`.
    pub(super) key: String,
    pub(super) reason: String,
}

/// Reads the list, returning its entries and every violation, each naming
/// its line.
pub(super) fn parse_accepted(text: &str) -> (Vec<Accepted>, Vec<String>) {
    let mut entries = Vec::new();
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();
    let mut pending: Option<(usize, String)> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim_end();
        if let Some(reason) = line.strip_prefix("reason:") {
            let reason = reason.trim();
            match pending.take() {
                Some((at, key)) if !reason.is_empty() => {
                    if !seen.insert(key.clone()) {
                        violations.push(format!("line {at}: mutant {key:?} is listed twice"));
                    }
                    entries.push(Accepted {
                        key,
                        reason: reason.to_owned(),
                    });
                }
                Some((at, key)) => {
                    violations.push(format!("line {at}: mutant {key:?} has no reason"));
                }
                None => violations.push(format!(
                    "line {number}: a reason with no mutant line directly above it"
                )),
            }
            continue;
        }
        if let Some((at, key)) = pending.take() {
            violations.push(format!("line {at}: mutant {key:?} has no reason under it"));
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(key) = line.strip_prefix("mutant:") {
            let key = key.trim();
            if !well_formed(key) {
                violations.push(format!(
                    "line {number}: {key:?} is not `file.rs: change` (no line or column)"
                ));
            }
            pending = Some((number, key.to_owned()));
        } else {
            violations.push(format!(
                "line {number}: neither `mutant:`, `reason:`, a comment nor blank"
            ));
        }
    }
    if let Some((at, key)) = pending {
        violations.push(format!("line {at}: mutant {key:?} has no reason under it"));
    }
    (entries, violations)
}

/// Whether `key` has the form `file.rs: change`.
fn well_formed(key: &str) -> bool {
    key.split_once(": ").is_some_and(|(file, change)| {
        std::path::Path::new(file)
            .extension()
            .is_some_and(|extension| extension == "rs")
            && !file.contains(':')
            && !change.trim().is_empty()
    })
}
