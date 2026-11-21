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

use std::fmt;
use std::path::PathBuf;

use definition::DefinitionError;

/// Why a cell command stopped.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The pin file has errors, each listed.
    Definition(Vec<DefinitionError>),
    /// A file or directory could not be read or written.
    Io {
        operation: &'static str,
        path: PathBuf,
        error: String,
    },
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
            Self::Io {
                operation,
                path,
                error,
            } => write!(formatter, "{operation} at {}: {error}", path.display()),
        }
    }
}
