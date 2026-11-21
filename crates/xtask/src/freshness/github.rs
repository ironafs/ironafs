// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads workflow runs and their first attempt's jobs from the GitHub API
//! through `gh api --jq`, which prints the fields the check needs as
//! tab-separated lines, so no JSON or HTTP crate is needed (coding standards
//! rule 21). `gh` takes its token from `GH_TOKEN` in CI; nothing here reads
//! or prints it.

use std::io::Read;
use std::process::{Command, Stdio};

use super::{Job, Listing, Run, Step};

#[cfg(test)]
mod tests;

/// Items per page: the API's largest page.
const PAGE: usize = 100;

/// The most pages read of one list: the API's 1,000-result limit for a
/// filtered search. A list still short after them is refused, not read.
pub(super) const MAX_PAGES: u64 = 10;

/// The most steps one job may list (GitHub's own limit per job).
pub(super) const MAX_STEPS: usize = 1_000;

/// The most bytes one page of `gh` output may hold.
pub(super) const MAX_OUTPUT: u64 = 4 << 20;

/// The total, then one line per run: id, latest attempt, status, event,
/// branch, workflow file and creation time.
const RUNS_JQ: &str = ".total_count, (.workflow_runs[] | [.id, .run_attempt, .status, .event, \
                       .head_branch, .path, .created_at] | @tsv)";

/// The total, then each job (`job`, index, name, attempt) followed by its
/// steps (`step`, the job's index, name, conclusion or empty).
const JOBS_JQ: &str = ".total_count, (.jobs | to_entries[] | .key as $k | .value as $j \
                       | ([\"job\", $k, $j.name, $j.run_attempt] | @tsv), \
                       ($j.steps[]? | [\"step\", $k, .name, (.conclusion // \"\")] | @tsv))";

/// Lists the scheduled runs of `workflow` on `branch`, newest first, page
/// by page until one holds a run created before `oldest` (every later page
/// is older still), the list ends, or [`MAX_PAGES`] were read.
///
/// `gh` resolves `{owner}/{repo}` from `GH_REPO` or the checkout's remote.
pub(super) fn runs(workflow: &str, branch: &str, oldest: u64) -> Result<Listing<Run>, String> {
    let file = workflow.rsplit('/').next().unwrap_or(workflow);
    let mut listing = collect_pages(
        &mut |page| {
            let text = gh(&[
                &format!("repos/{{owner}}/{{repo}}/actions/workflows/{file}/runs"),
                "-f",
                &format!("branch={branch}"),
                "-f",
                "event=schedule",
                "-f",
                &format!("per_page={PAGE}"),
                "-f",
                &format!("page={page}"),
                "--jq",
                RUNS_JQ,
            ])?;
            parse_runs(&text)
        },
        &|runs: &[Run]| runs.last().is_some_and(|run| run.created < oldest),
    )?;
    // A run created between two page reads shifts the pages by one.
    let mut seen = Vec::new();
    listing.items.retain(|run| {
        let first = !seen.contains(&run.id);
        seen.push(run.id);
        first
    });
    Ok(listing)
}

/// Lists every job of the first attempt of run `id`.
pub(super) fn first_attempt_jobs(id: u64) -> Result<Listing<Job>, String> {
    collect_pages(
        &mut |page| {
            let text = gh(&[
                &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}/attempts/1/jobs"),
                "-f",
                &format!("per_page={PAGE}"),
                "-f",
                &format!("page={page}"),
                "--jq",
                JOBS_JQ,
            ])?;
            parse_jobs(&text)
        },
        &|_: &[Job]| false,
    )
}

/// Reads pages 1, 2, ... of a list until it holds the first page's total,
/// a page comes back empty, `enough` says so, or [`MAX_PAGES`] were read.
/// The total stays the first page's, so a list left short shows it.
pub(super) fn collect_pages<T>(
    fetch: &mut dyn FnMut(u64) -> Result<Listing<T>, String>,
    enough: &dyn Fn(&[T]) -> bool,
) -> Result<Listing<T>, String> {
    let mut listing: Option<Listing<T>> = None;
    for page in 1..=MAX_PAGES {
        let next = fetch(page)?;
        let listing = listing.get_or_insert(Listing {
            total: next.total,
            items: Vec::new(),
        });
        if next.items.is_empty() {
            break;
        }
        listing.items.extend(next.items);
        let read = u64::try_from(listing.items.len()).unwrap_or(u64::MAX);
        if read >= listing.total || enough(&listing.items) {
            break;
        }
    }
    listing.ok_or_else(|| "no page was read".to_owned())
}

fn gh(arguments: &[&str]) -> Result<String, String> {
    let mut child = Command::new("gh")
        .args(["api", "--method", "GET"])
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("cannot run gh: {error}"))?;
    let text = child
        .stdout
        .take()
        .ok_or_else(|| "gh api gave no output pipe".to_owned())
        .and_then(|stdout| read_capped(stdout, MAX_OUTPUT));
    // Reaped even when the output was refused; a refused read ends the
    // pipe, so gh stops on its next write.
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for gh: {error}"))?;
    let text = text?;
    if !status.success() {
        return Err(format!("gh api failed with {status}"));
    }
    Ok(text)
}

/// Reads at most `cap` bytes of UTF-8, refusing anything longer.
pub(super) fn read_capped(reader: impl Read, cap: u64) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read gh api's output: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(format!("gh api printed more than {cap} bytes"));
    }
    String::from_utf8(bytes).map_err(|_| "gh api printed invalid UTF-8".to_owned())
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
    if lines.clone().count() > PAGE {
        return Err(format!("the API returned more than {PAGE} runs in a page"));
    }
    let items = lines
        .map(|line| {
            let fields: Vec<&str> = line.splitn(8, '\t').collect();
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
        let fields: Vec<&str> = line.splitn(5, '\t').collect();
        match *fields.as_slice() {
            ["job", index, name, attempt] => {
                if items.len() == PAGE {
                    return Err(format!("the API returned more than {PAGE} jobs in a page"));
                }
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
                if job.steps.len() == MAX_STEPS {
                    return Err(format!("job {index} lists more than {MAX_STEPS} steps"));
                }
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
