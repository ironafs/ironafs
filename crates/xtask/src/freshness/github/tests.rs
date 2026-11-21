// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading `gh api --jq` output and API timestamps.

use super::{parse_jobs, parse_runs, parse_utc};
use crate::freshness::{Job, Listing, Run, Step};

#[test]
fn parse_utc_reads_seconds_since_the_epoch() {
    assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_utc("2000-03-01T00:00:00Z"), Some(951_868_800));
    assert_eq!(parse_utc("2026-11-21T03:12:45Z"), Some(1_795_230_765));
}

#[test]
fn parse_utc_rejects_other_forms() {
    for text in [
        "",
        "2026-11-21",
        "2026-11-21T03:12:45",
        "2026-11-21T03:12:45+00:00",
        "2026-11-21T03:12:45.5Z",
        "2026-13-01T00:00:00Z",
        "2026-02-30T00:00:00Z",
        "2026-11-21T24:00:00Z",
        "2026-11-21T03:60:00Z",
        "1969-12-31T23:59:59Z",
        "2026-1x-21T03:12:45Z",
    ] {
        assert_eq!(parse_utc(text), None, "{text}");
    }
    assert_eq!(parse_utc("2024-02-29T00:00:00Z"), Some(1_709_164_800));
    assert_eq!(parse_utc("2025-02-29T00:00:00Z"), None);
}

#[test]
fn parse_runs_reads_the_total_and_each_run() {
    let text = "2\n\
        11\t1\tcompleted\tschedule\tmain\t.github/workflows/nightly.yml\t2026-11-21T03:12:45Z\n\
        10\t2\tin_progress\tschedule\tmain\t.github/workflows/nightly.yml\t1970-01-01T00:00:00Z\n";
    assert_eq!(
        parse_runs(text),
        Ok(Listing {
            total: 2,
            items: vec![
                Run {
                    id: 11,
                    attempt: 1,
                    status: "completed".into(),
                    event: "schedule".into(),
                    branch: "main".into(),
                    path: ".github/workflows/nightly.yml".into(),
                    created: 1_795_230_765,
                },
                Run {
                    id: 10,
                    attempt: 2,
                    status: "in_progress".into(),
                    event: "schedule".into(),
                    branch: "main".into(),
                    path: ".github/workflows/nightly.yml".into(),
                    created: 0,
                },
            ],
        })
    );
}

#[test]
fn parse_runs_rejects_malformed_output() {
    for text in [
        "",
        "x\n",
        "1\n11\t1\tcompleted\tschedule\tmain\t.github/workflows/nightly.yml\n",
        "1\n11\t1\tcompleted\tschedule\tmain\tp\t2026-11-21T03:12:45Z\textra\n",
        "1\nx\t1\tcompleted\tschedule\tmain\tp\t2026-11-21T03:12:45Z\n",
        "1\n11\t1\tcompleted\tschedule\tmain\tp\tyesterday\n",
    ] {
        assert!(
            parse_runs(text).is_err_and(|error| !error.is_empty()),
            "{text:?}"
        );
    }
}

#[test]
fn parse_jobs_reads_jobs_and_their_steps() {
    let text = "2\n\
        job\t0\tbuild\t1\n\
        step\t0\tSet up job\tsuccess\n\
        step\t0\trun build\tfailure\n\
        job\t1\tadvisories\t1\n";
    assert_eq!(
        parse_jobs(text),
        Ok(Listing {
            total: 2,
            items: vec![
                Job {
                    name: "build".into(),
                    attempt: 1,
                    steps: vec![
                        Step {
                            name: "Set up job".into(),
                            conclusion: "success".into(),
                        },
                        Step {
                            name: "run build".into(),
                            conclusion: "failure".into(),
                        },
                    ],
                },
                Job {
                    name: "advisories".into(),
                    attempt: 1,
                    steps: vec![],
                },
            ],
        })
    );
}

#[test]
fn parse_jobs_rejects_malformed_output() {
    for text in [
        "",
        "1\nstep\t0\trun build\tsuccess\n",
        "1\njob\t0\tbuild\t1\nstep\t1\trun build\tsuccess\n",
        "1\njob\t1\tbuild\t1\n",
        "1\njob\t0\tbuild\n",
        "1\nwhat\t0\tbuild\t1\n",
    ] {
        assert!(
            parse_jobs(text).is_err_and(|error| !error.is_empty()),
            "{text:?}"
        );
    }
}
