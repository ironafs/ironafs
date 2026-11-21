// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The freshness check of the scheduled tiers (`docs/specs/testing.md`,
//! 5.5, point 3): each tier's last complete run must be recent.
//!
//! A run counts only if it is a scheduled run of the tier's workflow on the
//! default branch, and only through its first attempt: a re-run never
//! replaces the first result. It is complete when every id of the tier's
//! checked-in manifest (`tests/tiers/<tier>.txt`) was reported as run, pass
//! or fail: the first attempt has a job named after the id, and every such
//! job has a step named `run <id>` that succeeded or failed. A step GitHub
//! skipped or cancelled did not run, so a job that failed before reaching
//! it leaves the run incomplete. Staleness and failure are separate alarms:
//! a complete run whose items failed still counts as fresh.

mod github;
mod self_test;

use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, metadata};

#[cfg(test)]
mod tests;

const CHECK: &str = "freshness";

pub(crate) const DAY: u64 = 86_400;

/// The branch scheduled workflows run on.
const DEFAULT_BRANCH: &str = "main";

/// A scheduled tier: its manifest is `tests/tiers/<name>.txt`.
#[derive(Debug)]
pub(crate) struct Tier {
    pub(crate) name: &'static str,
    /// The workflow file, as the API reports a run's `path`.
    pub(crate) workflow: &'static str,
    /// The oldest a complete run may be, in seconds.
    pub(crate) max_age: u64,
}

pub(crate) const TIERS: &[Tier] = &[
    Tier {
        name: "nightly",
        workflow: ".github/workflows/nightly.yml",
        max_age: 2 * DAY,
    },
    Tier {
        name: "weekly",
        workflow: ".github/workflows/weekly.yml",
        max_age: 8 * DAY,
    },
];

/// One page of an API list and the total the API says exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listing<T> {
    pub(crate) total: u64,
    pub(crate) items: Vec<T>,
}

/// A workflow run as the run list reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    pub(crate) id: u64,
    /// The latest attempt; the check reads attempt 1 whatever this is.
    pub(crate) attempt: u64,
    pub(crate) status: String,
    pub(crate) event: String,
    pub(crate) branch: String,
    pub(crate) path: String,
    /// Seconds since the Unix epoch.
    pub(crate) created: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    pub(crate) name: String,
    pub(crate) attempt: u64,
    pub(crate) steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) name: String,
    /// Empty while the step has not concluded.
    pub(crate) conclusion: String,
}

/// What the check found for one tier.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The newest complete run, and its age in seconds.
    Fresh { run: u64, age: u64 },
    /// No complete run within the tier's age, with a note per finding.
    Stale(Vec<String>),
    /// The answer depends on data the check could not read.
    Unverified(String),
}

/// Judges one tier from its run list, fetching a run's first-attempt jobs
/// through `jobs` only while the answer needs them.
pub(crate) fn evaluate(
    tier: &Tier,
    manifest: &[String],
    branch: &str,
    now: u64,
    runs: &Listing<Run>,
    jobs: &mut dyn FnMut(u64) -> Result<Listing<Job>, String>,
) -> Verdict {
    let mut ordered: Vec<&Run> = runs.items.iter().collect();
    ordered.sort_by_key(|run| std::cmp::Reverse(run.created));
    let mut notes = Vec::new();
    let mut reached_age_limit = false;
    for run in ordered {
        if run.path != tier.workflow || run.branch != branch || run.event != "schedule" {
            continue;
        }
        let age = now.saturating_sub(run.created);
        if age > tier.max_age {
            // The API lists newest first, so every run not on this page is
            // older still.
            reached_age_limit = true;
            break;
        }
        // A re-run starts only after attempt 1 completed.
        if run.attempt == 1 && run.status != "completed" {
            notes.push(format!("run {}: still {}", run.id, run.status));
            continue;
        }
        let listed = match jobs(run.id) {
            Ok(listed) => listed,
            Err(reason) => return Verdict::Unverified(format!("run {}: {reason}", run.id)),
        };
        if listed.total != u64::try_from(listed.items.len()).unwrap_or(u64::MAX) {
            return Verdict::Unverified(format!(
                "run {}: {} of {} jobs listed",
                run.id,
                listed.items.len(),
                listed.total
            ));
        }
        let missing: Vec<&str> = manifest
            .iter()
            .filter(|id| !reported(&listed.items, id))
            .map(String::as_str)
            .collect();
        if missing.is_empty() {
            return Verdict::Fresh { run: run.id, age };
        }
        notes.push(format!("run {}: not run: {}", run.id, missing.join(", ")));
    }
    let seen = u64::try_from(runs.items.len()).unwrap_or(u64::MAX);
    if !reached_age_limit && runs.total != seen {
        return Verdict::Unverified(format!(
            "only {seen} of {} runs listed, and none of them is complete and recent",
            runs.total
        ));
    }
    notes.push(format!(
        "no complete run of {} in the last {} days",
        tier.name,
        tier.max_age / DAY
    ));
    Verdict::Stale(notes)
}

