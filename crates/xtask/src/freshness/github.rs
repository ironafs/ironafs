// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads workflow runs and their first attempt's jobs from the GitHub API
//! through `gh api --jq`, which prints the fields the check needs as
//! tab-separated lines, so no JSON or HTTP crate is needed (coding standards
//! rule 21). `gh` takes its token from `GH_TOKEN` in CI; nothing here reads
//! or prints it.

use std::process::Stdio;

use super::{Job, Listing, Run, Step};
use crate::Command;

#[cfg(test)]
mod tests;

/// Runs per page: the API's largest page.
const PAGE: &str = "100";

/// The total, then one line per run: id, latest attempt, status, event,
/// branch, workflow file and creation time.
const RUNS_JQ: &str = ".total_count, (.workflow_runs[] | [.id, .run_attempt, .status, .event, \
                       .head_branch, .path, .created_at] | @tsv)";

/// The total, then each job (`job`, index, name, attempt) followed by its
/// steps (`step`, the job's index, name, conclusion or empty).
const JOBS_JQ: &str = ".total_count, (.jobs | to_entries[] | .key as $k | .value as $j \
                       | ([\"job\", $k, $j.name, $j.run_attempt] | @tsv), \
                       ($j.steps[]? | [\"step\", $k, .name, (.conclusion // \"\")] | @tsv))";

/// Lists the newest page of scheduled runs of `workflow` on `branch`.
///
/// `gh` resolves `{owner}/{repo}` from `GH_REPO` or the checkout's remote.
pub(super) fn runs(workflow: &str, branch: &str) -> Result<Listing<Run>, String> {
    let file = workflow.rsplit('/').next().unwrap_or(workflow);
    let text = gh(&[
        &format!("repos/{{owner}}/{{repo}}/actions/workflows/{file}/runs"),
        "-f",
        &format!("branch={branch}"),
        "-f",
        "event=schedule",
        "-f",
        &format!("per_page={PAGE}"),
        "--jq",
        RUNS_JQ,
    ])?;
    parse_runs(&text)
}

/// Lists the jobs of the first attempt of run `id`.
pub(super) fn first_attempt_jobs(id: u64) -> Result<Listing<Job>, String> {
    let text = gh(&[
        &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}/attempts/1/jobs"),
        "-f",
        &format!("per_page={PAGE}"),
        "--jq",
        JOBS_JQ,
    ])?;
    parse_jobs(&text)
}

fn gh(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(["api", "--method", "GET"])
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot run gh: {error}"))?;
    if !output.status.success() {
        return Err(format!("gh api failed with {}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|_| "gh api printed invalid UTF-8".to_owned())
}

/// Seconds since the Unix epoch of an API time, `YYYY-MM-DDTHH:MM:SSZ`.
pub(super) fn parse_utc(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20
        || [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'Z'),
        ]
        .iter()
        .any(|&(at, byte)| bytes.get(at) != Some(&byte))
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<u64> {
        let digits = text.get(range)?;
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| digits.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if year < 1970 || day == 0 || day > month_days || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // Days from 1970-01-01 to the date (the proleptic Gregorian calendar,
    // counted from March so the leap day ends each year).
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted / 400;
    let year_of_era = shifted % 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = (era * 146_097 + day_of_era).checked_sub(719_468)?;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Splits `gh`'s output into the total and the remaining lines.
fn total_and_lines(text: &str) -> Result<(u64, std::str::Lines<'_>), String> {
    let mut lines = text.lines();
    let total = lines
        .next()
        .and_then(|line| line.parse().ok())
        .ok_or("the API's output does not start with its total count")?;
    Ok((total, lines))
}

fn number(field: &str, what: &str) -> Result<u64, String> {
    field
        .parse()
        .map_err(|_| format!("{what} {field:?} is not a number"))
}

/// Reads [`RUNS_JQ`]'s output.
pub(super) fn parse_runs(text: &str) -> Result<Listing<Run>, String> {
    let (total, lines) = total_and_lines(text)?;
    let items = lines
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let &[id, attempt, status, event, branch, path, created] = fields.as_slice() else {
                return Err(format!("a run line has {} fields, not 7", fields.len()));
            };
            Ok(Run {
                id: number(id, "run id")?,
                attempt: number(attempt, "run attempt")?,
                status: status.to_owned(),
                event: event.to_owned(),
                branch: branch.to_owned(),
                path: path.to_owned(),
                created: parse_utc(created)
                    .ok_or_else(|| format!("run creation time {created:?} is not UTC"))?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(Listing { total, items })
}

/// Reads [`JOBS_JQ`]'s output.
pub(super) fn parse_jobs(text: &str) -> Result<Listing<Job>, String> {
    let (total, lines) = total_and_lines(text)?;
    let mut items: Vec<Job> = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match *fields.as_slice() {
            ["job", index, name, attempt] => {
                if u64::try_from(items.len()).ok() != Some(number(index, "job index")?) {
                    return Err(format!("job index {index} is out of order"));
                }
                items.push(Job {
                    name: name.to_owned(),
                    attempt: number(attempt, "job attempt")?,
                    steps: Vec::new(),
                });
            }
            ["step", index, name, conclusion] => {
                let index = number(index, "step's job index")?;
                let count = u64::try_from(items.len()).ok();
                let job = items
                    .last_mut()
                    .filter(|_| index.checked_add(1) == count)
                    .ok_or_else(|| format!("a step names job {index}, not the job before it"))?;
                job.steps.push(Step {
                    name: name.to_owned(),
                    conclusion: conclusion.to_owned(),
                });
            }
            _ => return Err(format!("unexpected line in the job listing: {line:?}")),
        }
    }
    Ok(Listing { total, items })
}
