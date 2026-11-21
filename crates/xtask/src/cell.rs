// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask cell`: the disposable AFS test cell, built from one pinned
//! definition in which every input is verified (ADR 0004 L40).
//!
//! The cell runs Debian's OpenAFS and MIT Kerberos packages as peers in
//! containers, installed from packages and never linked, shipped or
//! vendored (ADR 0001, "Test tools and test data"). Its names are
//! synthetic, under the reserved `.invalid` top-level domain.

mod definition;
mod runtime;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use definition::DefinitionError;

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
            Self::Io {
                operation,
                path,
                error,
            } => write!(formatter, "{operation} at {}: {error}", path.display()),
        }
    }
}
