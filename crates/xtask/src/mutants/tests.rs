// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the run guard, the pinned version, the selection the list
//! and the run share, and the source attribute check.

use super::report::Shard;
use super::{
    SHARD_COUNT, job_violation, mutated_packages, run_arguments, selection_arguments,
    shard_manifest_violations, skip_attribute_violations, tool_version_violation,
};

#[test]
fn the_real_run_refuses_outside_the_mutation_job() {
    assert_eq!(job_violation(Some("1")), None);
    for value in [None, Some(""), Some("0"), Some("true")] {
        let found = job_violation(value).expect("refused");
        assert!(found.contains("IRONAFS_MUTANTS_JOB=1"), "{found}");
    }
}

#[test]
fn only_the_pinned_tool_version_runs() {
    assert_eq!(tool_version_violation("cargo-mutants 27.1.0\n"), None);
    for text in [
        "cargo-mutants 27.0.0",
        "cargo-mutants 27.1.0-pre",
        "",
        "27.1.0",
    ] {
        assert!(tool_version_violation(text).is_some(), "{text}");
    }
}

#[test]
fn pure_crates_and_the_client_are_mutated() {
    let members = [
        "xtask",
        "ironafs-time",
        "ironafs-test-support",
        "ironafs-client",
        "ironafs-cli",
        "ironafs-xdr",
    ];
    assert_eq!(
        mutated_packages(members.iter().copied()),
        vec!["ironafs-client", "ironafs-time", "ironafs-xdr"]
    );
}

#[test]
fn the_list_and_the_run_select_the_same_mutants() {
    let shard = Shard { index: 2, count: 4 };
    let packages = vec!["ironafs-time".to_owned()];
    let selection = selection_arguments(shard, &packages);
    assert_eq!(
        selection,
        vec!["--shard", "2/4", "--package", "ironafs-time"]
    );
    let run = run_arguments(shard, &packages, "out");
    assert!(
        run.windows(selection.len())
            .any(|window| window == selection.as_slice())
    );
    for required in [
        "--no-shuffle",
        "--test-tool=nextest",
        "--cargo-test-arg=--retries=0",
        "--cargo-test-arg=--profile=mutants",
        "--cargo-test-arg=--no-tests=fail",
    ] {
        assert!(
            run.iter().any(|word| word == required),
            "{required}: {run:?}"
        );
    }
    assert!(run.windows(2).any(|pair| pair == ["--output", "out"]));
}

#[test]
fn a_skip_attribute_in_source_is_refused() {
    for text in [
        "#[mutants::skip]\nfn f() {}\n",
        "#[cfg_attr(test, mutants::skip)]\nfn f() {}\n",
        "#![mutants::skip]\n",
    ] {
        let found = skip_attribute_violations([("crates/a/src/lib.rs", text)]);
        assert_eq!(found.len(), 1, "{text}");
        assert!(found[0].contains("crates/a/src/lib.rs:"), "{found:?}");
    }
    assert_eq!(
        skip_attribute_violations([("crates/a/src/lib.rs", "fn mutants() {}\n")]),
        [] as [std::string::String; 0]
    );
}

#[test]
fn mutants_run_fails_on_missing_shard() {
    let full = "# nightly\nbuild\nmutants-0\nmutants-1\nmutants-2\nmutants-3\n";
    assert_eq!(shard_manifest_violations(full, 4), Vec::<String>::new());
    let missing = full.replace("mutants-2\n", "");
    let found = shard_manifest_violations(&missing, 4);
    assert!(found.iter().any(|v| v.contains("mutants-2")), "{found:?}");
    let extra = format!("{full}mutants-4\n");
    assert_ne!(
        shard_manifest_violations(&extra, 4),
        [] as [std::string::String; 0]
    );
}

#[test]
fn the_nightly_manifest_holds_every_shard() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = std::fs::read_to_string(root.join("tests/tiers/nightly.txt")).unwrap();
    assert_eq!(
        shard_manifest_violations(&manifest, SHARD_COUNT),
        Vec::<String>::new()
    );
}

#[test]
fn mutants_self_test_ends_every_case_as_planted() {
    let mut output = Vec::new();
    super::self_test::run(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("a run that selected no test is never caught: as planted"));
    assert!(text.contains("a skip attribute in source: as planted"));
}
