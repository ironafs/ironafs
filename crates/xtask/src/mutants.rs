// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask mutants`: mutation testing with the pinned cargo-mutants
//! (testing specification 5.4, point 4; issue #73).
//!
//! - `--shard k/4` runs one shard of the nightly run over the pure crates
//!   and `ironafs-client`, only in a nightly mutation job.
//! - `--in-diff <file>` runs the mutants a pull request's diff touches in
//!   the pure crates, only in the pull-request mutation job. It is report
//!   only: the reasons a result cannot be trusted go into the report, and
//!   the command does not fail on them.
//! - `--self-test` proves on planted tool output that each failure case is
//!   caught, and never runs the tool.
//!
//! The tool's console output, which can hold a test's output, is written
//! to a file under the target directory and never printed (5.5, point 2).
//! This module reads only the tool's `--list --json` for the selection and
//! its `outcomes.json` ([`outcomes`]); the classes, the checks and the
//! report are ours ([`report`]). Each mutant's tests run through nextest
//! with the `mutants` profile and `--retries 0`. Exclusions live only in
//! `.cargo/mutants.toml`; a skip attribute in the source of a mutated
//! crate fails the run.

mod accepted;
mod fixture;
mod outcomes;
mod report;
mod self_test;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::deps::{self, Kind};
use crate::metadata::{self, Workspace};
use crate::{Command, Error, nextest, rust_source};
use accepted::Accepted;
use report::Scope;
pub(crate) use report::Shard;

#[cfg(test)]
mod tests;

const CHECK: &str = "mutants";

/// The pinned tool version (issue #73; the decision record in
/// `docs/reviews/`).
const TOOL_VERSION: &str = "27.1.0";

/// A job allowed to run the tool, and the variable only it sets.
#[derive(Debug)]
struct Job {
    variable: &'static str,
    name: &'static str,
}

/// The nightly mutation jobs `mutants-0` to `mutants-3`.
const NIGHTLY_JOB: Job = Job {
    variable: "IRONAFS_MUTANTS_JOB",
    name: "the nightly mutation job",
};

/// The pull-request job `mutants-diff`.
const DIFF_JOB: Job = Job {
    variable: "IRONAFS_MUTANTS_PR_JOB",
    name: "the pull-request mutation job",
};

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

/// What `cargo xtask mutants` was asked to do.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    SelfTest,
    Shard(Shard),
    Diff(PathBuf),
}

/// Runs one shard or the diff run and prints its report, or the
/// self-test.
///
/// # Errors
///
/// Returns why the run could not start, and for a shard the reasons its
/// result cannot be trusted.
pub(super) fn run(mode: Mode, output: &mut impl Write) -> Result<(), Error> {
    match mode {
        Mode::SelfTest => self_test::run(output),
        Mode::Shard(shard) => run_shard(shard, output),
        Mode::Diff(diff) => run_diff(&diff, output),
    }
}

fn run_shard(shard: Shard, output: &mut impl Write) -> Result<(), Error> {
    let job = std::env::var(NIGHTLY_JOB.variable).ok();
    rules(
        job_violation(&NIGHTLY_JOB, job.as_deref())
            .into_iter()
            .collect(),
    )?;
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
    let packages = owned(mutated_packages(member_names(&workspace)));
    let accepted = prepare(&workspace, &packages)?;

    // A shard may be empty when the run holds fewer mutants than shards;
    // the whole run may not.
    let whole = list(&workspace, &package_arguments(&packages))?;
    rules(whole_run_violation(&whole).into_iter().collect())?;

    let selection = selection_arguments(shard, &packages);
    let found = run_selection(&workspace, Scope::Shard(shard), &selection, &accepted)?;
    output
        .write_all(found.text.as_bytes())
        .map_err(Error::io("write mutation report"))?;
    rules(found.violations)
}

fn run_diff(diff: &Path, output: &mut impl Write) -> Result<(), Error> {
    let job = std::env::var(DIFF_JOB.variable).ok();
    rules(
        job_violation(&DIFF_JOB, job.as_deref())
            .into_iter()
            .collect(),
    )?;
    let workspace = metadata::load(None, &[])?;
    let diff = fs::canonicalize(diff).map_err(Error::path_io("find the diff", diff))?;
    let packages = owned(diff_packages(member_names(&workspace)));
    let accepted = prepare(&workspace, &packages)?;
    let selection = diff_selection(&diff, &packages);
    let found = run_selection(&workspace, Scope::Diff, &selection, &accepted)?;
    output
        .write_all(report::diff_text(found).as_bytes())
        .map_err(Error::io("write mutation report"))
}

fn member_names(workspace: &Workspace) -> impl Iterator<Item = &str> {
    workspace.members.iter().map(|member| member.name.as_str())
}

fn owned(names: Vec<&str>) -> Vec<String> {
    names.into_iter().map(str::to_owned).collect()
}

