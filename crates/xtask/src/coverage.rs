// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask coverage`: the nightly coverage report (testing
//! specification 5.4, point 3; issue #74).
//!
//! The measuring tool is cargo-llvm-cov, pinned to one version and run only
//! by this command in the nightly coverage job. This module reads only the
//! tool's JSON export ([`export`]); the tables, the checks and the report
//! are ours ([`tables`]). Replacing the tool replaces [`run_tool`] and
//! nothing else.
//!
//! Nothing here fails on a figure. The command fails when the export cannot
//! be trusted: empty, malformed, written by another tool version, counting
//! a test file or an excluded file, missing an expected crate, or when a
//! crate is in no table or in two.

mod exclusions;
mod export;
mod self_test;
mod tables;

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::Error;
use crate::metadata::{self, Workspace};
use exclusions::{Exclusion, ignore_regex, parse_list};
use export::ExportError;
use tables::{Input, Library, TABLES, Tables, check};

#[cfg(test)]
mod tests;

const CHECK: &str = "coverage";

/// The pinned tool version (issue #74; the decision record in
/// `docs/reviews/`).
const TOOL_VERSION: &str = "0.9.1";

/// Set to `1` only by the nightly coverage job. The real run refuses
/// without it, so the tool is never run on a development machine or in a
/// pull request's jobs by accident.
const JOB_VARIABLE: &str = "IRONAFS_COVERAGE_JOB";

/// The exclusion list, relative to the workspace root.
const EXCLUSIONS: &str = "tests/coverage/exclusions.txt";

/// Runs the tool and prints the report, or with `self_test` proves on
/// planted exports that each failure case is caught.
///
/// # Errors
///
/// Returns the violations found, or why the report could not be made.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let job = std::env::var(JOB_VARIABLE).ok();
    rules(job_violation(job.as_deref()).into_iter().collect())?;
    let workspace = metadata::load(None, &[])?;
    let exclusions_path = workspace.root.join(EXCLUSIONS);
    let exclusions_text = fs::read_to_string(&exclusions_path)
        .map_err(Error::path_io("read coverage exclusions", &exclusions_path))?;
    let (exclusions, violations) = parse_list(&exclusions_text);
    rules(violations)?;
    let version = command_text(
        Command::new("cargo").args(["llvm-cov", "--version"]),
        "cargo llvm-cov --version",
    )?;
    rules(tool_version_violation(&version).into_iter().collect())?;
    let rustc = command_text(Command::new("rustc").arg("-vV"), "rustc -vV")?;
    let target = host_target(&rustc).ok_or(Error::UnexpectedOutput("rustc -vV"))?;

    let export_path = workspace
        .target_dir
        .join("xtask-coverage")
        .join("export.json");
    run_tool(&workspace, &exclusions, &export_path)?;
    let text = fs::read_to_string(&export_path)
        .map_err(Error::path_io("read coverage export", &export_path))?;

    let libraries = libraries(&workspace)?;
    let root = workspace
        .root
        .to_str()
        .ok_or(Error::UnexpectedOutput("cargo metadata"))?;
    let report =
        report(&text, root, target, &libraries, &exclusions, &TABLES).map_err(|violations| {
            Error::Rules {
                check: CHECK,
                violations,
            }
        })?;
    output
        .write_all(report.as_bytes())
        .map_err(Error::io("write coverage report"))
}

/// Reads an export and builds the report: the whole path after the tool,
/// shared by the real run and the self-test.
///
/// # Errors
///
/// Returns every violation found.
fn report(
    text: &str,
    root: &str,
    target: &str,
    libraries: &[Library],
    exclusions: &[Exclusion],
    tables: &Tables<'_>,
) -> Result<String, Vec<String>> {
    let export = export::parse(text).map_err(|error| {
        vec![match error {
            ExportError::Empty => "the export is empty: no file was measured".to_owned(),
            ExportError::Malformed(reason) => format!("the export is malformed: {reason}"),
        }]
    })?;
    check(&Input {
        root,
        target,
        libraries,
        exclusions,
        export: &export,
        tables,
    })
    .map(|report| report.render())
}

/// Runs the pinned tool over the workspace's tests through nextest, with
/// the CI profile and no retries, writing the JSON export to `path`. No
/// branch or doc-test option is passed: both need a nightly compiler.
fn run_tool(workspace: &Workspace, exclusions: &[Exclusion], path: &Path) -> Result<(), Error> {
    const NAME: &str = "cargo llvm-cov nextest";
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::path_io("remove old coverage export", path)(error)),
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&workspace.root)
        .stdin(Stdio::null())
        .args([
            "llvm-cov",
            "nextest",
            "--workspace",
            "--locked",
            "--no-cfg-coverage",
            "--json",
            "--output-path",
        ])
        .arg(path);
    let regex = ignore_regex(&workspace.root, exclusions).map_err(|violation| Error::Rules {
        check: CHECK,
        violations: vec![violation],
    })?;
    if let Some(regex) = regex {
        command.arg("--ignore-filename-regex").arg(regex);
    }
    command.args(["--profile", "ci", "--retries", "0", "--no-tests", "fail"]);
    let status = command.status().map_err(Error::io(NAME))?;
    Error::check_status(NAME, status)
}

/// The workspace crates that have a library, with their library directory
/// relative to the root.
fn libraries(workspace: &Workspace) -> Result<Vec<Library>, Error> {
    workspace
        .members
        .iter()
        .filter_map(|member| member.lib_dir().map(|dir| (member, dir)))
        .map(|(member, dir)| {
            let relative = dir
                .strip_prefix(&workspace.root)
                .ok()
                .and_then(Path::to_str)
                .ok_or(Error::UnexpectedOutput("cargo metadata"))?;
            Ok(Library {
                name: member.name.clone(),
                dir: relative.replace('\\', "/"),
            })
        })
        .collect()
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
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(name))?;
    Error::check_status(name, output.status)?;
    String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(name))
}

/// Why the installed tool is not the pinned one, from its `--version`
/// output, or `None` if it is.
fn tool_version_violation(version_output: &str) -> Option<String> {
    let expected = format!("cargo-llvm-cov {TOOL_VERSION}");
    let found = version_output.trim();
    (found != expected)
        .then(|| format!("the installed tool reports {found:?}, not the pinned {expected:?}"))
}

/// Why the real run may not start here, from the job variable's value, or
/// `None` in the coverage job.
fn job_violation(value: Option<&str>) -> Option<String> {
    (value != Some("1")).then(|| {
        format!(
            "the coverage report runs only in the nightly coverage job ({JOB_VARIABLE}=1); \
             `cargo xtask coverage --self-test` runs anywhere"
        )
    })
}

/// The host target from `rustc -vV`.
fn host_target(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("host:"))
        .map(str::trim)
        .filter(|host| !host.is_empty())
}
