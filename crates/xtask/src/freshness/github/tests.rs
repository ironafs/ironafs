// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading `gh api --jq` output and API timestamps.

use super::{MAX_PAGES, MAX_STEPS, collect_pages, parse_jobs, parse_runs, parse_utc, read_capped};
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

fn run_line(id: u64) -> String {
    format!("{id}\t1\tcompleted\tschedule\tmain\tp\t2026-11-21T03:12:45Z\n")
}

#[test]
fn parse_runs_rejects_more_than_a_page() {
    let page: String = (0..100).map(run_line).collect();
    assert!(parse_runs(&format!("100\n{page}")).is_ok());
    let over: String = (0..101).map(run_line).collect();
    assert_eq!(
        parse_runs(&format!("101\n{over}")),
        Err("the API returned more than 100 runs in a page".to_owned())
    );
}

fn job_line(index: usize) -> String {
    format!("job\t{index}\tj{index}\t1\n")
}

fn step_line(index: usize) -> String {
    format!("step\t0\ts{index}\tsuccess\n")
}

#[test]
fn parse_jobs_rejects_more_than_a_page_or_too_many_steps() {
    let page: String = (0..100).map(job_line).collect();
    assert!(parse_jobs(&format!("100\n{page}")).is_ok());
    let over: String = (0..101).map(job_line).collect();
    assert_eq!(
        parse_jobs(&format!("101\n{over}")),
        Err("the API returned more than 100 jobs in a page".to_owned())
    );
    let steps: String = (0..=MAX_STEPS).map(step_line).collect();
    assert_eq!(
        parse_jobs(&format!("1\njob\t0\tbuild\t1\n{steps}")),
        Err(format!("job 0 lists more than {MAX_STEPS} steps"))
    );
}

#[test]
fn read_capped_refuses_output_over_the_cap() {
    let exact = vec![b'a'; 8];
    assert_eq!(read_capped(exact.as_slice(), 8), Ok("aaaaaaaa".to_owned()));
    let over = vec![b'a'; 9];
    assert_eq!(
        read_capped(over.as_slice(), 8),
        Err("gh api printed more than 8 bytes".to_owned())
    );
    assert_eq!(
        read_capped(&[0xff_u8][..], 8),
        Err("gh api printed invalid UTF-8".to_owned())
    );
}

fn page_of(total: u64, ids: std::ops::Range<u64>) -> Listing<u64> {
    Listing {
        total,
        items: ids.collect(),
    }
}

#[test]
fn collect_pages_reads_until_the_total() {
    let mut asked = Vec::new();
    let listing = collect_pages(
        &mut |page| {
            asked.push(page);
            Ok(match page {
                1 => page_of(250, 0..100),
                2 => page_of(250, 100..200),
                _ => page_of(250, 200..250),
            })
        },
        &|_: &[u64]| false,
    );
    assert_eq!(asked, [1, 2, 3]);
    assert_eq!(listing, Ok(page_of(250, 0..250)));
}

#[test]
fn collect_pages_stops_when_the_caller_has_enough() {
    let mut asked = Vec::new();
    let listing = collect_pages(
        &mut |page| {
            asked.push(page);
            Ok(page_of(250, (page - 1) * 100..page * 100))
        },
        &|items: &[u64]| items.len() >= 100,
    );
    assert_eq!(asked, [1]);
    assert_eq!(listing, Ok(page_of(250, 0..100)));
}

#[test]
fn collect_pages_stops_at_the_page_cap_leaving_the_list_short() {
    let mut asked = 0;
    let listing = collect_pages(
        &mut |page| {
            asked += 1;
            Ok(page_of(5_000, (page - 1) * 100..page * 100))
        },
        &|_: &[u64]| false,
    )
    .unwrap();
    assert_eq!(asked, MAX_PAGES);
    assert_eq!(listing.total, 5_000);
    assert_eq!(listing.items.len(), 1_000);
}

#[test]
fn collect_pages_stops_on_an_empty_page_and_passes_errors_on() {
    let mut asked = Vec::new();
    let listing = collect_pages(
        &mut |page| {
            asked.push(page);
            Ok(if page == 1 {
                page_of(300, 0..100)
            } else {
                page_of(300, 0..0)
            })
        },
        &|_: &[u64]| false,
    );
    assert_eq!(asked, [1, 2]);
    assert_eq!(listing, Ok(page_of(300, 0..100)));
    assert_eq!(
        collect_pages(
            &mut |_| Err::<Listing<u64>, _>("refused".to_owned()),
            &|_: &[u64]| false
        ),
        Err("refused".to_owned())
    );
}
