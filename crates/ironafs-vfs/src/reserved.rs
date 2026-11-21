// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The operations reserved in the contract and refused until a volume
//! offers them (ADR 0004, N46, point 1).
//!
//! An operation present and refused is a capability check when a volume
//! first offers it; an operation absent from the contract would be a
//! contract change.

use crate::VfsError;

#[cfg(test)]
mod tests;

/// A reserved operation (N46, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reserved {
    /// Copy-range, with or without the clone flag: an atomic clone only
    /// with the volume's capability. Without it, callers fall back to
    /// reading and writing (N46, point 3).
    CopyRange {
        /// Whether the caller asked for an atomic clone.
        clone: bool,
    },
    /// Allocating a range.
    Allocate,
    /// Punching a hole.
    Punch,
    /// Writing or removing an extended attribute. Reads are an ordinary
    /// caller operation (C7, point 2).
    ExtendedAttributeWrite,
    /// Selecting a named stream of a file.
    NamedStream,
    /// A rename with flags; no-replace only with an atomic capability.
    RenameFlags {
        /// Whether the caller asked that the target not be replaced.
        no_replace: bool,
    },
}

impl Reserved {
    /// The error the core answers a reserved operation with while no
    /// volume capability offers it: not supported (N46, point 1).
    #[must_use]
    pub const fn refusal(self) -> VfsError {
        VfsError::NotSupported
    }
}
