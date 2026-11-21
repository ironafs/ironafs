// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `freshness --self-test`: for every tier of [`TIERS`], feeds planted
//! `gh api` output through the same readers and verdict as the live check,
//! and proves each planted failure (a run just past the tier's age, a
//! missing run, a failed run that did not run its item, a re-run attempt, a
//! partial run, a renamed job, a run of another workflow, branch or event,
//! malformed or truncated output, a bad manifest) ends as planted, beside
//! the clean cases that must pass, one of them just inside the tier's age.

use std::io::Write;

use super::github::{parse_jobs, parse_runs, parse_utc};
use super::{CHECK, DAY, TIERS, Tier, Verdict, evaluate, parse_manifest};
use crate::Error;

/// "Now" in every case: 2026-11-21T00:00:00Z.
const NOW: u64 = 1_795_219_200;

/// Each tier's planted times one second inside and one second past its age
/// limit; the self-test checks them against [`TIERS`], so a tier without a
/// row, or an age that drifted, fails it.
const LIMITS: &[(&str, &str, &str)] = &[
    ("nightly", "2026-11-19T00:00:01Z", "2026-11-18T23:59:59Z"),
    ("weekly", "2026-11-13T00:00:01Z", "2026-11-12T23:59:59Z"),
];

const MANIFEST: &str = "# planted manifest\nbuild\nadvisories\n";

/// Manifests that must be refused: empty, an id twice, ids that are not
/// lowercase words.
const BAD_MANIFESTS: &[&str] = &[
    "# nothing listed\n",
    "build\nbuild\n",
    "Build\n",
    "run build\n",
    "build;\n",
];

/// Attempt 1's jobs with both items run and passed.
const COMPLETE: &str = "2\njob\t0\tbuild\t1\nstep\t0\tSet up job\tsuccess\nstep\t0\trun build\tsuccess\n\
                        job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n";

/// How old a planted run is.
#[derive(Clone, Copy)]
enum Age {
    Hour,
    Day,
    /// One second inside the tier's limit.
    Inside,
    /// One second past the tier's limit.
    Past,
}

enum Expect {
    Fresh(u64),
    /// Stale, with a note containing this text.
    Stale(&'static str),
    /// Stale, with the tier's "no complete run" note.
    NoRecent,
    /// Unverified, with a reason containing this text.
    Unverified(&'static str),
}

struct Case {
    name: &'static str,
    runs: &'static [Line],
    /// The total the API claims, when it differs from the lines listed.
    total: Option<u64>,
    /// Planted run output used instead of `runs`.
    raw_runs: Option<&'static str>,
    jobs: &'static [(u64, &'static str)],
    expect: Expect,
}

/// One run as `gh api --jq` prints it; `path` is the tier's workflow
/// unless set.
struct Line {
    id: u64,
    attempt: u64,
    status: &'static str,
    event: &'static str,
    branch: &'static str,
    path: Option<&'static str>,
    age: Age,
}

const fn scheduled(id: u64, attempt: u64, age: Age) -> Line {
    Line {
        id,
        attempt,
        status: "completed",
        event: "schedule",
        branch: "main",
        path: None,
        age,
    }
}

const fn case(
    name: &'static str,
    runs: &'static [Line],
    jobs: &'static [(u64, &'static str)],
    expect: Expect,
) -> Case {
    Case {
        name,
        runs,
        total: None,
        raw_runs: None,
        jobs,
        expect,
    }
}

