// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What every call to `parse` must give, whatever the bytes
//! (`docs/specs/config-syntax.md`, section 10.4): an error positioned
//! within the input, or an accepted input that keeps every cap of section
//! 5.2. The property tests and the fuzz target
//! `ironafs-config_syntax` both check it; it uses only the crate's public
//! API and the standard library, so both build it from this file.

use std::collections::BTreeSet;

use ironafs_config::{
    Document, MAX_ARRAY_ELEMENTS, MAX_DEFINITIONS, MAX_DEPTH, MAX_FILE_BYTES, MAX_KEY_BYTES,
    MAX_LINE_BYTES, MAX_STRING_BYTES, Position, Scalar, SyntaxError, SyntaxErrorKind, Value,
};

/// The bidirectional controls, from the one test-only list the unit tests
/// share.
#[path = "bidi_controls.rs"]
mod bidi_controls;

pub(crate) use bidi_controls::BIDI_CONTROLS;

/// Whether a parsed key, segment or string may hold `character`: no
/// control character and no bidirectional control.
pub(crate) fn allowed_in_text(character: char) -> bool {
    !character.is_control() && !BIDI_CONTROLS.contains(&character)
}

/// Checks one call's outcome on `input`: a file over `MAX_FILE_BYTES` is
/// refused as too large, with no position, and nothing else is; any other
/// error lies within the input; an accepted input keeps the file and line
/// caps, and its document every cap it can show.
///
/// # Errors
///
/// Returns what was broken.
pub(crate) fn check(input: &[u8], outcome: &Result<Document, SyntaxError>) -> Result<(), String> {
    let too_large = input.len() > MAX_FILE_BYTES;
    match outcome {
        Err(error) if too_large => {
            if error.kind() == SyntaxErrorKind::FileTooLarge && error.position().is_none() {
                Ok(())
            } else {
                Err(format!("a file over the cap gave {error}"))
            }
        }
        Ok(_) if too_large => Err(format!("accepted a file of {} bytes", input.len())),
        Ok(document) => {
            within_lines(input)?;
            within_caps(document)
        }
        Err(error) => match error.position() {
            Some(position) => position_within(input, position),
            None => Err(format!("no position: {error}")),
        },
    }
}

/// Every line of an accepted input is at most `MAX_LINE_BYTES`, without
/// its line end, LF or CR LF.
fn within_lines(input: &[u8]) -> Result<(), String> {
    for (index, line) in input.split(|byte| *byte == b'\n').enumerate() {
        let text = line.strip_suffix(b"\r").unwrap_or(line);
        if text.len() > MAX_LINE_BYTES {
            return Err(format!(
                "accepted line {} of {} bytes",
                index + 1,
                text.len()
            ));
        }
    }
    Ok(())
}

/// A position lies within the input: its line exists, and its column is at
/// most one past the line's last character, counted in Unicode scalar
/// values. In a line that is not UTF-8, only its valid prefix counts, so
/// the latest column is the one just after that prefix.
///
/// # Errors
///
/// Returns where the position falls outside.
pub(crate) fn position_within(input: &[u8], position: Position) -> Result<(), String> {
    let line = position
        .line
        .checked_sub(1)
        .and_then(|index| input.split(|byte| *byte == b'\n').nth(index));
    let Some(line) = line else {
        return Err(format!("line {} past the input", position.line));
    };
    let valid = match std::str::from_utf8(line) {
        Ok(text) => text,
        Err(error) => line
            .get(..error.valid_up_to())
            .and_then(|prefix| std::str::from_utf8(prefix).ok())
            .unwrap_or_default(),
    };
    let last = valid.chars().count().saturating_add(1);
    if position.column == 0 || position.column > last {
        return Err(format!(
            "column {} outside its line of {} characters",
            position.column,
            last - 1
        ));
    }
    Ok(())
}

/// The caps of section 5.2 that an accepted document can show: depth, key,
/// segment, string and array lengths, and the definitions, counting each
/// implicit table once as a proper prefix of an explicit table's path.
/// Keys, segments and strings are not empty where the subset refuses that,
/// and hold no character section 5.1, point 3 refuses. The file and line
/// caps are checked on the input, in `check`.
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
