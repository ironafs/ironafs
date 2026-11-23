// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fuzz --self-test`: proves that the step's judgement, the one the real
//! replay uses, fails whenever it cannot show that every seed ran clean (a
//! manifest with no target or a target name it cannot read, a target
//! with no seed, a successful exit that ran nothing or skipped a seed, a
//! failed exit) and that a failure's
//! report holds the target and digest but none of the reproducer's bytes;
//! that, in a planted git repository, the seed reader returns exactly the
//! tracked regular files and refuses an untracked, changed, deleted or
//! linked entry, and git names a seed by its digest; and that the
//! repository's own manifest names targets that each have a tracked seed.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, MANIFEST, Violation, git_digest, git_in, plan, seeds, verdict};
use crate::Error;
use crate::fixture::{self, write_file};

const TARGET: &str = "planted_target";
const DIGEST: &str = "89abcdef0123456789abcdef0123456789abcdef";
/// A planted reproducer, as the engine may dump one: bytes, hexadecimal
/// and base64. None of it may reach the report.
const REPRODUCER_BYTES: &str = "SECRETINPUT";
const REPRODUCER_HEX: &str = "0x53,0x45,0x43";
const REPRODUCER_BASE64: &str = "U0VDUkVUSU5QVVQ=";
const REPRODUCER: &[&str] = &[REPRODUCER_BYTES, REPRODUCER_HEX, REPRODUCER_BASE64];

const SEED_ONE: &str = "seeds/planted_target/one";
const SEED_TWO: &str = "seeds/planted_target/two";

/// Stands in for `git hash-object`: digests only the second planted seed.
fn planted_digest(seed: &Path) -> Option<String> {
    (seed == Path::new(SEED_TWO)).then(|| DIGEST.to_owned())
}

fn planted_seeds() -> Vec<PathBuf> {
    vec![PathBuf::from(SEED_ONE), PathBuf::from(SEED_TWO)]
}

/// The engine's report of running each of `seeds` to the end.
fn executed(seeds: &[&str]) -> String {
    let mut log = String::new();
    for seed in seeds {
        log.push_str("Running: ");
        log.push_str(seed);
        log.push_str("\nExecuted ");
        log.push_str(seed);
        log.push_str(" in 0 ms\n");
    }
    log
}

/// The first seed ran; the engine crashed on the second, dumping it.
fn crash_log() -> String {
    format!(
        "{}Running: {SEED_TWO}\n==7== ERROR: libFuzzer: deadly signal\n\
         {REPRODUCER_HEX}\n{REPRODUCER_BYTES}\nBase64: {REPRODUCER_BASE64}\n",
        executed(&[SEED_ONE]),
    )
}

/// A planted case: the result the judgement gave, and what it must be.
struct Case {
    name: &'static str,
    result: fn() -> Result<(), Violation>,
    expect: fn(&Violation) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a manifest with no fuzz target",
        result: || plan("[package]\nname = \"ironafs-fuzz\"\n").map(|_| ()),
        expect: |found| *found == Violation::NoTargets,
    },
    Case {
        name: "a manifest with an unreadable target name among readable ones",
        result: || plan("[[bin]]\nname = \"one_a\"\n\n[[bin]]\nname = two_b\n").map(|_| ()),
        expect: |found| *found == Violation::UnreadableTarget { line: 5 },
    },
    Case {
        name: "a quoted [[\"bin\"]] header",
        result: || {
            plan("[package]\nautobins = false\n\n[[\"bin\"]]\nname = \"one_a\"\n").map(|_| ())
        },
        expect: |found| *found == Violation::UnreadableTarget { line: 4 },
    },
    Case {
        name: "a manifest that lets Cargo find targets of its own",
        result: || {
            plan("[package]\nname = \"ironafs-fuzz\"\n\n[[bin]]\nname = \"one_a\"\n").map(|_| ())
        },
        expect: |found| *found == Violation::AutoBins,
    },
    Case {
        name: "a target with no tracked seed",
        result: || {
            verdict(
                TARGET,
                &[],
                true,
                "Done 0 runs in 0 second(s)\n",
                &planted_digest,
            )
        },
        expect: |found| matches!(found, Violation::NoSeeds { .. }),
    },
    Case {
        name: "a successful exit that ran nothing",
        result: || verdict(TARGET, &planted_seeds(), true, "", &planted_digest),
        expect: |found| matches!(found, Violation::NotExecuted { .. }),
    },
    Case {
        name: "a successful exit that skipped a seed",
        result: || {
            verdict(
                TARGET,
                &planted_seeds(),
                true,
                &executed(&[SEED_ONE]),
                &planted_digest,
            )
        },
        expect: |found| matches!(found, Violation::NotExecuted { seed, .. } if seed.ends_with("two")),
    },
    Case {
        name: "a crash, reported by target and the failing seed's digest only",
        result: || {
            verdict(
                TARGET,
                &planted_seeds(),
                false,
                &crash_log(),
                &planted_digest,
            )
        },
        expect: |found| {
            let shown = found.to_string();
            matches!(found, Violation::Failed { digest: Some(digest), .. } if digest == DIGEST)
                && shown.contains(TARGET)
                && shown.contains(DIGEST)
                && REPRODUCER.iter().all(|bytes| !shown.contains(bytes))
                && planted_seeds()
                    .iter()
                    .all(|seed| !shown.contains(&seed.display().to_string()))
        },
    },
    Case {
        name: "a failed exit with no failing seed to name",
        result: || {
            let log = executed(&[SEED_ONE, SEED_TWO]);
            verdict(TARGET, &planted_seeds(), false, &log, &planted_digest)
        },
        expect: |found| matches!(found, Violation::Failed { digest: None, .. }),
    },
];

