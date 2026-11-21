// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the tier manifests and the freshness verdict.

use std::collections::BTreeMap;

use super::{
    DAY, Job, Listing, Run, Step, TIERS, Tier, Verdict, could_not_verify, evaluate, parse_manifest,
    self_test,
};

const NIGHTLY: Tier = Tier {
    name: "nightly",
    workflow: ".github/workflows/nightly.yml",
    max_age: 2 * DAY,
};

/// "Now" in every case: 2026-11-21T00:00:00Z.
const NOW: u64 = 1_795_219_200;
const HOUR: u64 = 3_600;

fn run(id: u64, attempt: u64, age: u64) -> Run {
    Run {
        id,
        attempt,
        status: "completed".into(),
        event: "schedule".into(),
        branch: "main".into(),
        path: NIGHTLY.workflow.into(),
        created: NOW - age,
    }
}

fn job(name: &str, run_step: Option<&str>) -> Job {
    let mut steps = vec![Step {
        name: "Set up job".into(),
        conclusion: "success".into(),
    }];
    if let Some(conclusion) = run_step {
        steps.push(Step {
            name: format!("run {name}"),
            conclusion: conclusion.into(),
        });
    }
    Job {
        name: name.into(),
        attempt: 1,
        steps,
    }
}

fn complete() -> Vec<Job> {
    vec![
        job("build", Some("success")),
        job("advisories", Some("success")),
    ]
}

fn manifest() -> Vec<String> {
    vec!["build".into(), "advisories".into()]
}

fn listing<T>(items: Vec<T>) -> Listing<T> {
    let total = items.len().try_into().unwrap();
    Listing { total, items }
}

#[track_caller]
fn verdict(runs: &Listing<Run>, jobs: &BTreeMap<u64, Listing<Job>>) -> Verdict {
    evaluate(&NIGHTLY, &manifest(), "main", NOW, runs, &mut |id| {
        jobs.get(&id)
            .cloned()
            .ok_or_else(|| format!("no jobs planted for run {id}"))
    })
}

#[track_caller]
fn assert_stale(verdict: &Verdict, note: &str) {
    match verdict {
        Verdict::Stale(notes) => assert!(
            notes.iter().any(|line| line.contains(note)),
            "no note containing {note:?} in {notes:?}"
        ),
        other => panic!("expected stale, got {other:?}"),
    }
}

#[test]
fn freshness_passes_on_recent_complete_first_attempt() {
    let verdict = verdict(
        &listing(vec![run(7, 1, 3 * HOUR)]),
        &BTreeMap::from([(7, listing(complete()))]),
    );
    assert_eq!(
        verdict,
        Verdict::Fresh {
            run: 7,
            age: 3 * HOUR
        }
    );
}

#[test]
fn nightly_freshness_fails_when_stale() {
    let verdict = verdict(
        &listing(vec![run(7, 1, 2 * DAY + 1)]),
        &BTreeMap::from([(7, listing(complete()))]),
    );
    assert_stale(&verdict, "no complete run of nightly in the last 2 days");
}

#[test]
fn freshness_fails_when_no_run_exists() {
    let verdict = verdict(&listing(vec![]), &BTreeMap::new());
    assert_stale(&verdict, "no complete run of nightly in the last 2 days");
}

#[test]
fn freshness_ignores_rerun_attempt() {
    // Attempt 1 skipped the build; a re-run completed it. The jobs listed
    // are attempt 1's, and a job carrying another attempt never counts.
    let mut rerun = job("build", Some("success"));
    rerun.attempt = 2;
    let verdict = verdict(
        &listing(vec![run(7, 2, HOUR)]),
        &BTreeMap::from([(
            7,
            listing(vec![
                job("build", Some("skipped")),
                rerun,
                job("advisories", Some("success")),
            ]),
        )]),
    );
    assert_stale(&verdict, "run 7: not run: build");
}

#[test]
fn freshness_refuses_truncated_page() {
    let mut runs = listing(vec![run(7, 1, HOUR)]);
    runs.items.first_mut().unwrap().status = "in_progress".into();
    runs.total = 101;
    let verdict = verdict(&runs, &BTreeMap::new());
    assert!(
        matches!(&verdict, Verdict::Unverified(reason) if reason.contains("1 of 101 runs")),
        "{verdict:?}"
    );
}

