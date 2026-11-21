// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask mutants --shard k/n`: one shard of the nightly mutation run
//! (testing specification 5.4, point 4; issue #73).
//!
//! The mutation tool is cargo-mutants, pinned to one version and run only
//! by this command in the nightly mutation job. Its console output, which
//! can hold a test's output, is written to a file under the target
//! directory and never printed (5.5, point 2). This module reads only the
//! tool's `--list --json` for the shard and its `outcomes.json`
//! ([`outcomes`]); the classes, the checks and the report are ours
//! ([`report`]).
//!
//! The pure crates and `ironafs-client` are mutated. Each mutant's tests
//! run through nextest with the `mutants` profile and `--retries 0`.
//! Exclusions live only in `.cargo/mutants.toml`; a skip attribute in the
//! source of a mutated crate fails the run.

mod accepted;
mod fixture;
mod outcomes;
mod report;
mod self_test;

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::deps::{self, Kind};
use crate::metadata::{self, Workspace};
use crate::{Command, Error, rust_source};
pub(crate) use report::Shard;

#[cfg(test)]
mod tests;

const CHECK: &str = "mutants";

/// The pinned tool version (issue #73; the decision record in
/// `docs/reviews/`).
const TOOL_VERSION: &str = "27.1.0";

/// Set to `1` only by the nightly mutation job. The real run refuses
/// without it, so the tool never runs on a development machine or in a
/// pull request's jobs by accident.
const JOB_VARIABLE: &str = "IRONAFS_MUTANTS_JOB";

/// How many shards the nightly run has: one job `mutants-<k>` per shard
/// in `tests/tiers/nightly.txt`, so the freshness check fails a night that
/// missed one. Revised with the budgets after a month of nightly figures.
const SHARD_COUNT: u32 = 4;

/// The nightly tier's manifest, relative to the workspace root.
const NIGHTLY_MANIFEST: &str = "tests/tiers/nightly.txt";

/// The accepted survivors, relative to the workspace root.
const ACCEPTED: &str = "tests/mutants-accepted.txt";

/// The attribute the tool honours in source, which the specification
/// forbids: exclusions live in the tool's configuration file.
const SKIP_ATTRIBUTE: &str = "mutants::skip";

/// Runs one shard and prints its report, or with `self_test` proves on
/// planted tool output that each failure case is caught.
///
/// # Errors
///
/// Returns the reasons the shard's result cannot be trusted, or why it
/// could not run.
pub(super) fn run(
    self_test: bool,
    shard: Option<Shard>,
    output: &mut impl Write,
) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let shard = shard.ok_or_else(|| Error::CouldNotRun {
        check: CHECK,
        reason: "the real run needs --shard k/n".to_owned(),
    })?;
    let job = std::env::var(JOB_VARIABLE).ok();
    rules(job_violation(job.as_deref()).into_iter().collect())?;

    if shard.count != SHARD_COUNT {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: format!(
                "the nightly run has {SHARD_COUNT} shards, not {}",
                shard.count
            ),
        });
    }

    let workspace = metadata::load(None, &[])?;
    let manifest_path = workspace.root.join(NIGHTLY_MANIFEST);
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(Error::path_io("read the nightly manifest", &manifest_path))?;
    rules(shard_manifest_violations(&manifest, SHARD_COUNT))?;
    let accepted_path = workspace.root.join(ACCEPTED);
    let accepted_text = fs::read_to_string(&accepted_path)
        .map_err(Error::path_io("read accepted mutants", &accepted_path))?;
    let (accepted, violations) = accepted::parse_accepted(&accepted_text);
    rules(violations)?;

    let packages: Vec<String> =
        mutated_packages(workspace.members.iter().map(|member| member.name.as_str()))
            .into_iter()
            .map(str::to_owned)
            .collect();
    if packages.is_empty() {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: "the workspace has no pure crate and no ironafs-client to mutate".to_owned(),
        });
    }
    rules(source_violations(&workspace, &packages)?)?;

    let version = command_text(
        Command::new("cargo").args(["mutants", "--version"]),
        "cargo mutants --version",
    )?;
    rules(tool_version_violation(&version).into_iter().collect())?;

    let out_dir = workspace.target_dir.join("xtask-mutants");
    fs::create_dir_all(&out_dir).map_err(Error::path_io("create mutation output", &out_dir))?;
    let list = command_text(
        Command::new("cargo")
            .current_dir(&workspace.root)
            .args(["mutants", "--list", "--json"])
            .args(selection_arguments(shard, &packages)),
        "cargo mutants --list --json",
    )?;
    let listed_any = outcomes::parse_list(&list).is_ok_and(|names| !names.is_empty());
    let outcomes_text = if listed_any {
        Some(run_tool(&workspace, shard, &packages, &out_dir)?)
    } else {
        None
    };
    let found = report::report(shard, outcomes_text.as_deref(), &list, &accepted);
    output
        .write_all(found.text.as_bytes())
        .map_err(Error::io("write mutation report"))?;
    rules(found.violations)
}