/// The checks every real run makes before the tool runs: something to
/// mutate, a valid accepted list, no skip attribute, the pinned version.
/// Returns the accepted list.
fn prepare(workspace: &Workspace, packages: &[String]) -> Result<Vec<Accepted>, Error> {
    if packages.is_empty() {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: "the workspace has no crate to mutate".to_owned(),
        });
    }
    let accepted_path = workspace.root.join(ACCEPTED);
    let accepted_text = fs::read_to_string(&accepted_path)
        .map_err(Error::path_io("read accepted mutants", &accepted_path))?;
    let (accepted, violations) = accepted::parse_accepted(&accepted_text);
    rules(violations)?;
    rules(source_violations(workspace, packages)?)?;
    let version = command_text(
        Command::new("cargo").args(["mutants", "--version"]),
        "cargo mutants --version",
    )?;
    rules(tool_version_violation(&version).into_iter().collect())?;
    Ok(accepted)
}

/// The tool's `--list --json` for a selection.
fn list(workspace: &Workspace, selection: &[String]) -> Result<String, Error> {
    command_text(
        Command::new("cargo")
            .current_dir(&workspace.root)
            .args(["mutants", "--list", "--json"])
            .args(selection),
        "cargo mutants --list --json",
    )
}

/// Lists the selection, runs the tool over it unless it is empty, and
/// builds the report.
fn run_selection(
    workspace: &Workspace,
    scope: Scope,
    selection: &[String],
    accepted: &[Accepted],
) -> Result<report::Report, Error> {
    let out_dir = workspace.target_dir.join("xtask-mutants");
    fs::create_dir_all(&out_dir).map_err(Error::path_io("create mutation output", &out_dir))?;
    let listed = list(workspace, selection)?;
    let listed_any = outcomes::parse_list(&listed).is_ok_and(|names| !names.is_empty());
    let outcomes_text = if listed_any {
        Some(run_tool(workspace, selection, &out_dir)?)
    } else {
        None
    };
    Ok(report::report(
        scope,
        outcomes_text.as_deref(),
        &listed,
        accepted,
    ))
}

/// Why a whole run's list cannot stand: it names no mutant (an exclusion
/// that leaves nothing, say), or it is not a list.
fn whole_run_violation(list: &str) -> Option<String> {
    match outcomes::parse_list(list) {
        Ok(names) if names.is_empty() => Some(
            "the whole run lists no mutant: check the exclusions in .cargo/mutants.toml".to_owned(),
        ),
        Ok(_) => None,
        Err(reason) => Some(format!("the whole run's list is malformed: {reason}")),
    }
}

/// Runs the tool over a selection, its console output to a file only, and
/// returns `outcomes.json`. The tool's exit status is not the verdict:
/// 0, 2 (missed) and 3 (timeouts) are expected, 4 (baseline failed) is
/// caught from the outcomes; any other status (5 and 6 are a diff that
/// does not match the tree or does not parse) is an error.
fn run_tool(workspace: &Workspace, selection: &[String], out_dir: &Path) -> Result<String, Error> {
    const NAME: &str = "cargo mutants";
    let outcomes_path = out_dir.join("mutants.out").join("outcomes.json");
    match fs::remove_file(&outcomes_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::path_io("remove old outcomes", &outcomes_path)(error)),
    }
    let out = out_dir
        .to_str()
        .ok_or(Error::UnexpectedOutput("cargo metadata"))?;
    let status = nextest::run_logged(
        Command::new("cargo")
            .current_dir(&workspace.root)
            .arg("mutants")
            .args(with_test_arguments(selection.to_vec(), out)),
        &out_dir.join("tool.log"),
        NAME,
    )?;
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

/// `--package` for each crate.
fn package_arguments(packages: &[String]) -> Vec<String> {
    packages
        .iter()
        .flat_map(|package| ["--package".to_owned(), package.clone()])
        .collect()
}

/// The arguments that choose a shard's mutants: the same for the list and
/// the run, so the two agree on the shard.
fn selection_arguments(shard: Shard, packages: &[String]) -> Vec<String> {
    let mut arguments = vec!["--shard".to_owned(), shard.to_string()];
    arguments.extend(package_arguments(packages));
    arguments
}

/// The arguments that choose the mutants a diff touches.
fn diff_selection(diff: &Path, packages: &[String]) -> Vec<String> {
    let mut arguments = vec!["--in-diff".to_owned(), diff.to_string_lossy().into_owned()];
    arguments.extend(package_arguments(packages));
    arguments
}

/// A shard run's arguments.
#[cfg(test)]
fn run_arguments(shard: Shard, packages: &[String], output: &str) -> Vec<String> {
    with_test_arguments(selection_arguments(shard, packages), output)
}

/// A diff run's arguments.
#[cfg(test)]
fn diff_run_arguments(diff: &Path, packages: &[String], output: &str) -> Vec<String> {
    with_test_arguments(diff_selection(diff, packages), output)
}

/// A selection plus a fixed order, nextest with the `mutants` profile, no
/// retries and no empty test selection, and the output directory.
fn with_test_arguments(mut arguments: Vec<String>, output: &str) -> Vec<String> {
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

/// The crates a pull request's diff run mutates: the pure crates, in name
/// order (5.4, point 4).
fn diff_packages<'a>(names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut chosen: Vec<&str> = names
        .filter(|name| deps::kind_of(name) == Some(Kind::Pure))
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

/// Why the real run may not start here, from the job's variable's value,
/// or `None` in that job.
fn job_violation(job: &Job, value: Option<&str>) -> Option<String> {
    (value != Some("1")).then(|| {
        format!(
            "this run starts only in {} ({}=1); `cargo xtask mutants --self-test` runs anywhere",
            job.name, job.variable
        )
    })
}
