// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the fuzz manifest and judging a replay.

use std::path::PathBuf;

use std::path::Path;

use super::{Violation, failing_seed, plan, seed_list, targets, verdict};

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
    assert_eq!(
        targets(manifest),
        Ok(vec!["one_a".to_owned(), "two_b".to_owned()])
    );
    assert_eq!(
        targets("[package]\nname = \"ironafs-fuzz\"\n"),
        Ok(Vec::new())
    );
}

#[test]
fn targets_reads_trailing_comments_and_single_quoted_names() {
    let manifest = "[[bin]] # the first\nname = \"one_a\"  # Rx header decode\n\n\
                    [[bin]]\nname = 'two_b'\n\n[[bin]]\nname='three_c'# tight\n";
    assert_eq!(
        targets(manifest),
        Ok(vec![
            "one_a".to_owned(),
            "two_b".to_owned(),
            "three_c".to_owned()
        ])
    );
}

#[test]
fn targets_refuses_a_name_it_cannot_read_among_readable_ones() {
    for (line, name) in [
        (6, "name = ironafs-rx_header"),
        (6, "name = \"rx_header\" trailing"),
        (6, "name = \"rx\\\"header\""),
        (6, "name = \"rx_header"),
        (6, "name = \"\""),
        (6, "name = \"\"\"rx_header\"\"\""),
        (6, "\"name\" = \"rx_header\""),
    ] {
        let manifest = format!("[[bin]]\nname = \"one_a\"\npath = \"a.rs\"\n\n[[bin]]\n{name}\n");
        assert_eq!(
            targets(&manifest),
            Err(Violation::UnreadableTarget { line }),
            "{name}"
        );
    }
}

