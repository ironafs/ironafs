// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What every call to `parse` must give, whatever the bytes
//! (`docs/specs/config-syntax.md`, section 10.4): an error positioned
//! within the input, or a document that keeps every cap of section 5.2 a
//! document can show. The property tests and the fuzz target
//! `ironafs-config_syntax` both check it; it uses only the crate's public
//! API and the standard library, so both build it from this file.

use std::collections::BTreeSet;

use ironafs_config::{
    Document, MAX_ARRAY_ELEMENTS, MAX_DEFINITIONS, MAX_DEPTH, MAX_KEY_BYTES, MAX_STRING_BYTES,
    Scalar, SyntaxError, Value,
};

/// Unicode's `Bidi_Control` set, which section 5.1, point 3 refuses,
/// written out here rather than read from the code under test.
pub(crate) const BIDI_CONTROLS: [char; 12] = [
    '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Whether a parsed key, segment or string may hold `character`: no
/// control character and no bidirectional control.
pub(crate) fn allowed_in_text(character: char) -> bool {
    !character.is_control() && !BIDI_CONTROLS.contains(&character)
}

/// Checks one call's outcome on `input`.
///
/// # Errors
///
/// Returns what was broken.
pub(crate) fn check(input: &[u8], outcome: &Result<Document, SyntaxError>) -> Result<(), String> {
    match outcome {
        Ok(document) => within_caps(document),
        Err(error) => within_input(input, error),
    }
}

/// An error's position lies within the input: its line exists, and its
/// column is at most one past the line's last character. Only a file over
/// `MAX_FILE_BYTES` has no position.
fn within_input(input: &[u8], error: &SyntaxError) -> Result<(), String> {
    let Some(position) = error.position() else {
        return if input.len() > ironafs_config::MAX_FILE_BYTES {
            Ok(())
        } else {
            Err(format!("no position: {error}"))
        };
    };
    let line = position
        .line
        .checked_sub(1)
        .and_then(|index| input.split(|byte| *byte == b'\n').nth(index));
    let Some(line) = line else {
        return Err(format!("line {} past the input", position.line));
    };
    // A column counts characters, so it is at most the line's bytes plus
    // one.
    if position.column == 0 || position.column > line.len().saturating_add(1) {
        return Err(format!("column {} outside its line", position.column));
    }
    Ok(())
}

/// The caps of section 5.2 that an accepted document can show: depth, key,
/// segment, string and array lengths, and the definitions, counting each
/// implicit table once as a proper prefix of an explicit table's path.
/// Keys, segments and strings are not empty where the subset refuses that,
/// and hold no character section 5.1, point 3 refuses. The file and line
/// caps are not visible in a document; the cap tests check them on the
/// input.
fn within_caps(document: &Document) -> Result<(), String> {
    let mut definitions: usize = 0;
    let mut tables: BTreeSet<&[String]> = BTreeSet::new();
    for table in document.tables() {
        let path = table.path();
        if path.is_empty() {
            return Err("explicit table with an empty path".to_string());
        }
        if path.len() > MAX_DEPTH {
            return Err(format!("table too deep: {path:?}"));
        }
        if path.iter().any(|segment| {
            segment.is_empty()
                || segment.len() > MAX_KEY_BYTES
                || !segment.chars().all(allowed_in_text)
        }) {
            return Err(format!("bad segment: {path:?}"));
        }
        for length in 1..=path.len() {
            tables.insert(path.get(..length).unwrap_or_default());
        }
    }
    definitions = definitions.saturating_add(tables.len());
    for (_, entry) in document.entries() {
        definitions = definitions.saturating_add(1);
        let key = entry.key();
        if key.is_empty() || key.len() > MAX_KEY_BYTES || !key.chars().all(allowed_in_text) {
            return Err(format!("bad key: {key:?}"));
        }
        let scalars: Vec<&Scalar> = match entry.value() {
            Value::Scalar(scalar) => vec![scalar],
            Value::Array(elements) => {
                if elements.len() > MAX_ARRAY_ELEMENTS {
                    return Err(format!("array too long: {}", elements.len()));
                }
                elements.iter().map(|element| &element.value).collect()
            }
        };
        for scalar in scalars {
            if let Scalar::String(text) = scalar
                && (text.len() > MAX_STRING_BYTES || !text.chars().all(allowed_in_text))
            {
                return Err(format!("bad string: {text:?}"));
            }
        }
    }
    if definitions > MAX_DEFINITIONS {
        return Err(format!("{definitions} definitions"));
    }
    Ok(())
}
