// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `cargo xtask` entry point and its diagnostic and exit-status handling.

mod abi;
mod args;
mod bans;
mod cell;
mod check;
mod deps;
mod files;
mod fixture;
mod fuzz;
mod fuzz_deny;
mod grants;
mod hooks;
mod json;
mod metadata;
mod os;
mod pure;
mod rust_source;
mod seams;
mod trailers;
mod wipe;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// The subprocess builder for repository automation: xtask runs cargo,
/// git, rustup and the checks' tools. Coding standards rule 7 bans
/// `std::process::Command` everywhere, since a subprocess can read the
/// host's clock or randomness; xtask is never shipped, so it is exempt by
/// this one alias, and every other ban still applies to it.
#[expect(
    clippy::disallowed_types,
    reason = "rule 7: xtask is repository automation, never shipped; it runs tools as \
              subprocesses"
)]
pub(crate) type Command = std::process::Command;

#[derive(Debug)]
enum Error {
    Usage(args::UsageError),
    Io {
        operation: &'static str,
        source: io::Error,
    },
    Failed {
        command: &'static str,
        status: ExitStatus,
    },
    UnexpectedOutput(&'static str),
    ForeignHook(PathBuf),
    PathIo {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Trailer(trailers::Violation),
    /// A test cell command failed.
    Cell(cell::Failure),
    /// A rule check found violations, each written out.
    Rules {
        check: &'static str,
        violations: Vec<String>,
    },
    /// A self-test case did not end as planted.
    SelfTest {
        check: &'static str,
        case: &'static str,
        found: Vec<String>,
    },
}

impl Error {
    fn io(operation: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::Io { operation, source }
    }

    fn path_io(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::PathIo {
            operation,
            path: path.to_path_buf(),
            source,
        }
    }

    fn check_status(command: &'static str, status: ExitStatus) -> Result<(), Self> {
        if status.success() {
            Ok(())
        } else {
            Err(Self::Failed { command, status })
        }
    }

    fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Failed { status, .. } => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;

                    if let Some(signal) = status.signal() {
                        return 128 + signal;
                    }
                }
                status.code().unwrap_or(1)
            }
            _ => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(error) => write!(formatter, "{error}"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::Failed { command, status } => write!(formatter, "{command} failed with {status}"),
            Self::UnexpectedOutput(command) => {
                write!(formatter, "unexpected output from {command}")
            }
            Self::ForeignHook(path) => {
                write!(
                    formatter,
                    "hook at {} was not written by xtask",
                    path.display()
                )
            }
            Self::PathIo {
                operation,
                path,
                source,
            } => {
                write!(formatter, "{operation} at {}: {source}", path.display())
            }
            Self::Trailer(violation) => write!(formatter, "{violation}"),
            Self::Cell(failure) => write!(formatter, "cell: {failure}"),
            Self::Rules { check, violations } => {
                write!(formatter, "{check} found {} violations:", violations.len())?;
                violations
                    .iter()
                    .try_for_each(|violation| write!(formatter, "\n  {violation}"))
            }
            Self::SelfTest { check, case, found } => {
                write!(
                    formatter,
                    "{check} --self-test: case \"{case}\" did not end as planted; found {}:",
                    found.len()
                )?;
                found
                    .iter()
                    .try_for_each(|violation| write!(formatter, "\n  {violation}"))
            }
        }
    }
}

fn run_task(
    arguments: impl Iterator<Item = OsString>,
    output: &mut impl Write,
) -> Result<(), Error> {
    match args::parse(arguments).map_err(Error::Usage)? {
        args::Task::Check => check::run(output),
        args::Task::CheckBans { self_test } => bans::run(self_test, output),
        args::Task::CheckDeps { self_test } => deps::run(self_test, output),
        args::Task::CheckFuzzDeny { self_test } => fuzz_deny::run(self_test, output),
        args::Task::Fuzz { self_test } => fuzz::run(self_test, output),
        args::Task::CheckGrants { self_test } => grants::run(self_test, output),
        args::Task::CheckPure { self_test } => pure::run(self_test, output),
        args::Task::CheckOs { self_test } => os::run(self_test, output),
        args::Task::CheckAbi {
            self_test,
            target,
            mode,
        } => abi::run(self_test, target.as_deref(), mode, output),
        args::Task::CheckSeams { self_test } => seams::run(self_test, output),
        args::Task::CheckWipe { self_test } => wipe::run(self_test, output),
        args::Task::InstallHook { force } => hooks::install(force, output),
        args::Task::UninstallHook => hooks::uninstall(),
        args::Task::Trailers(mode) => trailers::run(mode, output),
        args::Task::Cell(command) => cell::run(command, output),
    }
}

/// Runs the selected task and returns its process exit status.
///
/// Pass arguments after the executable name. Usage errors return 2 and write
/// a diagnostic.
///
/// ```
/// let mut output = Vec::new();
/// let mut diagnostics = Vec::new();
/// let status = xtask::run(std::iter::empty(), &mut output, &mut diagnostics);
/// assert_eq!(status, 2);
/// assert!(diagnostics.starts_with(b"xtask: missing command\nusage: cargo xtask check"));
/// ```
pub fn run(
    arguments: impl Iterator<Item = OsString>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    let result = match std::env::var_os(wipe::capture::SESSION_ENV) {
        Some(session) => wipe::capture::forward(Path::new(&session), arguments, diagnostics),
        None => run_task(arguments, output).map(|()| 0),
    };
    match result {
        Ok(status) => status,
        Err(error) => {
            let _ = writeln!(diagnostics, "xtask: {error}");
            error.exit_code()
        }
    }
}
