// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask cell`: the disposable AFS test cell, built from one pinned
//! definition in which every input is verified (ADR 0004 L40).
//!
//! The cell runs Debian's OpenAFS and MIT Kerberos packages as peers in
//! containers, installed from packages and never linked, shipped or
//! vendored (ADR 0001, "Test tools and test data"). Its names are
//! synthetic, under the reserved `.invalid` top-level domain.

mod archive;
mod definition;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(#47): `cell up` installs the cell image; it lands after the hosted-runner spike"
    )
)]
mod install;
mod pin;
mod runtime;
mod state;

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::args::CellCommand;
use crate::{Error, check, metadata};
use runtime::{Engine, EngineRun};
use state::{RunDir, State};

use definition::DefinitionError;
use install::InstallViolation;

#[cfg(test)]
mod tests;

/// Why a cell command stopped.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The pin file has errors, each listed.
    Definition(Vec<DefinitionError>),
    /// No container engine to run the cell on; names what was looked for.
    EngineNotFound(&'static str),
    /// An engine step ran and failed.
    Step {
        name: &'static str,
        code: Option<i32>,
        /// The end of the step's error output, unless the step is secret.
        detail: String,
    },
    /// apt's archive update reported an unsigned, unverified or stale
    /// archive; the line it reported.
    ArchiveNotVerified(String),
    /// The installed packages' bytes or versions differ from the pins.
    Install(Vec<InstallViolation>),
    /// A step's output could not be read; the step and the 1-based line.
    UnexpectedOutput { step: &'static str, line: usize },
    /// The run directory already exists or already holds keys.
    RunDirInUse(PathBuf),
    /// Teardown could not remove everything; each failure is listed, and
    /// what was not removed stays recorded.
    Teardown(Vec<Failure>),
    /// A command failed, and so did the teardown after it.
    AndTeardown {
        failure: Box<Failure>,
        teardown: Box<Failure>,
    },
    /// A file or directory could not be read or written.
    Io {
        operation: &'static str,
        path: PathBuf,
        error: String,
    },
}

impl Failure {
    /// Maps an I/O error at `path` to [`Failure::Io`].
    pub(crate) fn io(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        move |error| Self::Io {
            operation,
            path: path.to_path_buf(),
            error: error.to_string(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Definition(errors) => {
                write!(formatter, "the cell pin file has {} errors:", errors.len())?;
                errors
                    .iter()
                    .try_for_each(|error| write!(formatter, "\n  {error}"))
            }
            Self::EngineNotFound(wanted) => {
                write!(formatter, "no container engine: wanted {wanted}")
            }
            Self::Step { name, code, detail } => {
                match code {
                    Some(code) => write!(formatter, "step {name} failed with exit code {code}"),
                    None => write!(formatter, "step {name} was ended by a signal"),
                }?;
                write!(formatter, ":\n{detail}")
            }
            Self::ArchiveNotVerified(line) => write!(
                formatter,
                "apt could not verify the snapshot archive's signature: {line}"
            ),
            Self::Install(violations) => {
                write!(
                    formatter,
                    "the install differs from the pins in {} ways:",
                    violations.len()
                )?;
                violations
                    .iter()
                    .try_for_each(|violation| write!(formatter, "\n  {violation}"))
            }
            Self::UnexpectedOutput { step, line } => {
                write!(formatter, "step {step} printed an unexpected line {line}")
            }
            Self::RunDirInUse(path) => {
                write!(formatter, "the run directory {} is in use", path.display())
            }
            Self::Teardown(failures) => {
                write!(
                    formatter,
                    "teardown left {} failures; what was not removed stays recorded in the run directory:",
                    failures.len()
                )?;
                failures
                    .iter()
                    .try_for_each(|failure| write!(formatter, "\n  {failure}"))
            }
            Self::AndTeardown { failure, teardown } => {
                write!(formatter, "{failure}\nthen teardown failed: {teardown}")
            }
            Self::Io {
                operation,
                path,
                error,
            } => write!(formatter, "{operation} at {}: {error}", path.display()),
        }
    }
}

/// Combines a command's result with its teardown's: both failures are kept
/// when both fail.
fn finish<T>(result: Result<T, Failure>, teardown: Result<(), Failure>) -> Result<T, Failure> {
    match (result, teardown) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(failure), Ok(())) | (Ok(_), Err(failure)) => Err(failure),
        (Err(failure), Err(teardown)) => Err(Failure::AndTeardown {
            failure: Box::new(failure),
            teardown: Box::new(teardown),
        }),
    }
}

/// Runs a cell command.
///
/// # Errors
///
/// Returns [`Error::Cell`] if the command fails, and an error if the
/// workspace cannot be read or the output cannot be written.
pub(super) fn run(command: CellCommand, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    match command {
        CellCommand::Pin => {
            let pins = pin_command(&workspace.target_dir).map_err(Error::Cell)?;
            output
                .write_all(pins.as_bytes())
                .map_err(Error::io("write the pin file"))
        }
    }
}

/// `cell pin`: resolves the closure and returns the new pin file, which
/// must still parse with every required pin.
fn pin_command(target_dir: &Path) -> Result<String, Failure> {
    let (text, definition) = definition::load()?;
    let engine = Engine::choose(
        std::env::var_os(runtime::ENGINE_VARIABLE).as_deref(),
        check::in_path,
    )?;
    let dir = RunDir::create_new(target_dir, std::process::id())?;
    let mut engine = match EngineRun::new(engine, &dir.root) {
        Ok(engine) => engine,
        Err(failure) => {
            let removed = fs::remove_dir_all(&dir.root)
                .map_err(Failure::io("remove the run directory", &dir.root));
            return finish(Err(failure), removed);
        }
    };
    let mut state = State::default();
    let result = pin::pin(&mut engine, &definition, &dir, &mut state);
    let torn_down = state::teardown(&mut engine, &dir, &state);
    let packages = finish(result, torn_down)?;
    let rendered = pin::render(&text, &packages);
    definition::parse(&rendered).map_err(Failure::Definition)?;
    Ok(rendered)
}