const CASES: &[Case] = &[
    case(
        "a complete first attempt an hour old",
        &[scheduled(7, 1, Age::Hour)],
        &[(7, COMPLETE)],
        Expect::Fresh(7),
    ),
    case(
        "a complete run one second inside the tier's age",
        &[scheduled(7, 1, Age::Inside)],
        &[(7, COMPLETE)],
        Expect::Fresh(7),
    ),
    case(
        "a complete run whose items failed (staleness and failure are separate)",
        &[scheduled(7, 1, Age::Hour)],
        &[(
            7,
            "2\njob\t0\tbuild\t1\nstep\t0\trun build\tfailure\n\
             job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        Expect::Fresh(7),
    ),
    case(
        "an incomplete newest run, with the previous run complete",
        &[scheduled(8, 1, Age::Hour), scheduled(7, 1, Age::Day)],
        &[
            (8, "1\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n"),
            (7, COMPLETE),
        ],
        Expect::Fresh(7),
    ),
    case(
        "planted: the only complete run is one second past the tier's age",
        &[scheduled(7, 1, Age::Past)],
        &[(7, COMPLETE)],
        Expect::NoRecent,
    ),
    case("planted: no run at all", &[], &[], Expect::NoRecent),
    case(
        "planted: a failed run whose job stopped before its item ran",
        &[scheduled(7, 1, Age::Hour)],
        &[(
            7,
            "2\njob\t0\tbuild\t1\nstep\t0\tSet up job\tfailure\nstep\t0\trun build\tskipped\n\
             job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        Expect::Stale("run 7: not run: build"),
    ),
    case(
        "planted: a job whose steps ran without its run step",
        &[scheduled(7, 1, Age::Hour)],
        &[(
            7,
            "2\njob\t0\tbuild\t1\nstep\t0\tSet up job\tsuccess\nstep\t0\tInstall Rust\tsuccess\n\
             job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        Expect::Stale("run 7: not run: build"),
    ),
    case(
        "planted: a renamed job, with the job count unchanged",
        &[scheduled(7, 1, Age::Hour)],
        &[(
            7,
            "2\njob\t0\tbuilt\t1\nstep\t0\trun built\tsuccess\n\
             job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        Expect::Stale("run 7: not run: build"),
    ),
    case(
        "planted: a second attempt completed what the first skipped",
        &[scheduled(7, 2, Age::Hour)],
        &[(
            7,
            "3\njob\t0\tbuild\t1\nstep\t0\trun build\tcancelled\n\
             job\t1\tbuild\t2\nstep\t1\trun build\tsuccess\n\
             job\t2\tadvisories\t1\nstep\t2\trun advisories\tsuccess\n",
        )],
        Expect::Stale("run 7: not run: build"),
    ),
    case(
        "planted: a listing holding only the second attempt's job for an item",
        &[scheduled(7, 2, Age::Hour)],
        &[(
            7,
            "2\njob\t0\tbuild\t2\nstep\t0\trun build\tsuccess\n\
             job\t1\tadvisories\t1\nstep\t1\trun advisories\tsuccess\n",
        )],
        Expect::Stale("run 7: not run: build"),
    ),
    case(
        "planted: every job reported, but one manifest item has no job",
        &[scheduled(7, 1, Age::Hour)],
        &[(7, "1\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n")],
        Expect::Stale("run 7: not run: advisories"),
    ),
    case(
        "planted: a complete run of another workflow",
        &[Line {
            path: Some(".github/workflows/ci.yml"),
            ..scheduled(7, 1, Age::Hour)
        }],
        &[(7, COMPLETE)],
        Expect::NoRecent,
    ),
    case(
        "planted: a complete run on another branch",
        &[Line {
            branch: "topic",
            ..scheduled(7, 1, Age::Hour)
        }],
        &[(7, COMPLETE)],
        Expect::NoRecent,
    ),
    case(
        "planted: a complete dispatched run, not a scheduled one",
        &[Line {
            event: "workflow_dispatch",
            ..scheduled(7, 1, Age::Hour)
        }],
        &[(7, COMPLETE)],
        Expect::NoRecent,
    ),
    Case {
        total: Some(250),
        ..case(
            "planted: a run list cut short before any complete run",
            &[Line {
                status: "in_progress",
                ..scheduled(7, 1, Age::Hour)
            }],
            &[],
            Expect::Unverified("only 1 of 250 runs listed"),
        )
    },
    case(
        "planted: a job list cut short",
        &[scheduled(7, 1, Age::Hour)],
        &[(7, "3\njob\t0\tbuild\t1\nstep\t0\trun build\tsuccess\n")],
        Expect::Unverified("run 7: 1 of 3 jobs listed"),
    ),
    case(
        "planted: a job list that is not the API's output",
        &[scheduled(7, 1, Age::Hour)],
        &[(7, "{\"message\": \"Not Found\"}\n")],
        Expect::Unverified("run 7: the API's output does not start"),
    ),
    Case {
        raw_runs: Some("1\n7\t1\tcompleted\tschedule\n"),
        ..case(
            "planted: a run list with a malformed line",
            &[],
            &[],
            Expect::Unverified("a run line has 4 fields, not 7"),
        )
    },
];

/// The planted times of one tier: an hour, a day, and its two limits.
struct Times {
    inside: &'static str,
    past: &'static str,
}

fn times(tier: &Tier) -> Result<Times, Error> {
    let (inside, past) = LIMITS
        .iter()
        .find_map(|&(name, inside, past)| (name == tier.name).then_some((inside, past)))
        .ok_or_else(|| {
            failure(
                "every tier has its planted limits",
                vec![tier.name.to_owned()],
            )
        })?;
    let limit = NOW.checked_sub(tier.max_age);
    if parse_utc(inside) != limit.map(|limit| limit + 1)
        || parse_utc(past) != limit.and_then(|limit| limit.checked_sub(1))
    {
        return Err(failure(
            "every tier's planted limits sit one second either side of its age",
            vec![format!("{}: {inside}, {past}", tier.name)],
        ));
    }
    Ok(Times { inside, past })
}

fn runs_text(tier: &Tier, times: &Times, case: &Case) -> String {
    if let Some(raw) = case.raw_runs {
        return raw.to_owned();
    }
    let total = case.total.unwrap_or(case.runs.len() as u64);
    case.runs.iter().fold(format!("{total}\n"), |text, line| {
        let at = match line.age {
            Age::Hour => "2026-11-20T23:00:00Z",
            Age::Day => "2026-11-20T00:00:00Z",
            Age::Inside => times.inside,
            Age::Past => times.past,
        };
        format!(
            "{text}{}\t{}\t{}\t{}\t{}\t{}\t{at}\n",
            line.id,
            line.attempt,
            line.status,
            line.event,
            line.branch,
            line.path.unwrap_or(tier.workflow),
        )
    })
}

fn verdict_of(tier: &Tier, times: &Times, case: &Case, manifest: &[String]) -> Verdict {
    let runs = match parse_runs(&runs_text(tier, times, case)) {
        Ok(runs) => runs,
        Err(reason) => return Verdict::Unverified(reason),
    };
    evaluate(tier, manifest, "main", NOW, &runs, &mut |id| {
        let text = case
            .jobs
            .iter()
            .find_map(|&(planted, text)| (planted == id).then_some(text))
            .ok_or_else(|| format!("no jobs planted for run {id}"))?;
        parse_jobs(text)
    })
}

fn ended_as_planted(tier: &Tier, verdict: &Verdict, expect: &Expect) -> bool {
    let no_recent = format!(
        "no complete run of {} in the last {} days",
        tier.name,
        tier.max_age / DAY
    );
    match (verdict, expect) {
        (Verdict::Fresh { run, .. }, Expect::Fresh(expected)) => run == expected,
        (Verdict::Stale(notes), Expect::Stale(note)) => {
            notes.iter().any(|line| line.contains(note))
        }
        (Verdict::Stale(notes), Expect::NoRecent) => notes.contains(&no_recent),
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
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report freshness self-test"));
    let manifest = parse_manifest(MANIFEST)
        .map_err(|reason| failure("the planted manifest reads", vec![reason]))?;
    let accepted: Vec<String> = BAD_MANIFESTS
        .iter()
        .filter(|text| parse_manifest(text).is_ok())
        .map(|text| (*text).to_owned())
        .collect();
    if !accepted.is_empty() {
        return Err(failure(
            "planted: manifests empty, with an id twice, or with ids not lowercase words",
            accepted,
        ));
    }
    report(format!(
        "passed: planted: {} bad manifests refused",
        BAD_MANIFESTS.len()
    ))?;
    if TIERS.is_empty() {
        return Err(failure("the check has tiers", Vec::new()));
    }
    for tier in TIERS {
        let times = times(tier)?;
        for case in CASES {
            let verdict = verdict_of(tier, &times, case, &manifest);
            if !ended_as_planted(tier, &verdict, &case.expect) {
                return Err(failure(
                    case.name,
                    vec![format!("{}: {verdict:?}", tier.name)],
                ));
            }
            report(format!("passed: {}: {}", tier.name, case.name))?;
        }
    }
    report(format!(
        "freshness --self-test: {} cases ended as planted",
        TIERS.len() * CASES.len() + 1
    ))
}
