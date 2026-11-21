// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reads the tool's `outcomes.json` and `--list --json`, and classes each
//! mutant by IronAFS's rules (testing specification 5.4, point 4).
//!
//! The tool's own summary is not trusted on its own. cargo-mutants 27.1.0
//! calls a mutant caught when its test phase fails for any reason, which
//! includes nextest's "no tests run" (exit 4) and "build failed" (exit
//! 101). Here a mutant is caught only when its test phase exited with
//! nextest's "tests failed" (exit 100), ran through nextest and passed
//! `--retries 0`. Anything that does not fit a class is "no result", never
//! caught or missed.

use std::collections::BTreeMap;

use crate::json::{self, Value};
use crate::nextest;

#[cfg(test)]
mod tests;

/// nextest's exit codes, as the tool records them (64-bit in its JSON).
const TESTS_FAILED: i64 = nextest::TESTS_FAILED as i64;
const NO_TESTS_RUN: i64 = nextest::NO_TESTS_RUN as i64;

/// How one mutant ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Class {
    /// A test failed on the mutant.
    Caught,
    /// The tests passed on the mutant: it survived.
    Missed,
    /// The tool stopped the mutant's tests at its timeout.
    Timeout,
    /// The mutant did not build.
    Unviable,
    /// The run proves nothing about this mutant, for the reason given.
    NoResult(String),
}

/// One mutant and its class.
#[derive(Debug)]
pub(super) struct Mutant {
    /// The tool's name: `file:line:column: change`.
    pub(super) name: String,
    pub(super) package: String,
    pub(super) file: String,
    pub(super) class: Class,
}

impl Mutant {
    /// The name without its line and column, `file: change`, which stays
    /// the same when unrelated lines move. `None` if the name does not
    /// start with the mutant's file and a position.
    pub(super) fn key(&self) -> Option<String> {
        key_of(&self.name, &self.file)
    }
}

/// `name` without its `:line:column`, if it has the tool's form.
pub(super) fn key_of(name: &str, file: &str) -> Option<String> {
    let rest = name.strip_prefix(file)?.strip_prefix(':')?;
    let (line, rest) = rest.split_once(':')?;
    let (column, change) = rest.split_once(": ")?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    (digits(line) && digits(column)).then(|| format!("{file}: {change}"))
}

/// A whole run as `outcomes.json` records it.
#[derive(Debug)]
pub(super) struct Outcomes {
    /// `cargo_mutants_version`.
    pub(super) version: String,
    /// Whether the baseline passed; `None` if the run has no baseline.
    pub(super) baseline: Option<bool>,
    pub(super) mutants: Vec<Mutant>,
}

/// Reads `outcomes.json`.
///
/// # Errors
///
/// Returns why the text is not an outcomes file of the expected shape.
pub(super) fn parse_outcomes(text: &str) -> Result<Outcomes, String> {
    let root = json::parse(text).map_err(|error| format!("not JSON: {error}"))?;
    let version = root
        .get("cargo_mutants_version")
        .and_then(Value::as_str)
        .ok_or("no cargo_mutants_version string")?
        .to_owned();
    let scenarios = root
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or("no outcomes array")?;
    let mut baseline = None;
    let mut mutants = Vec::new();
    for scenario in scenarios {
        let summary = scenario
            .get("summary")
            .and_then(Value::as_str)
            .ok_or("an outcome has no summary")?;
        let phases = phases(scenario)?;
        match scenario.get("scenario") {
            Some(Value::String(kind)) if kind == "Baseline" => {
                if baseline.is_some() {
                    return Err("more than one baseline".to_owned());
                }
                let passed = summary == "Success"
                    && !phases.is_empty()
                    && phases.iter().all(|phase| phase.status == Status::Success);
                baseline = Some(passed);
            }
            Some(object) => {
                let fields = object
                    .get("Mutant")
                    .ok_or("an outcome's scenario is unknown")?;
                let field = |name: &str| {
                    fields
                        .get(name)
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| format!("a mutant has no {name}"))
                };
                mutants.push(Mutant {
                    name: field("name")?,
                    package: field("package")?,
                    file: field("file")?,
                    class: classify(summary, &phases),
                });
            }
            None => return Err("an outcome has no scenario".to_owned()),
        }
    }
    Ok(Outcomes {
        version,
        baseline,
        mutants,
    })
}

/// Reads `--list --json`: the names of the mutants a run (or shard) holds.
///
/// # Errors
///
/// Returns why the text is not a list of named mutants.
pub(super) fn parse_list(text: &str) -> Result<Vec<String>, String> {
    let root = json::parse(text).map_err(|error| format!("not JSON: {error}"))?;
    root.as_array()
        .ok_or("the list is not an array")?
        .iter()
        .map(|mutant| {
            mutant
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "a listed mutant has no name".to_owned())
        })
        .collect()
}