#[test]
fn freshness_decides_on_a_truncated_page_once_a_run_is_too_old() {
    let mut runs = listing(vec![run(7, 1, 3 * DAY)]);
    runs.total = 500;
    let verdict = verdict(&runs, &BTreeMap::new());
    assert_stale(&verdict, "no complete run of nightly in the last 2 days");
}

#[test]
fn freshness_refuses_truncated_job_list() {
    let mut jobs = listing(complete());
    jobs.total = 3;
    let verdict = verdict(
        &listing(vec![run(7, 1, HOUR)]),
        &BTreeMap::from([(7, jobs)]),
    );
    assert!(
        matches!(&verdict, Verdict::Unverified(reason) if reason.contains("run 7: 2 of 3 jobs")),
        "{verdict:?}"
    );
}

#[test]
fn freshness_refuses_when_jobs_cannot_be_read() {
    let verdict = verdict(&listing(vec![run(7, 1, HOUR)]), &BTreeMap::new());
    assert!(
        matches!(&verdict, Verdict::Unverified(reason) if reason.contains("no jobs planted for run 7")),
        "{verdict:?}"
    );
}

#[test]
fn freshness_fresh_on_failed_complete_run() {
    let verdict = verdict(
        &listing(vec![run(7, 1, HOUR)]),
        &BTreeMap::from([(
            7,
            listing(vec![
                job("build", Some("failure")),
                job("advisories", Some("success")),
            ]),
        )]),
    );
    assert_eq!(verdict, Verdict::Fresh { run: 7, age: HOUR });
}

#[test]
fn freshness_never_counts_failed_run_whose_item_did_not_run() {
    // The job failed before its run step, which GitHub then skipped.
    let verdict = verdict(
        &listing(vec![run(7, 1, HOUR)]),
        &BTreeMap::from([(
            7,
            listing(vec![job("build", None), job("advisories", Some("success"))]),
        )]),
    );
    assert_stale(&verdict, "run 7: not run: build");
}

#[test]
fn freshness_incomplete_when_manifest_item_not_run() {
    // Every job reported, but no job ran the manifest's `advisories`.
    let verdict = verdict(
        &listing(vec![run(7, 1, HOUR)]),
        &BTreeMap::from([(7, listing(vec![job("build", Some("success"))]))]),
    );
    assert_stale(&verdict, "run 7: not run: advisories");
}

#[test]
fn freshness_incomplete_when_one_of_two_same_named_jobs_skipped() {
    let verdict = verdict(
        &listing(vec![run(7, 1, HOUR)]),
        &BTreeMap::from([(
            7,
            listing(vec![
                job("build", Some("success")),
                job("build", Some("cancelled")),
                job("advisories", Some("success")),
            ]),
        )]),
    );
    assert_stale(&verdict, "run 7: not run: build");
}

#[test]
fn freshness_counts_the_previous_complete_run() {
    let verdict = verdict(
        &listing(vec![run(8, 1, HOUR), run(7, 1, DAY)]),
        &BTreeMap::from([
            (8, listing(vec![job("build", Some("success"))])),
            (7, listing(complete())),
        ]),
    );
    assert_eq!(verdict, Verdict::Fresh { run: 7, age: DAY });
}

#[test]
fn freshness_skips_a_run_still_in_progress() {
    let mut current = run(8, 1, HOUR);
    current.status = "in_progress".into();
    let verdict = verdict(
        &listing(vec![current, run(7, 1, DAY)]),
        &BTreeMap::from([(7, listing(complete()))]),
    );
    assert_eq!(verdict, Verdict::Fresh { run: 7, age: DAY });
}

