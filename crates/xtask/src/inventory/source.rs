// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the inventory's inputs come from: nextest's list and run, the
//! files in the working tree, and the base revision's files through git.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{DIR, FEATURE_SETS, Files, REMOVED, Run, listing};
use crate::{Error, junit};

/// nextest's exit status when tests ran and some failed; the run still
/// wrote its report.
const TESTS_FAILED: i32 = 100;

/// The nextest profile whose `JUnit` report the check reads (its `junit.path`
/// in `.config/nextest.toml`).
const PROFILE: &str = "ci";

/// Git's variables that would point a command at another repository than
/// the one in its working directory, as inside a git hook.
pub(super) const GIT_LOCATION: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

/// Which workspace nextest runs in.
#[derive(Debug)]
pub(super) enum Cargo<'a> {
    /// This repository, with its lock file held fixed.
    Repository(&'a Path),
    /// A self-test fixture, which has no lock file and no network.
    Fixture(&'a Path),
}

impl Cargo<'_> {
    fn nextest(&self, subcommand: &str) -> Command {
        let (root, option) = match self {
            Self::Repository(root) => (root, "--locked"),
            Self::Fixture(root) => (root, "--offline"),
        };
        let mut command = Command::new("cargo");
        // nextest reads its configuration from the workspace it runs in.
        command
            .current_dir(root)
            .args(["nextest", subcommand, "--workspace", option]);
        command
    }
}

/// Lists every test of the workspace for the host target with the default
/// feature set; nothing is run.
///
/// # Errors
///
/// Returns an error if nextest fails or its list cannot be read.
pub(super) fn list(cargo: &Cargo<'_>) -> Result<Run, Error> {
    list_with_dir(cargo).map(|(run, _)| run)
}

fn list_with_dir(cargo: &Cargo<'_>) -> Result<(Run, PathBuf), Error> {
    const COMMAND: &str = "cargo nextest list";
    let output = cargo
        .nextest("list")
        .args(["--message-format", "json"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    let text = String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(COMMAND))?;
    let (target, target_dir, listed) = listing::parse(&text)?;
    let run = Run {
        target,
        features: FEATURE_SETS.first().copied().unwrap_or_default().to_owned(),
        listed,
        results: Vec::new(),
    };
    Ok((run, target_dir))
}

/// Lists the tests, then runs every one of them once, with no retries and
/// without stopping at a failure, and reads how each ended from the run's
/// `JUnit` report. A report left by an earlier run is removed first, so only
/// this run's report is read.
///
/// # Errors
///
/// Returns an error if nextest cannot list or run the tests (a build
/// failure, no tests), or the report is missing or unreadable.
pub(super) fn run(cargo: &Cargo<'_>) -> Result<Run, Error> {
    const COMMAND: &str = "cargo nextest run";
    let (mut run, target_dir) = list_with_dir(cargo)?;
    let report = target_dir.join("nextest").join(PROFILE).join("junit.xml");
    match fs::remove_file(&report) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(Error::path_io("remove the earlier JUnit report", &report)(
                error,
            ));
        }
        _ => {}
    }
    let status = cargo
        .nextest("run")
        .args([
            "--profile",
            PROFILE,
            "--retries",
            "0",
            "--no-tests",
            "fail",
            "--no-fail-fast",
        ])
        .status()
        .map_err(Error::io(COMMAND))?;
    if !status.success() && status.code() != Some(TESTS_FAILED) {
        return Err(Error::Failed {
            command: COMMAND,
            status,
        });
    }
    let text = fs::read_to_string(&report).map_err(Error::path_io(
        "read the run's JUnit report (the ci profile's junit.path in .config/nextest.toml)",
        &report,
    ))?;
    run.results = junit::parse(&text).map_err(|error| Error::CouldNotRun {
        check: super::CHECK,
        reason: format!("{}: {error}", report.display()),
    })?;
    Ok(run)
}

