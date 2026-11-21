// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tree of tables and keys a file defines, and the rules that refuse a
//! second definition (`docs/specs/config-syntax.md`, section 5.4).
//!
//! Each table, explicit or implicit, is a node holding its children by
//! segment; a child is a table or a key that holds a value. A path is
//! looked up by walking from the root, so no path is copied to look it up.
//! Every node and every key/value pair counts towards
//! [`MAX_DEFINITIONS`], checked before it is stored.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::{
    Document, Entry, MAX_DEFINITIONS, Position, SyntaxError, SyntaxErrorKind, Table, Value,
};

/// The root table's index in `Builder::nodes` and `Document::paths`.
const ROOT: usize = 0;

/// What a segment names inside a table.
#[derive(Clone, Copy, Debug)]
enum Child {
    /// A table: its index.
    Table(usize),
    /// A key that holds a value, and where the key was defined.
    Value(Position),
}

/// One table, explicit or implicit.
#[derive(Debug)]
struct Node {
    children: BTreeMap<String, Child>,
    /// Whether a header of its own has defined it.
    explicit: bool,
    /// Where the header that created it is: its own, or a deeper one's.
    /// This is its first definition (`docs/specs/config-syntax.md`, section
    /// 5.4, point 4), whether or not a header later makes it explicit.
    created: Position,
}

impl Node {
    fn new(created: Position) -> Self {
        Self {
            children: BTreeMap::new(),
            explicit: false,
            created,
        }
    }

    /// Where the table was first defined, for a later redefinition's
    /// error.
    fn first(&self) -> Position {
        self.created
    }
}

/// A segment of a header, with where it starts.
pub(super) struct Segment {
    pub(super) text: String,
    pub(super) position: Position,
}

/// Builds a [`Document`], refusing every redefinition as it is made.
#[derive(Debug)]
pub(super) struct Builder {
    document: Document,
    /// Parallel to `document.paths`.
    nodes: Vec<Node>,
    definitions: usize,
}

impl Builder {
    pub(super) fn new() -> Self {
        let origin = Position { line: 1, column: 1 };
        Self {
            document: Document {
                paths: vec![Vec::new()],
                tables: Vec::new(),
                entries: Vec::new(),
            },
            nodes: vec![Node::new(origin)],
            definitions: 0,
        }
    }

    /// The root table, where key/value pairs before any header go.
    pub(super) fn root() -> usize {
        ROOT
    }

    /// Defines the table a header names, creating its missing prefixes as
    /// implicit tables, and returns its index.
    pub(super) fn open_table(
        &mut self,
        segments: Vec<Segment>,
        header: Position,
    ) -> Result<usize, SyntaxError> {
        let last = segments.len().checked_sub(1);
        let mut current = ROOT;
        for (index, segment) in segments.into_iter().enumerate() {
            let existing = self
                .nodes
                .get(current)
                .and_then(|node| node.children.get(&segment.text).copied());
            current = match existing {
                Some(Child::Table(table)) => table,
                Some(Child::Value(first)) if Some(index) == last => {
                    return Err(SyntaxError::at(
                        header,
                        SyntaxErrorKind::Redefined { first },
                    ));
                }
                Some(Child::Value(first)) => {
                    return Err(SyntaxError::at(
                        segment.position,
                        SyntaxErrorKind::NotATable { first },
                    ));
                }
                None => self.create_table(current, segment, header)?,
            };
        }
        // `current` always indexes `nodes`: it is the root or a table this
        // builder created.
        if let Some(node) = self.nodes.get_mut(current) {
            if node.explicit {
                return Err(SyntaxError::at(
                    header,
                    SyntaxErrorKind::Redefined {
                        first: node.first(),
                    },
                ));
            }
            node.explicit = true;
        }
        let path = self
            .document
            .paths
            .get(current)
            .cloned()
            .unwrap_or_default();
        self.document.tables.push(Table {
            path,
            position: header,
        });
        Ok(current)
    }

    /// Checks that `key` may be defined in `table`: it is not defined
    /// there yet, and one more definition is within the cap. The parser
    /// calls this before it reads the key's value, so an error is reported
    /// at the first place in the file where it can be seen.
    pub(super) fn check_key(
        &self,
        table: usize,
        key: &str,
        key_position: Position,
    ) -> Result<(), SyntaxError> {
        // `table` always indexes `nodes`: it came from `root` or
        // `open_table`.
        let existing = self
            .nodes
            .get(table)
            .and_then(|node| node.children.get(key).copied());
        match existing {
            Some(Child::Value(first)) => {
                return Err(SyntaxError::at(
                    key_position,
                    SyntaxErrorKind::Redefined { first },
                ));
            }
            Some(Child::Table(index)) => {
                let first = self.nodes.get(index).map_or(key_position, Node::first);
                return Err(SyntaxError::at(
                    key_position,
                    SyntaxErrorKind::Redefined { first },
                ));
            }
            None => {}
        }
        if self.definitions >= MAX_DEFINITIONS {
            return Err(SyntaxError::at(
                key_position,
                SyntaxErrorKind::TooManyDefinitions,
            ));
        }
        Ok(())
    }

    /// Adds a key/value pair to a table, after the checks of
    /// [`Builder::check_key`].
    pub(super) fn add_entry(
        &mut self,
        table: usize,
        key: String,
        key_position: Position,
        value: Value,
        value_position: Position,
    ) -> Result<(), SyntaxError> {
        self.check_key(table, &key, key_position)?;
        self.count(key_position)?;
        if let Some(node) = self.nodes.get_mut(table) {
            node.children
                .insert(key.clone(), Child::Value(key_position));
        }
        self.document.entries.push(Entry {
            table,
            key,
            key_position,
            value,
            value_position,
        });
        Ok(())
    }

    pub(super) fn finish(self) -> Document {
        self.document
    }

    /// Creates an implicit table `segment` inside `parent`.
    fn create_table(
        &mut self,
        parent: usize,
        segment: Segment,
        header: Position,
    ) -> Result<usize, SyntaxError> {
        self.count(segment.position)?;
        let index = self.nodes.len();
        let mut path = self.document.paths.get(parent).cloned().unwrap_or_default();
        path.push(segment.text.clone());
        self.document.paths.push(path);
        self.nodes.push(Node::new(header));
        if let Some(node) = self.nodes.get_mut(parent) {
            node.children.insert(segment.text, Child::Table(index));
        }
        Ok(index)
    }

    /// Counts one more definition, refusing it past the cap.
    fn count(&mut self, position: Position) -> Result<(), SyntaxError> {
        let refused = SyntaxError::at(position, SyntaxErrorKind::TooManyDefinitions);
        let next = self.definitions.checked_add(1).ok_or(refused)?;
        if next > MAX_DEFINITIONS {
            return Err(refused);
        }
        self.definitions = next;
        Ok(())
    }
}