/// Runs the tool over the shard, its console output to a file only, and
/// returns `outcomes.json`. The tool's exit status is not the verdict:
/// 0, 2 (missed) and 3 (timeouts) are expected, 4 (baseline failed) is
/// caught from the outcomes; any other status is an error.
fn run_tool(
    workspace: &Workspace,
    shard: Shard,
    packages: &[String],
    out_dir: &Path,
) -> Result<String, Error> {
    const NAME: &str = "cargo mutants";
    let results = out_dir.join("mutants.out");
    let outcomes_path = results.join("outcomes.json");
    match fs::remove_file(&outcomes_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::path_io("remove old outcomes", &outcomes_path)(error)),
    }
    let log_path = out_dir.join("tool.log");
    let log = File::create(&log_path).map_err(Error::path_io("create tool log", &log_path))?;
    let log_err = log
        .try_clone()
        .map_err(Error::path_io("open tool log", &log_path))?;
    let out = out_dir
        .to_str()
        .ok_or(Error::UnexpectedOutput("cargo metadata"))?;
    let status = Command::new("cargo")
        .current_dir(&workspace.root)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .args(["mutants"])
        .args(run_arguments(shard, packages, out))
        .status()
        .map_err(Error::io(NAME))?;
    match status.code() {
        Some(0 | 2 | 3 | 4) => {}
        _ => {
            return Err(Error::Failed {
                command: NAME,
                status,
            });
        }
    }
    fs::read_to_string(&outcomes_path).map_err(Error::path_io("read outcomes", &outcomes_path))
}

/// The arguments that choose the mutants: the same for the list and the
/// run, so the two agree on the shard.
fn selection_arguments(shard: Shard, packages: &[String]) -> Vec<String> {
    let mut arguments = vec!["--shard".to_owned(), shard.to_string()];
    for package in packages {
        arguments.push("--package".to_owned());
        arguments.push(package.clone());
    }
    arguments
}

/// The run's arguments: the selection, a fixed order, nextest with the
/// `mutants` profile, no retries and no empty test selection, and the
/// output directory.
fn run_arguments(shard: Shard, packages: &[String], output: &str) -> Vec<String> {
    let mut arguments = selection_arguments(shard, packages);
    arguments.extend(
        [
            "--no-shuffle",
            "--test-tool=nextest",
            "--cargo-test-arg=--profile=mutants",
            "--cargo-test-arg=--retries=0",
            "--cargo-test-arg=--no-tests=fail",
            "--output",
            output,
        ]
        .map(str::to_owned),
    );
    arguments
}

/// Why the nightly manifest does not list exactly one job `mutants-<k>`
/// for each shard `k` below `count`.
fn shard_manifest_violations(manifest: &str, count: u32) -> Vec<String> {
    let ids: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("mutants-"))
        .collect();
    let mut violations = Vec::new();
    for index in 0..count {
        let id = format!("mutants-{index}");
        let listed = ids.iter().filter(|found| **found == id).count();
        if listed != 1 {
            violations.push(format!(
                "the nightly manifest lists shard job {id} {listed} times, not once"
            ));
        }
    }
    for id in ids {
        let known = id
            .strip_prefix("mutants-")
            .and_then(|index| index.parse::<u32>().ok())
            .is_some_and(|index| index < count && id == format!("mutants-{index}"));
        if !known {
            violations.push(format!(
                "the nightly manifest lists {id}, which is not a shard below {count}"
            ));
        }
    }
    violations
}

/// The workspace crates mutated: the pure crates and `ironafs-client`, in
/// name order.
fn mutated_packages<'a>(names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut chosen: Vec<&str> = names
        .filter(|name| matches!(deps::kind_of(name), Some(Kind::Pure | Kind::Composition)))
        .collect();
    chosen.sort_unstable();
    chosen
}

/// Every line of the given files that names the tool's skip attribute.
fn skip_attribute_violations<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    files
        .into_iter()
        .flat_map(|(path, text)| {
            text.lines()
                .enumerate()
                .filter(|(_, line)| line.contains(SKIP_ATTRIBUTE))
                .map(move |(index, _)| {
                    format!(
                        "{path}:{}: names `{SKIP_ATTRIBUTE}`; exclusions belong in \
                         .cargo/mutants.toml",
                        index + 1
                    )
                })
        })
        .collect()
}

/// The skip attribute check over every `.rs` file of the mutated crates.
fn source_violations(workspace: &Workspace, packages: &[String]) -> Result<Vec<String>, Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    for member in &workspace.members {
        if packages.contains(&member.name) {
            rust_source::files(
                &member.dir,
                &|path: &Path| path.ends_with("target"),
                &mut files,
            )?;
        }
    }
    let mut texts = Vec::new();
    for file in files {
        let text =
            fs::read_to_string(&file).map_err(Error::path_io("read mutated source", &file))?;
        let shown = file
            .strip_prefix(&workspace.root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        texts.push((shown, text));
    }
    Ok(skip_attribute_violations(
        texts
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    ))
}

fn rules(violations: Vec<String>) -> Result<(), Error> {
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Error::Rules {
            check: CHECK,
            violations,
        })
    }
}

fn command_text(command: &mut Command, name: &'static str) -> Result<String, Error> {
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(name))?;
    Error::check_status(name, output.status)?;
    String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(name))
}

/// Why the installed tool is not the pinned one, from its `--version`
/// output, or `None` if it is.
fn tool_version_violation(version_output: &str) -> Option<String> {
    let expected = format!("cargo-mutants {TOOL_VERSION}");
    let found = version_output.trim();
    (found != expected)
        .then(|| format!("the installed tool reports {found:?}, not the pinned {expected:?}"))
}

/// Why the real run may not start here, from the job variable's value, or
/// `None` in the mutation job.
fn job_violation(value: Option<&str>) -> Option<String> {
    (value != Some("1")).then(|| {
        format!(
            "mutation testing runs only in the nightly mutation job ({JOB_VARIABLE}=1); \
             `cargo xtask mutants --self-test` runs anywhere"
        )
    })
}