/// Reads the inventory files in `dir`; a missing directory holds none.
///
/// # Errors
///
/// Returns an error if the directory or a file cannot be read.
pub(super) fn read_files(dir: &Path) -> Result<Files, Error> {
    let mut files = Files {
        inventories: Vec::new(),
        removed: None,
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(files),
        Err(error) => return Err(Error::path_io("list the inventory", dir)(error)),
    };
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(
            entry
                .map_err(Error::path_io("list the inventory", dir))?
                .path(),
        );
    }
    paths.sort();
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".txt") else {
            continue;
        };
        let text =
            fs::read_to_string(&path).map_err(Error::path_io("read the inventory", &path))?;
        if name == REMOVED {
            files.removed = Some(text);
        } else {
            files.inventories.push((stem.to_owned(), text));
        }
    }
    Ok(files)
}

/// Writes each changed crate file into `dir`.
///
/// # Errors
///
/// Returns an error if a file cannot be written.
pub(super) fn write_files(dir: &Path, changed: &[(String, String)]) -> Result<(), Error> {
    fs::create_dir_all(dir).map_err(Error::path_io("create the inventory", dir))?;
    for (name, text) in changed {
        let path = dir.join(format!("{name}.txt"));
        fs::write(&path, text).map_err(Error::path_io("write the inventory", &path))?;
    }
    Ok(())
}

/// Runs git in `root`'s repository and returns its standard output.
fn git(root: &Path, command: &'static str, args: &[&str]) -> Result<Vec<u8>, Error> {
    let mut git = Command::new("git");
    git.current_dir(root).args(args).stderr(Stdio::inherit());
    for variable in GIT_LOCATION {
        git.env_remove(variable);
    }
    let output = git.output().map_err(Error::io(command))?;
    Error::check_status(command, output.status)?;
    Ok(output.stdout)
}

fn git_text(root: &Path, command: &'static str, args: &[&str]) -> Result<String, Error> {
    String::from_utf8(git(root, command, args)?).map_err(|_| Error::UnexpectedOutput(command))
}

/// Without `--base`: where this branch left `main`, the merge base of
/// `HEAD` and `origin/main`.
///
/// # Errors
///
/// Returns an error if git cannot find it.
pub(super) fn default_base(root: &Path) -> Result<String, Error> {
    let base = git_text(
        root,
        "git merge-base HEAD origin/main",
        &["merge-base", "HEAD", "origin/main"],
    )?;
    Ok(base.trim().to_owned())
}

/// The inventory files of revision `base`, through git, and how to show
/// the revision; `None` if that revision has no inventory directory.
///
/// # Errors
///
/// Returns an error if the revision does not exist or a file cannot be
/// read.
pub(super) fn base_files(root: &Path, base: &str) -> Result<(String, Option<Files>), Error> {
    let commit = format!("{base}^{{commit}}");
    let sha = git_text(
        root,
        "git rev-parse",
        &["rev-parse", "--verify", "--quiet", &commit],
    )?;
    let sha = sha.trim();
    let shown = format!("{base} ({sha})");
    let names = git_text(
        root,
        "git ls-tree",
        &[
            "ls-tree",
            "-z",
            "--name-only",
            sha,
            "--",
            &format!("{DIR}/"),
        ],
    )?;
    let names: Vec<&str> = names.split('\0').filter(|name| !name.is_empty()).collect();
    if names.is_empty() {
        return Ok((shown, None));
    }
    let mut files = Files {
        inventories: Vec::new(),
        removed: None,
    };
    for path in names {
        let Some(stem) = path
            .strip_prefix(&format!("{DIR}/"))
            .and_then(|name| name.strip_suffix(".txt"))
        else {
            continue;
        };
        let text = git_text(root, "git show", &["show", &format!("{sha}:{path}")])?;
        if format!("{stem}.txt") == REMOVED {
            files.removed = Some(text);
        } else {
            files.inventories.push((stem.to_owned(), text));
        }
    }
    Ok((shown, Some(files)))
}