#[test]
fn targets_refuses_a_bin_table_without_a_name_or_bins_in_another_form() {
    let unnamed = "[[bin]]\nname = \"one_a\"\n\n[[bin]]\npath = \"b.rs\"\n\n[dependencies]\n";
    assert_eq!(
        targets(unnamed),
        Err(Violation::UnreadableTarget { line: 4 })
    );
    assert_eq!(
        targets("[[bin]]\nname = \"one_a\"\n[[bin]]\n"),
        Err(Violation::UnreadableTarget { line: 3 })
    );
    assert_eq!(
        targets("[package]\nname = \"x\"\nbin = [{ name = \"one_a\" }]\n"),
        Err(Violation::UnreadableTarget { line: 3 })
    );
    assert_eq!(
        targets("[[ bin ]]\nname = \"one_a\"\n"),
        Err(Violation::UnreadableTarget { line: 1 })
    );
    for header in [
        "[[\"bin\"]]",
        "[['bin']]",
        "[[ \"bin\" ]]",
        "[\"bin\"]",
        "[[\"\\u0062in\"]]",
        "[[\"b\\u0069n\"]]",
    ] {
        assert_eq!(
            targets(&format!("{header}\nname = \"one_a\"\n")),
            Err(Violation::UnreadableTarget { line: 1 }),
            "{header}"
        );
    }
    assert_eq!(
        targets("[[bin]]\nname = \"one_a\"\nname = \"two_b\"\n"),
        Err(Violation::UnreadableTarget { line: 3 })
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

fn assert_replay_args(target: &str, limit: Option<&str>) {
    use std::ffi::OsStr;

    let command =
        super::replay_command(Path::new("fuzz"), target, &seeds(), Path::new("artefacts"));
    let mut expected = vec![
        "fuzz",
        "run",
        target,
        "seeds/t/a",
        "seeds/t/b",
        "--",
        "-artifact_prefix=artefacts/",
    ];
    expected.extend(limit);
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        expected.into_iter().map(OsStr::new).collect::<Vec<_>>(),
        "{target}"
    );
}

#[test]
fn replay_passes_the_xdr_message_cap_plus_one() {
    assert_replay_args("ironafs-xdr_decode", Some("-max_len=257"));
}

#[test]
fn replay_passes_the_configuration_file_cap_plus_one() {
    assert_replay_args("ironafs-config_syntax", Some("-max_len=262145"));
}

#[test]
fn replay_preserves_arguments_for_targets_without_a_declared_cap() {
    assert_replay_args("ironafs-os_fdpass", None);
    assert_replay_args("other-target", None);
}

#[test]
fn replay_refuses_a_seed_that_the_engine_would_truncate() {
    use std::fs;

    let executable = std::env::current_exe().unwrap();
    let build_dir = executable.parent().unwrap();
    let dir = crate::fixture::fresh_dir(build_dir, "fuzz-seed-length").unwrap();
    fs::create_dir_all(&dir).unwrap();
    let seed = PathBuf::from("synthetic-seed");
    let seed_path = dir.join(&seed);
    let file = fs::File::create(&seed_path).unwrap();
    // An artefact directory under a file cannot be created. A seed that fits
    // reaches that step, and neither fixture can start the engine.
    let artefacts = seed_path.join("artefacts");
    let mut results = Vec::new();
    for (target, limit) in [
        ("ironafs-xdr_decode", 257),
        ("ironafs-config_syntax", 262_145),
    ] {
        file.set_len(limit).unwrap();
        let at = super::replay(&dir, target, std::slice::from_ref(&seed), &artefacts);
        file.set_len(limit + 1).unwrap();
        let above = super::replay(&dir, target, std::slice::from_ref(&seed), &artefacts);
        results.push((target, limit, at, above));
    }
    drop(file);
    crate::fixture::remove_dir(&dir).unwrap();

    for (target, limit, at, above) in results {
        assert!(matches!(
            at,
            Err(crate::Error::PathIo {
                operation: "create artefact directory",
                ..
            })
        ));
        let message = above
            .ok()
            .and_then(Result::err)
            .map(|error| error.to_string());
        assert_eq!(
            message,
            Some(format!(
                "{target}: a tracked seed exceeds the engine limit of {limit} bytes"
            ))
        );
    }
}

#[test]
fn targets_refuses_a_multi_line_string() {
    for opening in ["\"\"\"", "'''"] {
        let manifest = format!(
            "[package]\ndescription = {opening}\n[[bin]]\nname = \"phantom\"\n{opening}\n\n\
             [[bin]]\nname = \"one_a\"\n"
        );
        assert_eq!(
            targets(&manifest),
            Err(Violation::UnreadableTarget { line: 2 }),
            "{opening}"
        );
    }
}

#[test]
fn plan_requires_autobins_off_in_the_package_table() {
    let bins = "\n[[bin]]\nname = \"one_a\"\n";
    assert_eq!(
        plan(&format!(
            "[package]\nname = \"x\"\nautobins = false # only [[bin]]\n{bins}"
        )),
        Ok(vec!["one_a".to_owned()])
    );
    for package in [
        "[package]\nname = \"x\"\n",
        "[package]\nautobins = true\n",
        "[package]\n# autobins = false\n",
        "[package]\n\n[package.metadata]\nautobins = false\n",
    ] {
        assert_eq!(
            plan(&format!("{package}{bins}")),
            Err(Violation::AutoBins),
            "{package}"
        );
    }
}

/// An entry of `git ls-files -z --stage`.
fn staged(mode: &str, path: &str) -> String {
    format!("{mode} 0123456789abcdef0123456789abcdef01234567 0\t{path}\0")
}

#[test]
fn seed_list_keeps_the_tracked_regular_files_sorted() {
    let tracked = staged("100644", "seeds/t/b") + &staged("100755", "seeds/t/a");
    assert_eq!(
        seed_list("t", tracked.as_bytes(), b""),
        Ok(vec![PathBuf::from("seeds/t/a"), PathBuf::from("seeds/t/b")])
    );
    assert_eq!(seed_list("t", b"", b""), Ok(Vec::new()));
}

#[test]
fn seed_list_refuses_a_stray_entry_or_a_tracked_entry_that_is_no_regular_file() {
    let tracked = staged("100644", "seeds/t/a");
    let refused = |entry: &str| {
        Err(Violation::UntrackedSeed {
            target: "t".to_owned(),
            entry: PathBuf::from(entry),
        })
    };
    // Untracked, changed or deleted: `--others --modified` names it.
    assert_eq!(
        seed_list("t", tracked.as_bytes(), b"seeds/t/new\0"),
        refused("seeds/t/new")
    );
    // A link, or a submodule.
    for mode in ["120000", "160000"] {
        let with = tracked.clone() + &staged(mode, "seeds/t/odd");
        assert_eq!(
            seed_list("t", with.as_bytes(), b""),
            refused("seeds/t/odd"),
            "{mode}"
        );
    }
    // An unmerged entry.
    let unmerged = "100644 0123456789abcdef0123456789abcdef01234567 2\tseeds/t/c\0";
    assert_eq!(
        seed_list("t", unmerged.as_bytes(), b""),
        refused("seeds/t/c")
    );
}

#[test]
fn targets_refuses_a_key_with_an_escape() {
    // At the top level, an escape could spell `bin`; in a [[bin]] table,
    // `name`.
    for (manifest, line) in [
        ("[package]\n\"\\u0062in\" = [{ name = \"one_a\" }]\n", 2),
        ("[[bin]]\n\"\\u006eame\" = \"one_a\"\n", 2),
        ("[[bin]]\nname = \"one_a\"\n\"pa\\u0074h\" = \"a.rs\"\n", 3),
    ] {
        assert_eq!(
            targets(manifest),
            Err(Violation::UnreadableTarget { line }),
            "{manifest}"
        );
    }
}
