// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask plant`: the probe of hand-planted defects for class S and
//! R guards (testing specification 5.2.8, N2; issue #73).
//!
//! `tests/planted-defects.txt` lists, for each guard, its plant: the file,
//! an anchor (exact text) and how many times it occurs, the change that
//! replaces it, and the test that must fail. For each, the probe runs the
//! test unplanted (it must pass), applies the plant, checks the file
//! changed, runs the test (it must fail), restores the file and checks the
//! restore. The verdict is "caught", "survived" or "no result": never
//! caught or survived when the anchor did not match the stated number of
//! times, the file did not change, the baseline was not green, the test
//! did not run or did not build, or the restore was not verified. A
//! restore that cannot be verified stops the probe with an error.
//!
//! The tests run through nextest with the `mutants` profile, `--retries 0`
//! and `--no-tests fail`, their output to a file under the target
//! directory, never to the log (5.5, point 2).

mod fake;
mod self_test;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::{Command, Error};
use crate::{metadata, nextest};

const CHECK: &str = "plant";

/// The record, relative to the workspace root.
const RECORD: &str = "tests/planted-defects.txt";

/// One guard's planted defect.
#[derive(Debug)]
pub(super) struct Plant {
    /// The line of the record where the entry starts.
    pub(super) line: usize,
    /// What the guard refuses, in a few words.
    pub(super) guard: String,
    /// The file, relative to the workspace root.
    pub(super) file: String,
    /// The exact text the plant replaces.
    pub(super) anchor: String,
    /// How many times the anchor occurs in the file.
    pub(super) count: usize,
    /// The text that replaces every occurrence of the anchor.
    pub(super) change: String,
    /// The package whose test must fail.
    pub(super) package: String,
    /// The test's full name within its binary.
    pub(super) test: String,
}

/// How one run of the named test ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TestOutcome {
    Passed,
    Failed,
    NoTestRan,
    DidNotBuild,
    Other(String),
}

/// How one plant ended.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// The named test failed on the plant.
    Caught,
    /// The named test passed on the plant.
    Survived,
    /// The run proves nothing, for the reason given.
    NoResult(String),
}

/// Reading and writing the source tree.
pub(super) trait Files {
    /// The file's text, or why it could not be read.
    fn read(&mut self, path: &str) -> Result<String, String>;
    /// Replaces the file's text.
    fn write(&mut self, path: &str, text: &str) -> Result<(), Error>;
}

/// Running one named test.
pub(super) trait Runner {
    /// Runs `test` of `package`; `file_now` is the planted file's text at
    /// the time, for fakes that record it.
    fn run(&mut self, package: &str, test: &str, file_now: &str) -> Result<TestOutcome, Error>;
}

const KEYS: &[&str] = &[
    "guard", "file", "anchor", "count", "change", "package", "test",
];

/// Reads the record: entries of `key: value` lines, separated by blank
/// lines or comments, each with every key once.
pub(super) fn parse_record(text: &str) -> (Vec<Plant>, Vec<String>) {
    let mut plants = Vec::new();
    let mut violations = Vec::new();
    let mut guards = BTreeSet::new();
    let mut entry: Vec<(usize, &str, &str)> = Vec::new();
    let mut finish = |entry: &mut Vec<(usize, &str, &str)>,
                      plants: &mut Vec<Plant>,
                      violations: &mut Vec<String>| {
        if entry.is_empty() {
            return;
        }
        let start = entry.first().map_or(0, |(line, _, _)| *line);
        match build(start, entry) {
            Ok(plant) => {
                if !guards.insert(plant.guard.clone()) {
                    violations.push(format!(
                        "line {start}: guard {:?} is recorded twice",
                        plant.guard
                    ));
                }
                plants.push(plant);
            }
            Err(found) => violations.extend(found),
        }
        entry.clear();
    };
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            finish(&mut entry, &mut plants, &mut violations);
            continue;
        }
        match line.split_once(':') {
            Some((key, value)) if KEYS.contains(&key) => {
                entry.push((number, key, value.trim()));
            }
            Some((key, _)) if !key.contains(' ') => {
                violations.push(format!("line {number}: unknown key {key:?}"));
            }
            _ => violations.push(format!("line {number}: not a `key: value` line")),
        }
    }
    finish(&mut entry, &mut plants, &mut violations);
    (plants, violations)
}

