// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `cargo xtask` entry point and its diagnostic and exit-status handling.

mod args;
mod check;
mod hooks;
mod trailers;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

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
    HookIo {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Trailer(trailers::Violation),
}

impl Error {
    fn io(operation: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::Io { operation, source }
    }

    fn hook_io(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::HookIo {
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
            Self::HookIo {
                operation,
                path,
                source,
            } => {
                write!(formatter, "{operation} at {}: {source}", path.display())
            }
            Self::Trailer(violation) => write!(formatter, "{violation}"),
        }
    }
}

fn run_task(
    arguments: impl Iterator<Item = OsString>,
    output: &mut impl Write,
) -> Result<(), Error> {
    match args::parse(arguments).map_err(Error::Usage)? {
        args::Task::Check => check::run(output),
        args::Task::InstallHook { force } => hooks::install(force, output),
        args::Task::UninstallHook => hooks::uninstall(),
        args::Task::Trailers(mode) => trailers::run(mode, output),
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
    match run_task(arguments, output) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(diagnostics, "xtask: {error}");
            error.exit_code()
        }
    }
}
