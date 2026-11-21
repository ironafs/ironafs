// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `freshness --self-test`: feeds planted `gh api` output through the same
//! readers and verdict as the live check, and proves each planted failure
//! (a stale run, a missing run, a failed run that did not run its item, a
//! re-run attempt, a partial run, a run of another workflow, branch or
//! event, a truncated list) ends as planted, beside the clean cases that
//! must pass.

use std::io::Write;

use super::github::{parse_jobs, parse_runs};
use super::{CHECK, DAY, Tier, Verdict, evaluate, parse_manifest};
use crate::Error;

/// "Now" in every case: 2026-11-21T00:00:00Z.
const NOW: u64 = 1_795_219_200;

const TIER: Tier = Tier {
    name: "nightly",
    workflow: ".github/workflows/nightly.yml",
    max_age: 2 * DAY,
};

const MANIFEST: &str = "# planted manifest\nbuild\nadvisories\n";

/// Run lines: one hour, one day and three days before [`NOW`].
const HOUR_OLD: &str = "2026-11-20T23:00:00Z";
const DAY_OLD: &str = "2026-11-20T00:00:00Z";
const STALE: &str = "2026-11-18T00:00:00Z";

/// Attempt 1's jobs with both items run and passed.
const COMPLETE: &str = "2\njob\t0\tbuild\t1\nstep\t0\tSet up job\tsuccess\nstep\t0\trun build\tsuccess\n\
                        job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n";

enum Expect {
    Fresh(u64),
    /// Stale, with a note containing this text.
    Stale(&'static str),
    /// Unverified, with a reason containing this text.
    Unverified(&'static str),
}

struct Case {
    name: &'static str,
    runs: &'static [Line],
    /// The total the API claims, when it differs from the lines listed.
    total: Option<u64>,
    jobs: &'static [(u64, &'static str)],
    expect: Expect,
}

/// One run as `gh api --jq` prints it.
struct Line {
    id: u64,
    attempt: u64,
    status: &'static str,
    event: &'static str,
    branch: &'static str,
    path: &'static str,
    at: &'static str,
}

const fn scheduled(id: u64, attempt: u64, at: &'static str) -> Line {
    Line {
        id,
        attempt,
        status: "completed",
        event: "schedule",
        branch: "main",
        path: TIER.workflow,
        at,
    }
}

fn runs_text(case: &Case) -> String {
    let total = case.total.unwrap_or(case.runs.len() as u64);
    case.runs.iter().fold(format!("{total}\n"), |text, line| {
        format!(
            "{text}{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            line.id, line.attempt, line.status, line.event, line.branch, line.path, line.at
        )
    })
}

const NO_RECENT: &str = "no complete run of nightly in the last 2 days";

