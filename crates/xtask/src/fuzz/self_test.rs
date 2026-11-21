// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fuzz --self-test`: proves that the step's judgement, the one the real
//! replay uses, fails whenever it cannot show that every seed ran clean (a
//! manifest with no target or a target name it cannot read, a target
//! with no seed, a successful exit that ran nothing or skipped a seed, a
//! failed exit) and that a failure's
//! report holds the target and digest but none of the reproducer's bytes;
//! and that the repository's own manifest names targets that each have a
//! tracked seed.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, MANIFEST, Violation, plan, seeds, verdict};
use crate::Error;

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

/// Runs the self-test; `fuzz_dir` is the repository's fuzz crate.
///
/// # Errors
///
/// Returns the first case that did not end as planted, or why the
/// repository's manifest or seeds could not be read.
pub(super) fn run(fuzz_dir: &Path, output: &mut impl Write) -> Result<(), Error> {
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

    let manifest_path = fuzz_dir.join(MANIFEST);
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(Error::path_io("read fuzz manifest", &manifest_path))?;
    let targets =
        plan(&manifest).map_err(|found| failure("the repository's manifest", Some(&found)))?;
    for target in &targets {
        if seeds(fuzz_dir, target)?.is_empty() {
            let found = Violation::NoSeeds {
                target: target.clone(),
            };
            return Err(failure("the repository's seeds", Some(&found)));
        }
    }
    report(format!(
        "passed: the repository's manifest names {} targets, each with tracked seeds",
        targets.len()
    ))?;
    report(format!(
        "{CHECK} --self-test: the clean cases passed and {} planted cases failed as planted",
        CASES.len()
    ))
}

fn failure(case: &'static str, found: Option<&Violation>) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.map(ToString::to_string).into_iter().collect(),
    }
}
