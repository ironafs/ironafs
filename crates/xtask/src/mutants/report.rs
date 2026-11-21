// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report of a shard or of a pull request's diff: counts per package, the surviving and timed-out
//! mutants with their accepted reason or "new", and every mutant with no
//! result. It names code only, never a test's output (testing
//! specification 5.5, point 2).
//!
//! A shard fails when its result cannot be trusted: the outputs are
//! malformed or from another tool version, the baseline is missing or
//! failed, or a mutant has no result. Survivors and timeouts are reported
//! and do not fail it while mutation testing is report only (5.4, point 4).

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use super::TOOL_VERSION;
use super::accepted::Accepted;
use super::outcomes::{Class, Mutant, parse_list, parse_outcomes, reconcile};

#[cfg(test)]
mod tests;

/// One shard of a run, `index/count`, numbered from zero as the tool
/// numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shard {
    pub(crate) index: u32,
    pub(crate) count: u32,
}

impl Shard {
    /// Reads `k/n` with `k < n`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (index, count) = text.split_once('/')?;
        let number = |part: &str| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        };
        let (index, count) = (number(index)?, number(count)?);
        (index < count).then_some(Self { index, count })
    }
}

impl fmt::Display for Shard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.index, self.count)
    }
}

/// What a report covers: one shard of the nightly run, or the mutants a
/// pull request's diff touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Shard(Shard),
    Diff,
}

impl fmt::Display for Scope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shard(shard) => write!(formatter, "shard {shard}"),
            Self::Diff => formatter.write_str("the pull request's diff"),
        }
    }
}

/// The report's Markdown and the reasons the shard fails, if any.
#[derive(Debug)]
pub(super) struct Report {
    pub(super) text: String,
    pub(super) violations: Vec<String>,
}

/// Builds the report of one shard, or of a diff, from the tool's
/// `--list --json` for the same selection and its `outcomes.json` (`None`
/// when the tool was not run because the list was empty).
pub(super) fn report(
    scope: Scope,
    outcomes: Option<&str>,
    list: &str,
    accepted: &[Accepted],
) -> Report {
    let mut violations = Vec::new();
    let mut text =
        format!("## Mutation testing: {scope}\n\nTool: cargo-mutants {TOOL_VERSION}.\n\n");
    let listed = match parse_list(list) {
        Ok(listed) => listed,
        Err(reason) => {
            violations.push(format!("the mutant list is malformed: {reason}"));
            return Report { text, violations };
        }
    };
    let classed = match outcomes {
        None if listed.is_empty() => {
            text.push_str(match scope {
                Scope::Shard(_) => "This shard has no mutants.\n",
                Scope::Diff => "The diff touches no mutants.\n",
            });
            return Report { text, violations };
        }
        None => {
            violations.push(format!(
                "{} mutants are listed for {scope}, but there are no outcomes",
                listed.len()
            ));
            return Report { text, violations };
        }
        Some(outcomes) => match parse_outcomes(outcomes) {
            Ok(parsed) => {
                if parsed.version != TOOL_VERSION {
                    violations.push(format!(
                        "outcomes.json was written by cargo-mutants {}, not the pinned {TOOL_VERSION}",
                        parsed.version
                    ));
                }
                if !listed.is_empty() && parsed.baseline != Some(true) {
                    violations.push(
                        "the baseline is missing or failed, so no mutant result counts".to_owned(),
                    );
                }
                reconcile(&parsed, &listed)
            }
            Err(reason) => {
                violations.push(format!("outcomes.json is malformed: {reason}"));
                return Report { text, violations };
            }
        },
    };
    let _ = writeln!(text, "Mutants listed for {scope}: {}.\n", listed.len());
    write_counts(&mut text, &classed);
    write_survivors(&mut text, &classed, accepted);
    let no_result: Vec<&Mutant> = classed
        .iter()
        .filter(|mutant| matches!(mutant.class, Class::NoResult(_)))
        .collect();
    if !no_result.is_empty() {
        text.push_str("\n### No result\n\n");
        for mutant in no_result {
            if let Class::NoResult(reason) = &mutant.class {
                let _ = writeln!(text, "- `{}`: {reason}", mutant.name);
                violations.push(format!("no result for `{}`: {reason}", mutant.name));
            }
        }
    }
    Report { text, violations }
}

/// The pull-request report: report only, so the reasons a result cannot
/// be trusted are shown in it rather than failing the job.
pub(super) fn diff_text(report: Report) -> String {
    let mut text = report.text;
    text.push_str(
        "\nReport only: mutation testing over the diff never fails the pull request \
         (testing specification 5.4, point 4).\n",
    );
    if !report.violations.is_empty() {
        text.push_str("\n### Not trusted\n\n");
        for violation in report.violations {
            let _ = writeln!(text, "- {violation}");
        }
    }
    text
}

/// Caught, missed, timed out, unviable and no result, per package.
fn write_counts(text: &mut String, classed: &[Mutant]) {
    let mut counts: BTreeMap<&str, [usize; 5]> = BTreeMap::new();
    for mutant in classed {
        let column = match mutant.class {
            Class::Caught => 0,
            Class::Missed => 1,
            Class::Timeout => 2,
            Class::Unviable => 3,
            Class::NoResult(_) => 4,
        };
        let row = counts.entry(mutant.package.as_str()).or_default();
        if let Some(cell) = row.get_mut(column) {
            *cell += 1;
        }
    }
    text.push_str("| Package | Caught | Missed | Timed out | Unviable | No result |\n");
    text.push_str("|---|---|---|---|---|---|\n");
    for (package, [caught, missed, timeout, unviable, none]) in counts {
        let package = if package.is_empty() {
            "(unknown)"
        } else {
            package
        };
        let _ = writeln!(
            text,
            "| {package} | {caught} | {missed} | {timeout} | {unviable} | {none} |"
        );
    }
}

/// The missed and timed-out mutants, each with its accepted reason or
/// "new".
fn write_survivors(text: &mut String, classed: &[Mutant], accepted: &[Accepted]) {
    let survivors: Vec<(&Mutant, &str)> = classed
        .iter()
        .filter_map(|mutant| match mutant.class {
            Class::Missed => Some((mutant, "missed")),
            Class::Timeout => Some((mutant, "timed out")),
            _ => None,
        })
        .collect();
    if survivors.is_empty() {
        return;
    }
    text.push_str("\n### Missed and timed out\n\n");
    for (mutant, how) in survivors {
        let reason = mutant.key().and_then(|key| {
            accepted
                .iter()
                .find(|entry| entry.key == key)
                .map(|entry| entry.reason.as_str())
        });
        match reason {
            Some(reason) => {
                let _ = writeln!(text, "- `{}`: {how}, accepted: {reason}", mutant.name);
            }
            None => {
                let _ = writeln!(text, "- `{}`: {how}, new", mutant.name);
            }
        }
    }
}