#[test]
fn freshness_ignores_runs_of_another_workflow_branch_or_event() {
    let mut other_workflow = run(7, 1, HOUR);
    other_workflow.path = ".github/workflows/weekly.yml".into();
    let mut other_branch = run(8, 1, HOUR);
    other_branch.branch = "topic".into();
    let mut dispatched = run(9, 1, HOUR);
    dispatched.event = "workflow_dispatch".into();
    for planted in [other_workflow, other_branch, dispatched] {
        let id = planted.id;
        let verdict = verdict(
            &listing(vec![planted]),
            &BTreeMap::from([(id, listing(complete()))]),
        );
        assert_stale(&verdict, "no complete run of nightly in the last 2 days");
    }
}

#[test]
fn freshness_orders_runs_by_creation_not_by_listing() {
    // An old complete run listed first must not hide that the newer one is
    // incomplete, nor stop the scan before it.
    let verdict = verdict(
        &listing(vec![run(5, 1, 3 * DAY), run(8, 1, HOUR)]),
        &BTreeMap::from([(8, listing(complete()))]),
    );
    assert_eq!(verdict, Verdict::Fresh { run: 8, age: HOUR });
}

#[test]
fn parse_manifest_reads_ids_and_skips_comments() {
    assert_eq!(
        parse_manifest("# SPDX\n\nbuild\n  # slot: fuzz\nfuzz-xdr-decode\n"),
        Ok(vec!["build".to_owned(), "fuzz-xdr-decode".to_owned()])
    );
}

#[test]
fn parse_manifest_rejects_empty_duplicate_and_odd_ids() {
    for text in [
        "",
        "# only a comment\n",
        "build\nbuild\n",
        "Build\n",
        "a b\n",
        "run build\n",
    ] {
        assert!(
            parse_manifest(text).is_err_and(|error| !error.is_empty()),
            "{text:?}"
        );
    }
}

#[test]
fn tiers_name_their_workflows_and_ages() {
    let tiers: Vec<_> = TIERS
        .iter()
        .map(|tier| (tier.name, tier.workflow, tier.max_age))
        .collect();
    assert_eq!(
        tiers,
        [
            ("nightly", ".github/workflows/nightly.yml", 2 * DAY),
            ("weekly", ".github/workflows/weekly.yml", 8 * DAY),
        ]
    );
}

#[test]
fn freshness_self_test_ends_every_case_as_planted() {
    let mut output = Vec::new();
    self_test::run(&mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.ends_with("freshness --self-test: 39 cases ended as planted\n"),
        "{output}"
    );
}

/// What a scheduled workflow must not do (`docs/specs/testing.md`, 5.5,
/// points 1, 2 and 4), read from its text: every job is a manifest id with
/// its `run <id>` step and only `contents: read`; no concurrency (so no
/// cancellation), no secret or token, no artefact and no cache.
fn scheduled_workflow_violations(workflow: &str, manifest: &[String]) -> Vec<String> {
    let lines: Vec<&str> = workflow
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .collect();
    let mut found = Vec::new();
    for banned in [
        "concurrency",
        "secrets.",
        "github.token",
        "GITHUB_TOKEN",
        "upload-artifact",
        "actions/cache",
    ] {
        if lines.iter().any(|line| line.contains(banned)) {
            found.push(format!("uses {banned}"));
        }
    }
    // Every `permissions` key, in any form, is either the empty top-level
    // map or a job's block granting `contents: read` alone.
    for (index, line) in lines.iter().enumerate() {
        let Some((indent, value)) = line.split_once("permissions:") else {
            continue;
        };
        let value = value.trim();
        if indent.is_empty() && value == "{}" {
            continue;
        }
        let granted: Vec<&str> = lines
            .iter()
            .skip(index + 1)
            .take_while(|next| next.len() - next.trim_start().len() > indent.len())
            .map(|next| next.trim())
            .collect();
        if !value.is_empty() || indent != "    " || granted != ["contents: read"] {
            found.push(format!(
                "permissions at line {index} grant {value:?} {granted:?}"
            ));
        }
    }
    if !lines.contains(&"permissions: {}") {
        found.push("no empty top-level permissions".to_owned());
    }
    // Every job (a two-space key under `jobs:`) is a manifest id, named as
    // its key, with its own permissions block and its `run <id>` step.
    let Some(start) = lines.iter().position(|line| *line == "jobs:") else {
        found.push("no jobs".to_owned());
        return found;
    };
    let mut keys = Vec::new();
    let body: Vec<&str> = lines.iter().skip(start + 1).copied().collect();
    for (index, line) in body.iter().enumerate() {
        let Some(key) = line
            .strip_prefix("  ")
            .and_then(|rest| rest.strip_suffix(':'))
        else {
            continue;
        };
        if key.starts_with(' ') {
            continue;
        }
        keys.push(key);
        let block: Vec<&str> = body
            .iter()
            .skip(index + 1)
            .take_while(|next| next.starts_with("    "))
            .copied()
            .collect();
        for required in [
            format!("    name: {key}"),
            "    permissions:".to_owned(),
            format!("      - name: run {key}"),
        ] {
            if !block.contains(&required.as_str()) {
                found.push(format!("job {key} lacks {:?}", required.trim()));
            }
        }
    }
    keys.sort_unstable();
    let mut expected: Vec<&str> = manifest.iter().map(String::as_str).collect();
    expected.sort_unstable();
    if keys != expected {
        found.push(format!(
            "jobs {keys:?} differ from the manifest {expected:?}"
        ));
    }
    found
}

