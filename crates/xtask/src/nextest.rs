// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! nextest's exit codes and the logged subprocess run, shared by the
//! mutation run and the planted-defect probe.
//!
//! The exit codes are those of the pinned cargo-nextest 0.9.151
//! (`nextest-metadata`, `NextestExitCode`). A scheduled job's test output
//! never reaches its log (testing specification 5.5, point 2), so both
//! commands run their tools through [`run_logged`].

use std::fs::File;
use std::path::Path;
use std::process::{ExitStatus, Stdio};

use crate::{Command, Error};

#[cfg(test)]
mod tests;

/// No test was selected to run.
pub(crate) const NO_TESTS_RUN: i32 = 4;

/// One or more tests failed.
pub(crate) const TESTS_FAILED: i32 = 100;

/// Building the tests failed.
pub(crate) const BUILD_FAILED: i32 = 101;

/// Runs `command` with standard input closed and its standard output and
/// error appended to `log`, printing nothing, and returns its status.
///
/// # Errors
///
/// Returns an error if the log cannot be opened or the command not started.
pub(crate) fn run_logged(
    command: &mut Command,
    log: &Path,
    name: &'static str,
) -> Result<ExitStatus, Error> {
    let out = File::options()
        .create(true)
        .append(true)
        .open(log)
        .map_err(Error::path_io("open tool log", log))?;
    let err = out
        .try_clone()
        .map_err(Error::path_io("open tool log", log))?;
    command
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()
        .map_err(Error::io(name))
}