/// Matches the outcomes with the shard's list: every listed mutant gets
/// exactly one class. A listed mutant with no outcome, or with more than
/// one, and an outcome of a mutant not listed, are "no result".
pub(super) fn reconcile(outcomes: &Outcomes, listed: &[String]) -> Vec<Mutant> {
    let mut by_name: BTreeMap<&str, Vec<&Mutant>> = BTreeMap::new();
    for mutant in &outcomes.mutants {
        by_name.entry(&mutant.name).or_default().push(mutant);
    }
    let mut wanted: BTreeMap<&str, usize> = BTreeMap::new();
    for name in listed {
        *wanted.entry(name).or_default() += 1;
    }
    let mut classed = Vec::new();
    for (name, count) in &wanted {
        let found = by_name.remove(name).unwrap_or_default();
        let (package, file) = found.first().map_or_else(Default::default, |mutant| {
            (mutant.package.clone(), mutant.file.clone())
        });
        let class = match found.as_slice() {
            [] => Class::NoResult("listed for this shard, but has no outcome".to_owned()),
            [one] if *count == 1 => one.class.clone(),
            _ => Class::NoResult(format!(
                "listed {count} times, with {} outcomes: more than one outcome or name",
                found.len()
            )),
        };
        classed.push(Mutant {
            name: (*name).to_owned(),
            package,
            file,
            class,
        });
    }
    for (name, found) in by_name {
        for mutant in found {
            classed.push(Mutant {
                name: name.to_owned(),
                package: mutant.package.clone(),
                file: mutant.file.clone(),
                class: Class::NoResult("has an outcome but is not in the shard's list".to_owned()),
            });
        }
    }
    classed
}

/// A child process's end, as the tool records it.
#[derive(Debug, PartialEq, Eq)]
enum Status {
    Success,
    Failure(i64),
    Timeout,
    Other(String),
}

#[derive(Debug)]
struct Phase {
    name: String,
    status: Status,
    argv: Vec<String>,
}

fn phases(scenario: &Value) -> Result<Vec<Phase>, String> {
    scenario
        .get("phase_results")
        .and_then(Value::as_array)
        .ok_or("an outcome has no phase_results")?
        .iter()
        .map(|phase| {
            let name = phase
                .get("phase")
                .and_then(Value::as_str)
                .ok_or("a phase has no name")?
                .to_owned();
            let status = match phase.get("process_status") {
                Some(Value::String(text)) if text == "Success" => Status::Success,
                Some(Value::String(text)) if text == "Timeout" => Status::Timeout,
                Some(Value::String(text)) => Status::Other(text.clone()),
                Some(object @ Value::Object(_)) => match object.get("Failure") {
                    Some(Value::Number(code)) => Status::Failure(
                        code.parse()
                            .map_err(|_| format!("a phase's exit code {code} is not an integer"))?,
                    ),
                    _ => Status::Other(format!("{object:?}")),
                },
                _ => return Err("a phase has no process_status".to_owned()),
            };
            let argv = phase
                .get("argv")
                .and_then(Value::as_array)
                .ok_or("a phase has no argv")?
                .iter()
                .map(|word| {
                    word.as_str()
                        .map(str::to_owned)
                        .ok_or("an argv word is not text")
                })
                .collect::<Result<_, _>>()?;
            Ok(Phase { name, status, argv })
        })
        .collect()
}

/// Classes a mutant from the tool's summary and the phases it records.
fn classify(summary: &str, phases: &[Phase]) -> Class {
    let test = phases.iter().find(|phase| phase.name == "Test");
    let test_trusted = test.map(|phase| trusted_test_phase(&phase.argv));
    match (summary, test, test_trusted) {
        (_, _, Some(Err(reason))) => Class::NoResult(reason),
        ("Unviable", _, _) => {
            let build_failed = phases
                .iter()
                .any(|phase| phase.name != "Test" && matches!(phase.status, Status::Failure(_)));
            if build_failed {
                Class::Unviable
            } else {
                Class::NoResult("the tool says unviable, but no build failed".to_owned())
            }
        }
        ("Timeout", _, _) if phases.iter().any(|phase| phase.status == Status::Timeout) => {
            Class::Timeout
        }
        ("CaughtMutant", Some(phase), _) => match phase.status {
            Status::Failure(TESTS_FAILED) => Class::Caught,
            Status::Failure(NO_TESTS_RUN) => {
                Class::NoResult("no test ran (nextest exit 4): never caught".to_owned())
            }
            Status::Failure(code) => Class::NoResult(format!(
                "the test phase exited {code}, which is not a test failure"
            )),
            ref other => Class::NoResult(format!("the test phase ended {other:?}")),
        },
        ("MissedMutant", Some(phase), _) if phase.status == Status::Success => Class::Missed,
        _ => Class::NoResult(format!(
            "the tool's summary {summary:?} does not match its phases"
        )),
    }
}

/// Whether a test phase ran through nextest with no retries.
fn trusted_test_phase(argv: &[String]) -> Result<(), String> {
    if !argv.iter().any(|word| word == "nextest") {
        return Err("the test phase did not run through nextest".to_owned());
    }
    let retries_zero = argv.iter().any(|word| word == "--retries=0")
        || argv
            .windows(2)
            .any(|pair| matches!(pair, [flag, value] if flag == "--retries" && value == "0"));
    if retries_zero {
        Ok(())
    } else {
        Err("the test phase did not pass --retries 0".to_owned())
    }
}
