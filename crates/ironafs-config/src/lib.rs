// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's configuration: the parser for its strict subset of TOML, and
//! the canonical cell name (ADR 0004, C4, point 1; O18).
//!
//! [`parse`] turns the bytes of one configuration file into a
//! [`Document`], or refuses them with a positioned [`SyntaxError`]. It
//! accepts only the constructs `docs/specs/config-syntax.md` lists, and
//! refuses every other TOML construct rather than skip or repair it, so a
//! file it accepts means the same here as in TOML. Finding and reading the
//! files, their order and the file name an error names are the caller's.
//!
//! [`CellName`] is the one type for a cell's name. Its constructor is the
//! only way to make one, and applies the rule every cell name passes,
//! whatever its source (the specification, section 5.7).
//!
//! ```
//! use ironafs_config::{CellName, Scalar, Value, parse};
//!
//! let document = parse(b"[cells.\"Example.ORG.\"]\ndiscovery = false\n")?;
//! let (table, entry) = document.entries().next().ok_or("no entry")?;
//! let [_, cell] = table else { return Err("not a cell table".into()) };
//! assert_eq!(CellName::new(cell.as_bytes())?.as_str(), "example.org");
//! assert_eq!(entry.key(), "discovery");
//! assert_eq!(entry.value(), &Value::Scalar(Scalar::Boolean(false)));
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

#![no_std]

extern crate alloc;

mod cell_name;
mod syntax;

pub use cell_name::{CellName, CellNameError};
pub use syntax::{
    Document, Element, Entry, Expected, Feature, MAX_ARRAY_ELEMENTS, MAX_DEFINITIONS, MAX_DEPTH,
    MAX_FILE_BYTES, MAX_KEY_BYTES, MAX_LINE_BYTES, MAX_STRING_BYTES, Position, Scalar, SyntaxError,
    SyntaxErrorKind, Table, Value, parse,
};