fn build(start: usize, entry: &[(usize, &str, &str)]) -> Result<Plant, Vec<String>> {
    let mut violations = Vec::new();
    let mut value = |key: &str| {
        let found: Vec<&str> = entry
            .iter()
            .filter(|(_, name, _)| *name == key)
            .map(|(_, _, value)| *value)
            .collect();
        match found.as_slice() {
            [one] if !one.is_empty() => (*one).to_owned(),
            [] | [_] => {
                violations.push(format!("line {start}: the entry has no {key}"));
                String::new()
            }
            _ => {
                violations.push(format!("line {start}: the entry gives {key} twice"));
                String::new()
            }
        }
    };
    let plant = Plant {
        line: start,
        guard: value("guard"),
        file: value("file"),
        anchor: value("anchor"),
        count: 0,
        change: value("change"),
        package: value("package"),
        test: value("test"),
    };
    let count_text = value("count");
    let count = match count_text.parse::<usize>() {
        Ok(count) if count > 0 => count,
        _ => {
            if !count_text.is_empty() {
                violations.push(format!(
                    "line {start}: count {count_text:?} is not a positive whole number"
                ));
            }
            0
        }
    };
    if violations.is_empty() {
        Ok(Plant { count, ..plant })
    } else {
        Err(violations)
    }
}

/// Applies one plant, runs its test, restores the file.
///
/// # Errors
///
/// Returns an error when the file could not be restored to its original
/// text, or a write or the runner failed outright; the file is restored
/// first wherever it can be.
pub(super) fn probe(
    plant: &Plant,
    files: &mut impl Files,
    runner: &mut impl Runner,
) -> Result<Verdict, Error> {
    let original = match files.read(&plant.file) {
        Ok(text) => text,
        Err(reason) => {
            return Ok(Verdict::NoResult(format!(
                "could not read the file: {reason}"
            )));
        }
    };
    let matches = original.matches(plant.anchor.as_str()).count();
    if matches != plant.count {
        return Ok(Verdict::NoResult(format!(
            "the anchor matched {matches} times, not {}: nothing was planted",
            plant.count
        )));
    }
    match runner.run(&plant.package, &plant.test, &original)? {
        TestOutcome::Passed => {}
        other => {
            return Ok(Verdict::NoResult(format!(
                "the baseline was not green ({other:?}): nothing was planted"
            )));
        }
    }
    let planted = original.replace(plant.anchor.as_str(), &plant.change);
    if planted == original {
        return Ok(Verdict::NoResult(
            "the plant did not change the file".to_owned(),
        ));
    }
    let plant_write = files.write(&plant.file, &planted);
    let applied = plant_write.is_ok() && files.read(&plant.file).is_ok_and(|now| now == planted);
    let outcome = if applied {
        runner.run(&plant.package, &plant.test, &planted)
    } else {
        Ok(TestOutcome::Other(String::new()))
    };
    // The original is written back whatever happened above, a failed
    // plant write included: a partial write may have changed the file.
    let restore_write = files.write(&plant.file, &original);
    let restored = files.read(&plant.file).ok().as_deref() == Some(original.as_str());
    if restore_write.is_err() || !restored {
        let why = restore_write
            .err()
            .map_or_else(String::new, |error| format!(" ({error})"));
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: format!(
                "the restore of {} could not be verified{why}: the file may still hold \
                 the plant; check it by hand",
                plant.file
            ),
        });
    }
    if let Err(error) = plant_write {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: format!(
                "the plant of {} could not be written ({error}); the original is restored",
                plant.file
            ),
        });
    }
    if !applied {
        return Ok(Verdict::NoResult(
            "the file did not change on disk after the plant was written".to_owned(),
        ));
    }
    Ok(match outcome? {
        TestOutcome::Failed => Verdict::Caught,
        TestOutcome::Passed => Verdict::Survived,
        TestOutcome::NoTestRan => {
            Verdict::NoResult("no test ran on the plant (nextest exit 4)".to_owned())
        }
        TestOutcome::DidNotBuild => Verdict::NoResult("the planted code did not build".to_owned()),
        TestOutcome::Other(reason) => {
            Verdict::NoResult(format!("the test run ended otherwise: {reason}"))
        }
    })
}