fn repository_file(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn scheduled_workflows_follow_their_manifests_and_the_rules() {
    for tier in TIERS {
        let manifest =
            parse_manifest(&repository_file(&format!("tests/tiers/{}.txt", tier.name))).unwrap();
        let workflow = repository_file(tier.workflow);
        assert_eq!(
            scheduled_workflow_violations(&workflow, &manifest),
            Vec::<String>::new(),
            "{}",
            tier.workflow
        );
        assert!(workflow.contains("\n  schedule:\n"), "{}", tier.workflow);
    }
}

#[test]
fn scheduled_workflow_rules_catch_planted_violations() {
    let manifest = vec!["build".to_owned(), "fuzz-xdr".to_owned()];
    let planted = "permissions:\n  contents: write\n\
                   concurrency:\n  group: nightly\n  cancel-in-progress: true\n\
                   jobs:\n  build:\n    name: build\n    permissions:\n      contents: read\n      \
                   issues: write\n    steps:\n      - uses: actions/upload-artifact@x\n      \
                   - uses: actions/cache@x\n      - name: run build\n        env:\n          \
                   TOKEN: ${{ secrets.X }}\n\
                   \x20 unnamed:\n    runs-on: ubuntu-24.04\n    permissions: write-all\n    \
                   steps:\n      - name: run unnamed\n\
                   \x20 flow:\n    name: flow\n    permissions: { contents: write }\n    \
                   steps:\n      - run: true\n";
    let found = scheduled_workflow_violations(planted, &manifest);
    for expected in [
        "uses concurrency",
        "uses secrets.",
        "uses upload-artifact",
        "uses actions/cache",
        "no empty top-level permissions",
        "permissions at line 0 grant \"\" [\"contents: write\"]",
        "permissions at line 8 grant \"\" [\"contents: read\", \"issues: write\"]",
        "permissions at line 19 grant \"write-all\"",
        "permissions at line 24 grant \"{ contents: write }\"",
        "job unnamed lacks \"name: unnamed\"",
        "job unnamed lacks \"permissions:\"",
        "job flow lacks \"permissions:\"",
        "job flow lacks \"- name: run flow\"",
        "jobs [\"build\", \"flow\", \"unnamed\"] differ",
    ] {
        assert!(
            found.iter().any(|line| line.starts_with(expected)),
            "{expected} not in {found:?}"
        );
    }
}

#[test]
fn freshness_reports_every_failure_to_verify_as_exit_2() {
    let unread = could_not_verify(crate::Error::Io {
        operation: "run cargo metadata",
        source: std::io::ErrorKind::NotFound.into(),
    });
    assert!(
        matches!(&unread, crate::Error::CouldNotRun { check: "freshness", reason } if reason.starts_with("run cargo metadata")),
        "{unread:?}"
    );
    assert_eq!(unread.exit_code(), 2);
    let stale = could_not_verify(crate::Error::Rules {
        check: "freshness",
        violations: vec!["nightly: stale".to_owned()],
    });
    assert_eq!(stale.exit_code(), 1);
}
