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
    MAX_LINE_BYTES, MAX_STRING_BYTES, Position, Scalar, SyntaxError, SyntaxErrorKind, Value, parse,
};

/// Unicode's `Bidi_Control` set, which section 5.1, point 3 refuses: the
/// one test-only list, written out here rather than read from the code
/// under test. U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to
/// U+2069.
pub(crate) const BIDI_CONTROLS: [char; 12] = [
    '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Whether a parsed key, segment or string may hold `character`: no
/// control character and no bidirectional control.
pub(crate) fn allowed_in_text(character: char) -> bool {
    !character.is_control() && !BIDI_CONTROLS.contains(&character)
}

/// Checks one input's parser invariants and repeats the parse.
///
/// # Panics
///
/// Panics if the parser violates an invariant or the two results differ.
#[track_caller]
pub(super) fn check_input(input: &[u8]) {
    let first = parse(input);
    let checked = check(input, &first);
    assert!(checked.is_ok(), "{checked:?}");
    assert!(parse(input) == first, "the same input gave another result");
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
/// its line end, LF or CR LF. A CR is part of the line end only before an
/// LF, so the last line, which no LF ends, keeps a final CR.
fn within_lines(input: &[u8]) -> Result<(), String> {
    let mut lines = input.split(|byte| *byte == b'\n').enumerate().peekable();
    while let Some((index, line)) = lines.next() {
        let text = if lines.peek().is_some() {
            line.strip_suffix(b"\r").unwrap_or(line)
        } else {
            line
        };
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
/// values, the CR of a CR LF not counted. Only the file's valid UTF-8
/// prefix counts: a line after the first invalid sequence does not exist,
/// and the line holding it ends just before it.
///
/// # Errors
///
/// Returns where the position falls outside.
pub(crate) fn position_within(input: &[u8], position: Position) -> Result<(), String> {
    let valid = match std::str::from_utf8(input) {
        Ok(text) => text,
        Err(error) => input
            .get(..error.valid_up_to())
            .and_then(|prefix| std::str::from_utf8(prefix).ok())
            .unwrap_or_default(),
    };
    let mut lines = valid.split('\n').peekable();
    let mut line = None;
    for number in 1.. {
        let Some(text) = lines.next() else { break };
        if number == position.line {
            let ended = lines.peek().is_some();
            line = Some(if ended {
                text.strip_suffix('\r').unwrap_or(text)
            } else {
                text
            });
            break;
        }
    }
    let Some(line) = line else {
        return Err(format!("line {} past the input", position.line));
    };
    let characters = line.chars().count();
    if position.column == 0 || position.column > characters.saturating_add(1) {
        return Err(format!(
            "column {} outside its line of {characters} characters",
            position.column
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
