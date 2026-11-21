// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract's name of one directory entry, and the per-system name
//! codecs (ADR 0004, C7, point 1; L29).
//!
//! The core keeps a name's raw bytes and never normalises them (L29,
//! point 1): two names are the same only if their bytes are. A [`Name`]
//! is one path component, never a path, and never `.` or `..`, so no
//! name a layer or a server gives can climb out of its directory.
//!
//! The codecs turn the bytes an operating system gives into a [`Name`]
//! and back. The Linux codec ([`linux`]) shows the bytes (L29, point 2).
//! The macOS lookup fallback (L29, points 3 to 6) and the Windows name
//! table (L24, L25) are not here yet.

use alloc::vec::Vec;
use core::fmt;

pub mod linux;

#[cfg(test)]
mod tests;

/// The most bytes one name holds: Linux's `NAME_MAX`, the longest
/// component its system calls accept.
pub const MAX_NAME_BYTES: usize = 255;

/// The raw bytes of one directory entry's name.
///
/// Its bytes are 1 to [`MAX_NAME_BYTES`] long, hold no `/` and no NUL, and
/// are neither `.` nor `..`; [`Name::new`] is its only constructor and
/// checks all four. Bytes need not be UTF-8.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(Vec<u8>);

impl Name {
    /// The name with these bytes, unchanged.
    ///
    /// # Errors
    ///
    /// The first of these that applies: [`NameError::Empty`],
    /// [`NameError::TooLong`], [`NameError::Dot`], [`NameError::DotDot`],
    /// [`NameError::Slash`], [`NameError::Nul`].
    pub fn new(bytes: &[u8]) -> Result<Self, NameError> {
        if bytes.is_empty() {
            return Err(NameError::Empty);
        }
        if bytes.len() > MAX_NAME_BYTES {
            return Err(NameError::TooLong);
        }
        if bytes == b"." {
            return Err(NameError::Dot);
        }
        if bytes == b".." {
            return Err(NameError::DotDot);
        }
        if bytes.contains(&b'/') {
            return Err(NameError::Slash);
        }
        if bytes.contains(&0) {
            return Err(NameError::Nul);
        }
        Ok(Self(bytes.to_vec()))
    }

    /// The name's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Name(\"")?;
        for byte in &self.0 {
            write!(formatter, "{}", byte.escape_ascii())?;
        }
        formatter.write_str("\")")
    }
}

/// Why bytes are not a [`Name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameError {
    /// No bytes.
    Empty,
    /// The name of the directory itself.
    Dot,
    /// The name of the parent directory.
    DotDot,
    /// A `/`, which separates path components.
    Slash,
    /// A NUL byte, which ends a name in the systems' interfaces.
    Nul,
    /// More than [`MAX_NAME_BYTES`] bytes.
    TooLong,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the name is empty",
            Self::Dot => "the name is `.`",
            Self::DotDot => "the name is `..`",
            Self::Slash => "the name holds a `/`",
            Self::Nul => "the name holds a nul byte",
            Self::TooLong => "the name is longer than 255 bytes",
        })
    }
}

impl core::error::Error for NameError {}
