// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the fuzz manifest and judging a replay.

use std::path::PathBuf;

use std::path::Path;

use super::{Violation, failing_seed, targets, verdict};

const DIGEST: &str = "0123456789abcdef0123456789abcdef01234567";

/// Digests only the second seed, as a test double for `git hash-object`.
fn fake_digest(seed: &Path) -> Option<String> {
    (seed == Path::new("seeds/t/b")).then(|| DIGEST.to_owned())
}

fn seeds() -> Vec<PathBuf> {
    vec![PathBuf::from("seeds/t/a"), PathBuf::from("seeds/t/b")]
}

#[test]
fn targets_reads_each_bin_name_in_order() {
    let manifest = "[package]\nname = \"ironafs-fuzz\"\n\n[[bin]]\nname = \"one_a\"\npath = \"x.rs\"\n\n\
                    [dependencies]\nname = \"not-a-target\"\n\n[[bin]]\n  name   =   \"two_b\"\n";
    assert_eq!(targets(manifest), ["one_a", "two_b"]);
    assert_eq!(
        targets("[package]\nname = \"ironafs-fuzz\"\n"),
        [] as [String; 0]
    );
}

#[test]
fn verdict_passes_only_when_every_seed_was_executed() {
    let log = "Running: seeds/t/a\nExecuted seeds/t/a in 0 ms\nRunning: seeds/t/b\n\
               Executed seeds/t/b in 1 ms\n";
    assert_eq!(verdict("t", &seeds(), true, log, &fake_digest), Ok(()));
    let partial = "Executed seeds/t/a in 0 ms\n";
    assert_eq!(
        verdict("t", &seeds(), true, partial, &fake_digest),
        Err(Violation::NotExecuted {
            target: "t".to_owned(),
            seed: PathBuf::from("seeds/t/b"),
        })
    );
}

#[test]
fn verdict_fails_a_successful_exit_that_ran_nothing() {
    assert_eq!(
        verdict("t", &seeds(), true, "", &fake_digest),
        Err(Violation::NotExecuted {
            target: "t".to_owned(),
            seed: PathBuf::from("seeds/t/a"),
        })
    );
    assert_eq!(
        verdict("t", &[], true, "Done 0 runs\n", &fake_digest),
        Err(Violation::NoSeeds {
            target: "t".to_owned()
        })
    );
}

#[test]
fn verdict_reports_a_failure_by_target_and_the_failing_seed_s_digest_only() {
    let log = "Running: seeds/t/a\nExecuted seeds/t/a in 0 ms\nRunning: seeds/t/b\n\
               ==1== ERROR: libFuzzer: deadly signal\nPLANTED\n";
    let found = verdict("t", &seeds(), false, log, &fake_digest);
    assert_eq!(
        found,
        Err(Violation::Failed {
            target: "t".to_owned(),
            digest: Some(DIGEST.to_owned()),
        })
    );
    let shown = found.unwrap_err().to_string();
    assert_eq!(
        shown,
        format!(
            "t: failed on a seed; reproducer digest {DIGEST} (git hash-object); the input \
             is never printed: report it privately, as SECURITY.md says"
        )
    );
}

#[test]
fn failing_seed_is_the_last_one_running_and_never_executed() {
    let seeds = seeds();
    let log = "Running: seeds/t/a\nExecuted seeds/t/a in 0 ms\nRunning: seeds/t/b\nboom\n";
    assert_eq!(failing_seed(&seeds, log), Some(&seeds[1]));
    let all = "Running: seeds/t/a\nExecuted seeds/t/a in 0 ms\nRunning: seeds/t/b\n\
               Executed seeds/t/b in 0 ms\n";
    assert_eq!(failing_seed(&seeds, all), None);
    assert_eq!(failing_seed(&seeds, "Running: seeds/t/other\n"), None);
    assert_eq!(failing_seed(&seeds, ""), None);
}

#[test]
fn verdict_fails_a_failed_exit_even_when_every_seed_ran() {
    let log = "Executed seeds/t/a in 0 ms\nExecuted seeds/t/b in 0 ms\n";
    assert_eq!(
        verdict("t", &seeds(), false, log, &fake_digest),
        Err(Violation::Failed {
            target: "t".to_owned(),
            digest: None,
        })
    );
}

#[test]
fn fuzz_crate_commands_run_on_the_crate_s_own_toolchain() {
    // The rustup proxy exports the workspace's toolchain to `cargo xtask`;
    // inherited, it would override fuzz/rust-toolchain.toml.
    let command = super::in_fuzz_crate(std::path::Path::new("fuzz"));
    let removed = command
        .get_envs()
        .any(|(name, value)| name == std::ffi::OsStr::new("RUSTUP_TOOLCHAIN") && value.is_none());
    assert!(removed, "RUSTUP_TOOLCHAIN is not removed");
    assert_eq!(
        command.get_current_dir(),
        Some(std::path::Path::new("fuzz"))
    );
}