/// Whether attempt 1 ran manifest item `id`: at least one job of that
/// name, and in every one of them the step `run <id>` succeeded or failed.
fn reported(jobs: &[Job], id: &str) -> bool {
    let step = format!("run {id}");
    let mut named = jobs
        .iter()
        .filter(|job| job.attempt == 1 && job.name == id)
        .peekable();
    named.peek().is_some()
        && named.all(|job| {
            job.steps.iter().any(|candidate| {
                candidate.name == step
                    && matches!(candidate.conclusion.as_str(), "success" | "failure")
            })
        })
}

/// Reads a tier manifest: one id per line (lowercase letters, digits and
/// `-`), `#` comments and blank lines ignored, at least one id, none twice.
pub(crate) fn parse_manifest(text: &str) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!("{line:?} is not a manifest id"));
        }
        if ids.iter().any(|id| id == line) {
            return Err(format!("{line:?} is listed twice"));
        }
        ids.push(line.to_owned());
    }
    if ids.is_empty() {
        return Err("the manifest lists nothing".to_owned());
    }
    Ok(ids)
}

/// Runs the check over every tier against the GitHub API, or its self-test.
///
/// # Errors
///
/// [`Error::Rules`] when a tier is stale or incomplete (exit status 1);
/// [`Error::CouldNotRun`] when a manifest or the API could not be read, or a
/// list was cut short (exit status 2).
#[expect(
    clippy::disallowed_methods,
    reason = "rule 7 covers shipped code; the freshness check measures a run's age against \
              the real date, and xtask is never shipped"
)]
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let root = metadata::load(None, &[])?.root;
    // The one read of the real clock; everything below takes `now`.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::CouldNotRun {
            check: CHECK,
            reason: format!("the system clock is before 1970: {error}"),
        })?
        .as_secs();
    let mut stale = Vec::new();
    let mut unverified = Vec::new();
    for tier in TIERS {
        let path = root.join("tests/tiers").join(format!("{}.txt", tier.name));
        let manifest = fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| parse_manifest(&text));
        let verdict = match manifest {
            Err(reason) => Verdict::Unverified(format!("manifest {}: {reason}", path.display())),
            Ok(manifest) => match github::runs(tier.workflow, DEFAULT_BRANCH) {
                Err(reason) => Verdict::Unverified(reason),
                Ok(runs) => evaluate(
                    tier,
                    &manifest,
                    DEFAULT_BRANCH,
                    now,
                    &runs,
                    &mut github::first_attempt_jobs,
                ),
            },
        };
        match verdict {
            Verdict::Fresh { run, age } => writeln!(
                output,
                "fresh: {} (run {run}, {} hours old)",
                tier.name,
                age / 3_600
            )
            .map_err(Error::io("report freshness"))?,
            Verdict::Stale(notes) => stale.extend(
                notes
                    .into_iter()
                    .map(|note| format!("{}: {note}", tier.name)),
            ),
            Verdict::Unverified(reason) => unverified.push(format!("{}: {reason}", tier.name)),
        }
    }
    if !stale.is_empty() {
        stale.extend(
            unverified
                .into_iter()
                .map(|line| format!("not verified: {line}")),
        );
        return Err(Error::Rules {
            check: CHECK,
            violations: stale,
        });
    }
    if !unverified.is_empty() {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: unverified.join("; "),
        });
    }
    Ok(())
}
