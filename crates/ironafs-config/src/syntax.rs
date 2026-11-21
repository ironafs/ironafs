// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strict TOML subset of IronAFS's configuration files (ADR 0004, O18,
//! points 1 and 2; `docs/specs/config-syntax.md`).
//!
//! [`parse`] checks the whole file before it reads any line (size, byte
//! order mark, UTF-8, line ends, control characters, line length), then
//! parses the lines in order and stops at the first error. Every cap of
//! the specification's section 5.2 is checked before the bytes it bounds
//! are stored.

mod document;
mod error;
mod parser;

use alloc::string::String;
use alloc::vec::Vec;

pub use error::{Expected, Feature, SyntaxError, SyntaxErrorKind};

#[cfg(test)]
mod tests;

/// The largest file accepted, in bytes. A caller reads at most one byte
/// more, so a larger file is seen and refused.
pub const MAX_FILE_BYTES: usize = 262_144;

/// The longest line accepted, in bytes, without its line end.
pub const MAX_LINE_BYTES: usize = 4_096;

/// The longest key or table-path segment, in bytes, after escapes. A
/// quoted cell name of 253 bytes fits.
pub const MAX_KEY_BYTES: usize = 255;

/// The most segments in one table header.
pub const MAX_DEPTH: usize = 8;

/// The longest string value, in bytes, after escapes.
pub const MAX_STRING_BYTES: usize = 1_024;

/// The most elements in one array.
pub const MAX_ARRAY_ELEMENTS: usize = 256;

/// The most definitions in one file: key/value pairs, plus tables,
/// explicit or created implicitly by a deeper header.
pub const MAX_DEFINITIONS: usize = 4_096;

/// Where something is in a file: a line and a column, both from 1. The
/// column counts Unicode scalar values, so a tab or a non-ASCII character
/// counts as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// The line, from 1.
    pub line: usize,
    /// The column, from 1, in Unicode scalar values.
    pub column: usize,
}

/// A value that is not an array.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    /// A basic string, with its escapes applied.
    String(String),
    /// A decimal integer.
    Integer(i64),
    /// `true` or `false`.
    Boolean(bool),
}

/// One element of an array, with its position.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Element {
    /// The element's value. Every element of one array has the same type.
    pub value: Scalar,
    /// Where the element starts.
    pub position: Position,
}

/// The value of a key/value pair.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// A string, an integer or a boolean.
    Scalar(Scalar),
    /// An array of scalars of one type, possibly empty.
    Array(Vec<Element>),
}

/// A key/value pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    table: usize,
    key: String,
    key_position: Position,
    value: Value,
    value_position: Position,
}

impl Entry {
    /// The key, with a quoted key's escapes applied.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Where the key starts.
    #[must_use]
    pub fn key_position(&self) -> Position {
        self.key_position
    }

    /// The value.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Where the value starts.
    #[must_use]
    pub fn value_position(&self) -> Position {
        self.value_position
    }
}

/// A table declared by a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    path: Vec<String>,
    position: Position,
}

impl Table {
    /// The table's path: the header's segments, in order.
    #[must_use]
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Where the header starts.
    #[must_use]
    pub fn position(&self) -> Position {
        self.position
    }
}

/// A parsed file: the tables its headers declare, and its key/value pairs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// The path of every table, explicit or implicit; index 0 is the root.
    paths: Vec<Vec<String>>,
    /// The explicit tables, in the order of their headers.
    tables: Vec<Table>,
    /// The key/value pairs in file order; each names its table by index
    /// into `paths`.
    entries: Vec<Entry>,
}

impl Document {
    /// The tables declared by headers, in file order. Tables created only
    /// implicitly, as prefixes of a deeper header, are not listed.
    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.tables.iter()
    }

    /// The key/value pairs in file order, each with its table's path (empty
    /// for the root table).
    pub fn entries(&self) -> impl Iterator<Item = (&[String], &Entry)> {
        self.entries.iter().map(|entry| {
            let path = self.paths.get(entry.table).map_or(&[][..], Vec::as_slice);
            (path, entry)
        })
    }
}

/// Parses one configuration file.
///
/// # Errors
///
/// Returns a [`SyntaxError`] for the first thing in the file that is not
/// in the subset, or that breaks a cap: the error's kind says what, and
/// its position says where.
pub fn parse(input: &[u8]) -> Result<Document, SyntaxError> {
    parser::parse(input)
}