/// Probes every plant in the record and prints each verdict, or with
/// `self_test` proves on an in-memory tree that each failure case is
/// caught.
///
/// # Errors
///
/// Returns the plants not caught, or why the probe could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let workspace = metadata::load(None, &[])?;
    let record_path = workspace.root.join(RECORD);
    let text = fs::read_to_string(&record_path)
        .map_err(Error::path_io("read planted-defect record", &record_path))?;
    let (plants, violations) = parse_record(&text);
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations,
        });
    }
    if plants.is_empty() {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: format!("{RECORD} records no plant"),
        });
    }
    let log_dir = workspace.target_dir.join("xtask-plant");
    fs::create_dir_all(&log_dir).map_err(Error::path_io("create probe output", &log_dir))?;
    let mut files = TreeFiles {
        root: workspace.root.clone(),
    };
    let mut runner = Nextest {
        root: workspace.root.clone(),
        log: log_dir.join("tests.log"),
    };
    let mut failures = Vec::new();
    for plant in &plants {
        let verdict = probe(plant, &mut files, &mut runner)?;
        let shown = match &verdict {
            Verdict::Caught => "caught".to_owned(),
            Verdict::Survived => "survived".to_owned(),
            Verdict::NoResult(reason) => format!("no result: {reason}"),
        };
        writeln!(
            output,
            "{CHECK}: {} ({}, {}): {shown}",
            plant.guard, plant.file, plant.test
        )
        .map_err(Error::io("write probe result"))?;
        if verdict != Verdict::Caught {
            failures.push(format!("line {}: {}: {shown}", plant.line, plant.guard));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Error::Rules {
            check: CHECK,
            violations: failures,
        })
    }
}

/// The real source tree.
struct TreeFiles {
    root: PathBuf,
}

impl Files for TreeFiles {
    fn read(&mut self, path: &str) -> Result<String, String> {
        fs::read_to_string(self.root.join(path)).map_err(|error| format!("{path}: {error}"))
    }

    fn write(&mut self, path: &str, text: &str) -> Result<(), Error> {
        let full = self.root.join(path);
        fs::write(&full, text).map_err(Error::path_io("write planted file", &full))
    }
}

/// Runs one test through nextest, its output appended to a log file.
struct Nextest {
    root: PathBuf,
    log: PathBuf,
}

impl Runner for Nextest {
    fn run(&mut self, package: &str, test: &str, _file_now: &str) -> Result<TestOutcome, Error> {
        const NAME: &str = "cargo nextest run (planted defect)";
        let status = nextest::run_logged(
            Command::new("cargo").current_dir(&self.root).args([
                "nextest",
                "run",
                "--package",
                package,
                "--profile",
                "mutants",
                "--retries",
                "0",
                "--no-tests",
                "fail",
                "-E",
                &format!("test(={test})"),
            ]),
            &self.log,
            NAME,
        )?;
        Ok(match status.code() {
            Some(0) => TestOutcome::Passed,
            Some(nextest::TESTS_FAILED) => TestOutcome::Failed,
            Some(nextest::NO_TESTS_RUN) => TestOutcome::NoTestRan,
            Some(nextest::BUILD_FAILED) => TestOutcome::DidNotBuild,
            _ => TestOutcome::Other(status.to_string()),
        })
    }
}