const CASES: &[Case] = &[
    Case {
        name: "a complete first attempt an hour old",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(7, COMPLETE)],
        expect: Expect::Fresh(7),
    },
    Case {
        name: "a complete run whose items failed (staleness and failure are separate)",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(
            7,
            "2\njob\t0\tbuild\t1\nstep\t0\trun build\tfailure\n\
                 job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        expect: Expect::Fresh(7),
    },
    Case {
        name: "an incomplete newest run, with the previous run complete",
        runs: &[scheduled(8, 1, HOUR_OLD), scheduled(7, 1, DAY_OLD)],
        total: None,
        jobs: &[
            (8, "1\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n"),
            (7, COMPLETE),
        ],
        expect: Expect::Fresh(7),
    },
    Case {
        name: "planted: the only complete run is three days old",
        runs: &[scheduled(7, 1, STALE)],
        total: None,
        jobs: &[(7, COMPLETE)],
        expect: Expect::Stale(NO_RECENT),
    },
    Case {
        name: "planted: no run at all",
        runs: &[],
        total: None,
        jobs: &[],
        expect: Expect::Stale(NO_RECENT),
    },
    Case {
        name: "planted: a failed run whose job stopped before its item ran",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(
            7,
            "2\njob\t0\tbuild\t1\nstep\t0\tSet up job\tfailure\nstep\t0\trun build\tskipped\n\
                 job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        expect: Expect::Stale("run 7: not run: build"),
    },
    Case {
        name: "planted: a second attempt completed what the first skipped",
        runs: &[scheduled(7, 2, HOUR_OLD)],
        total: None,
        jobs: &[(
            7,
            "3\njob\t0\tbuild\t1\nstep\t0\trun build\tcancelled\n\
                 job\t1\tbuild\t2\nstep\t1\trun build\tsuccess\n\
                 job\t2\tadvisories\t1\nstep\t2\trun advisories\tsuccess\n",
        )],
        expect: Expect::Stale("run 7: not run: build"),
    },
    Case {
        name: "planted: a listing holding only the second attempt's job for an item",
        runs: &[scheduled(7, 2, HOUR_OLD)],
        total: None,
        jobs: &[(
            7,
            "2\njob\t0\tbuild\t2\nstep\t0\trun build\tsuccess\n\
                 job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        expect: Expect::Stale("run 7: not run: build"),
    },
    Case {
        name: "planted: every job reported, but one manifest item has no job",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(7, "1\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n")],
        expect: Expect::Stale("run 7: not run: advisories"),
    },
    Case {
        name: "planted: a complete run of another workflow",
        runs: &[Line {
            path: ".github/workflows/weekly.yml",
            ..scheduled(7, 1, HOUR_OLD)
        }],
        total: None,
        jobs: &[(7, COMPLETE)],
        expect: Expect::Stale(NO_RECENT),
    },
    Case {
        name: "planted: a complete run on another branch",
        runs: &[Line {
            branch: "topic",
            ..scheduled(7, 1, HOUR_OLD)
        }],
        total: None,
        jobs: &[(7, COMPLETE)],
        expect: Expect::Stale(NO_RECENT),
    },
    Case {
        name: "planted: a complete dispatched run, not a scheduled one",
        runs: &[Line {
            event: "workflow_dispatch",
            ..scheduled(7, 1, HOUR_OLD)
        }],
        total: None,
        jobs: &[(7, COMPLETE)],
        expect: Expect::Stale(NO_RECENT),
    },
    Case {
        name: "planted: a run list cut short before any complete run",
        runs: &[Line {
            status: "in_progress",
            ..scheduled(7, 1, HOUR_OLD)
        }],
        total: Some(250),
        jobs: &[],
        expect: Expect::Unverified("only 1 of 250 runs listed"),
    },
    Case {
        name: "planted: a job list cut short",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(7, "3\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n")],
        expect: Expect::Unverified("run 7: 1 of 3 jobs listed"),
    },
    Case {
        name: "planted: a job list that is not the API's output",
        runs: &[scheduled(7, 1, HOUR_OLD)],
        total: None,
        jobs: &[(7, "{\"message\": \"Not Found\"}\n")],
        expect: Expect::Unverified("run 7: the API's output does not start"),
    },
];

fn verdict_of(case: &Case, manifest: &[String]) -> Verdict {
    let runs = match parse_runs(&runs_text(case)) {
        Ok(runs) => runs,
        Err(reason) => return Verdict::Unverified(reason),
    };
    evaluate(&TIER, manifest, "main", NOW, &runs, &mut |id| {
        let text = case
            .jobs
            .iter()
            .find_map(|&(planted, text)| (planted == id).then_some(text))
            .ok_or_else(|| format!("no jobs planted for run {id}"))?;
        parse_jobs(text)
    })
}

fn ended_as_planted(verdict: &Verdict, expect: &Expect) -> bool {
    match (verdict, expect) {
        (Verdict::Fresh { run, .. }, Expect::Fresh(expected)) => run == expected,
        (Verdict::Stale(notes), Expect::Stale(note)) => {
            notes.iter().any(|line| line.contains(note))
        }
        (Verdict::Unverified(reason), Expect::Unverified(text)) => reason.contains(text),
        _ => false,
    }
}

fn failure(case: &'static str, found: Vec<String>) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found,
    }
}

pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let manifest = parse_manifest(MANIFEST)
        .map_err(|reason| failure("the planted manifest reads", vec![reason]))?;
    for text in ["# nothing listed\n", "build\nbuild\n"] {
        if parse_manifest(text).is_ok() {
            return Err(failure(
                "planted: an empty manifest, and one listing an id twice",
                vec![text.to_owned()],
            ));
        }
    }
    writeln!(
        output,
        "passed: planted: an empty manifest, and one listing an id twice"
    )
    .map_err(Error::io("report freshness self-test"))?;
    for case in CASES {
        let verdict = verdict_of(case, &manifest);
        if !ended_as_planted(&verdict, &case.expect) {
            return Err(failure(case.name, vec![format!("{verdict:?}")]));
        }
        writeln!(output, "passed: {}", case.name)
            .map_err(Error::io("report freshness self-test"))?;
    }
    writeln!(
        output,
        "freshness --self-test: {} cases ended as planted",
        CASES.len() + 1
    )
    .map_err(Error::io("report freshness self-test"))
}