/// Runs `git` with `args` in `dir`, which must succeed.
fn git(dir: &Path, args: &[&str]) -> Result<(), Error> {
    let status = git_in(dir)
        .args(args)
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(Error::io("run git"))?;
    Error::check_status("git in the planted repository", status)
}

/// What the seed reader gives in `fuzz_dir` for the planted target.
fn read_seeds(fuzz_dir: &Path) -> Result<Result<Vec<PathBuf>, Violation>, Error> {
    seeds(fuzz_dir, TARGET)
}

/// Whether `found` refuses the planted entry `entry`.
fn refuses(found: &Result<Vec<PathBuf>, Violation>, entry: &str) -> bool {
    matches!(found, Err(Violation::UntrackedSeed { entry: refused, .. }) if refused == Path::new(entry))
}

/// The seed reader and the digest, on a git repository planted under
/// `dir`: its index holds two seeds; then one change at a time is made to
/// the working tree, and each must be refused.
fn planted_repository(
    dir: &Path,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let fuzz_dir = dir.join("fuzz");
    let one = fuzz_dir.join(SEED_ONE);
    let two = fuzz_dir.join(SEED_TWO);
    write_file(&one, "one")?;
    write_file(&two, "two")?;
    git(dir, &["init", "-q"])?;
    git(dir, &["add", "--", "fuzz"])?;
    let clean = "the tracked seeds of a planted repository";
    if read_seeds(&fuzz_dir)? != Ok(planted_seeds()) {
        return Err(failure(clean, None));
    }
    if git_digest(&fuzz_dir, Path::new(SEED_ONE)).is_none() {
        return Err(failure("a planted seed's digest from git", None));
    }
    report(format!("passed: {clean}, and a seed's digest from git"))?;

    let stray = "seeds/planted_target/stray";
    write_file(&fuzz_dir.join(stray), "stray")?;
    let found = read_seeds(&fuzz_dir)?;
    std::fs::remove_file(fuzz_dir.join(stray)).map_err(Error::path_io("remove", &fuzz_dir))?;
    check_refused("an untracked file among the seeds", &found, stray, report)?;

    write_file(&one, "changed")?;
    let found = read_seeds(&fuzz_dir)?;
    write_file(&one, "one")?;
    check_refused(
        "a tracked seed changed in the working tree",
        &found,
        SEED_ONE,
        report,
    )?;

    std::fs::remove_file(&two).map_err(Error::path_io("remove", &two))?;
    let found = read_seeds(&fuzz_dir)?;
    write_file(&two, "two")?;
    check_refused(
        "a tracked seed deleted from the working tree",
        &found,
        SEED_TWO,
        report,
    )?;

    #[cfg(unix)]
    {
        let link = "seeds/planted_target/link";
        std::os::unix::fs::symlink("one", fuzz_dir.join(link))
            .map_err(Error::path_io("link", &fuzz_dir))?;
        git(dir, &["add", "--", "fuzz"])?;
        let found = read_seeds(&fuzz_dir)?;
        check_refused("a tracked link among the seeds", &found, link, report)?;
    }
    Ok(())
}

fn check_refused(
    case: &'static str,
    found: &Result<Vec<PathBuf>, Violation>,
    entry: &str,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    match found {
        Err(violation) if refuses(found, entry) => report(format!("caught: {case}: {violation}")),
        Err(violation) => Err(failure(case, Some(violation))),
        Ok(_) => Err(failure(case, None)),
    }
}

/// Runs the self-test; `fuzz_dir` is the repository's fuzz crate, and the
/// planted repository goes under `target_dir`.
///
/// # Errors
///
/// Returns the first case that did not end as planted, or why the
/// repository's manifest or seeds could not be read.
pub(super) fn run(
    fuzz_dir: &Path,
    target_dir: &Path,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report fuzz self-test"));

    let planted = planted_seeds();
    if verdict(
        TARGET,
        &planted,
        true,
        &executed(&[SEED_ONE, SEED_TWO]),
        &planted_digest,
    )
    .is_err()
    {
        return Err(failure("every seed executed, a successful exit", None));
    }
    report("passed: every seed executed, a successful exit".to_owned())?;

    for case in CASES {
        match (case.result)() {
            Err(found) if (case.expect)(&found) => {
                report(format!("caught: {}: {found}", case.name))?;
            }
            Err(found) => return Err(failure(case.name, Some(&found))),
            Ok(()) => return Err(failure(case.name, None)),
        }
    }

    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let planted = planted_repository(&dir, &mut report);
    fixture::remove_dir(&dir)?;
    planted?;

    let manifest_path = fuzz_dir.join(MANIFEST);
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(Error::path_io("read fuzz manifest", &manifest_path))?;
    let targets =
        plan(&manifest).map_err(|found| failure("the repository's manifest", Some(&found)))?;
    for target in &targets {
        let found = match seeds(fuzz_dir, target)? {
            Ok(seeds) if !seeds.is_empty() => continue,
            Ok(_) => Violation::NoSeeds {
                target: target.clone(),
            },
            Err(violation) => violation,
        };
        return Err(failure("the repository's seeds", Some(&found)));
    }
    report(format!(
        "passed: the repository's manifest names {} targets, each with tracked seeds",
        targets.len()
    ))?;
    report(format!(
        "{CHECK} --self-test: the clean cases passed and every planted case failed as planted"
    ))
}

fn failure(case: &'static str, found: Option<&Violation>) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.map(ToString::to_string).into_iter().collect(),
    }
}
