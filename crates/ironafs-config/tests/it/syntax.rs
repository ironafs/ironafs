// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties of the parser: any bytes give a document or a positioned
//! error within the input, an accepted document keeps the caps a document
//! can show, a file near the definitions cap is accepted exactly when it is
//! within it, and a generated document written in the subset parses back
//! to itself.

use std::fmt::Write;

use ironafs_config::{
    MAX_DEFINITIONS, MAX_DEPTH, MAX_FILE_BYTES, MAX_LINE_BYTES, Position, Scalar, Value, parse,
};
use ironafs_test_support::{Property, Source};

use crate::invariants;

/// Pieces of TOML and of what the subset refuses, so arbitrary inputs
/// reach deep into the grammar.
const FRAGMENTS: &[&str] = &[
    "[",
    "]",
    "[[",
    "\"",
    "'",
    "=",
    " = ",
    "a",
    "k",
    "1",
    "0",
    "-",
    "_",
    ".",
    ",",
    "#",
    "\n",
    "\r\n",
    "\r",
    " ",
    "\t",
    "{",
    "}",
    "\\",
    "\\\"",
    "\\n",
    "true",
    "false",
    "1.5",
    "0x1",
    "+",
    "1979-05-27",
    ":",
    "\u{e9}",
    "\u{85}",
    "\u{202e}",
    "\u{2066}",
    "\u{0}",
    "\u{7f}",
    "\"\"\"",
    "9223372036854775808",
];

/// Arbitrary pieces, half the time inside a string, a header or an array,
/// so that what a string or an array may hold is reached as often as what
/// a line may hold.
fn fragments(source: &mut Source) -> Vec<u8> {
    let (open, close) = *source.pick(&[
        ("", ""),
        ("", ""),
        ("", ""),
        ("k = \"", "\""),
        ("[\"", "\"]"),
        ("k = [", "]"),
    ]);
    let mut bytes = open.as_bytes().to_vec();
    for _ in 0..source.usize_in(0..=48) {
        if source.weighted(&[15, 1]) == 0 {
            bytes.extend_from_slice(source.pick(FRAGMENTS).as_bytes());
        } else {
            bytes.push(source.u8());
        }
    }
    bytes.extend_from_slice(close.as_bytes());
    bytes
}

#[test]
fn parse_any_bytes_returns_document_or_error_within_input() {
    Property::new("parse: any bytes")
        .cases(4_096)
        .check(fragments, |input| invariants::check(input, &parse(input)));
}

/// Key/value pairs in the order they are written.
type Entries = Vec<(Key, Value)>;

/// Tables, each with its path and its entries.
type Tables = Vec<(Vec<Key>, Entries)>;

/// A generated document: root entries, then tables with their entries.
#[derive(Debug, Clone)]
struct Model {
    root: Entries,
    tables: Tables,
}

/// A key or segment, and whether to write it quoted.
#[derive(Debug, Clone)]
struct Key {
    text: String,
    quoted: bool,
}

/// Text with no control character and no bidirectional control, which a
/// string may hold.
fn text(source: &mut Source, max_bytes: usize) -> String {
    source
        .string(max_bytes)
        .chars()
        .filter(|character| invariants::allowed_in_text(*character))
        .collect()
}

/// A key unique by `prefix` and `index`: the text up to the first `-` is
/// different for every index.
fn key(source: &mut Source, prefix: char, index: usize) -> Key {
    let bare = source.bool();
    let suffix: String = if bare {
        source
            .ascii_string(8)
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || "_-".contains(*character))
            .collect()
    } else {
        text(source, 12)
    };
    Key {
        text: format!("{prefix}{index}-{suffix}"),
        quoted: !bare || source.bool(),
    }
}

fn scalar(source: &mut Source, kind: usize) -> Scalar {
    match kind {
        0 => Scalar::String(text(source, 40)),
        1 => Scalar::Integer(source.i64()),
        _ => Scalar::Boolean(source.bool()),
    }
}

fn value(source: &mut Source) -> Value {
    let kind = source.usize_in(0..=2);
    if source.weighted(&[3, 1]) == 0 {
        Value::Scalar(scalar(source, kind))
    } else {
        let elements = source.vec_of(0..=4, |source| scalar(source, kind));
        Value::Array(
            elements
                .into_iter()
                .map(|value| ironafs_config::Element {
                    value,
                    position: ironafs_config::Position { line: 0, column: 0 },
                })
                .collect(),
        )
    }
}

fn entries(source: &mut Source) -> Vec<(Key, Value)> {
    let count = source.usize_in(0..=4);
    (0..count)
        .map(|index| (key(source, 'k', index), value(source)))
        .collect()
}

fn model(source: &mut Source) -> Model {
    let root = entries(source);
    let mut tables: Tables = Vec::new();
    for _ in 0..source.usize_in(0..=4) {
        let depth = source.usize_in(1..=3);
        // Segment indexes from a small range, so tables share prefixes.
        let path: Vec<Key> = (0..depth)
            .map(|_| {
                let index = source.usize_in(0..=2);
                Key {
                    text: format!("t{index}"),
                    quoted: source.bool(),
                }
            })
            .collect();
        let texts: Vec<&String> = path.iter().map(|segment| &segment.text).collect();
        let seen = tables.iter().any(|(other, _)| {
            other
                .iter()
                .map(|segment| &segment.text)
                .collect::<Vec<_>>()
                == texts
        });
        let table_entries = entries(source);
        if !seen {
            tables.push((path, table_entries));
        }
    }
    Model { root, tables }
}

fn quote(text: &str) -> String {
    let mut quoted = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn write_key(key: &Key) -> String {
    if key.quoted {
        quote(&key.text)
    } else {
        key.text.clone()
    }
}

fn write_scalar(scalar: &Scalar) -> String {
    match scalar {
        Scalar::String(text) => quote(text),
        Scalar::Integer(value) => value.to_string(),
        Scalar::Boolean(value) => value.to_string(),
    }
}

/// Writes the model in the subset, varying whitespace, comments and line
/// ends with draws from `source`.
fn render(model: &Model, source: &mut Source) -> String {
    let mut out = String::new();
    let space = |source: &mut Source| *source.pick(&["", " ", "\t", "  "]);
    let end = |source: &mut Source| *source.pick(&["\n", "\r\n", " # note\n", "\t#\r\n"]);
    let write_entries = |out: &mut String, list: &[(Key, Value)], source: &mut Source| {
        for (key, value) in list {
            let text = match value {
                Value::Scalar(scalar) => write_scalar(scalar),
                Value::Array(elements) => {
                    let parts: Vec<String> = elements
                        .iter()
                        .map(|element| write_scalar(&element.value))
                        .collect();
                    let separator = *source.pick(&[",", ", ", ",\n  ", ", # c\n"]);
                    format!("[{}]", parts.join(separator))
                }
            };
            let (before, after) = (space(source), space(source));
            let line_end = end(source);
            write!(
                out,
                "{before}{}{after}={after}{text}{line_end}",
                write_key(key)
            )
            .unwrap();
        }
    };
    write_entries(&mut out, &model.root, source);
    for (path, list) in &model.tables {
        let separator = *source.pick(&[".", " . "]);
        let segments: Vec<String> = path.iter().map(write_key).collect();
        let line_end = end(source);
        write!(out, "[{}]{line_end}", segments.join(separator)).unwrap();
        write_entries(&mut out, list, source);
    }
    out
}

/// A value without positions, for comparison.
fn plain(value: &Value) -> Vec<Scalar> {
    match value {
        Value::Scalar(scalar) => vec![scalar.clone()],
        Value::Array(elements) => elements
            .iter()
            .map(|element| element.value.clone())
            .collect(),
    }
}

type Flat = Vec<(Vec<String>, String, bool, Vec<Scalar>)>;

fn expected_entries(model: &Model) -> Flat {
    let flat = |path: Vec<String>, list: &[(Key, Value)]| -> Flat {
        list.iter()
            .map(|(key, value)| {
                let is_array = matches!(value, Value::Array(_));
                (path.clone(), key.text.clone(), is_array, plain(value))
            })
            .collect()
    };
    let mut all = flat(Vec::new(), &model.root);
    for (path, list) in &model.tables {
        let texts = path.iter().map(|segment| segment.text.clone()).collect();
        all.extend(flat(texts, list));
    }
    all
}

#[test]
fn parse_generated_document_reads_back_its_model() {
    Property::new("parse: generated documents round-trip")
        .cases(1_024)
        .check(
            |source| {
                let model = model(source);
                let text = render(&model, source);
                (model, text)
            },
            |(model, text)| {
                let document = parse(text.as_bytes()).map_err(|error| format!("{error}"))?;
                let tables: Vec<Vec<String>> = document
                    .tables()
                    .map(|table| table.path().to_vec())
                    .collect();
                let want_tables: Vec<Vec<String>> = model
                    .tables
                    .iter()
                    .map(|(path, _)| path.iter().map(|segment| segment.text.clone()).collect())
                    .collect();
                if tables != want_tables {
                    return Err(format!("tables {tables:?}, expected {want_tables:?}"));
                }
                let found: Flat = document
                    .entries()
                    .map(|(path, entry)| {
                        let is_array = matches!(entry.value(), Value::Array(_));
                        (
                            path.to_vec(),
                            entry.key().to_string(),
                            is_array,
                            plain(entry.value()),
                        )
                    })
                    .collect();
                let want = expected_entries(model);
                if found != want {
                    return Err(format!("entries {found:?}, expected {want:?}"));
                }
                Ok(())
            },
        );
}

/// A file of `keys` key/value pairs, then one header `depth` segments deep
/// whose every prefix is a new table.
fn keys_then_header(keys: usize, depth: usize) -> String {
    let mut text = String::new();
    for index in 0..keys {
        writeln!(text, "k{index} = 1").unwrap();
    }
    let segments: Vec<String> = (0..depth).map(|index| format!("s{index}")).collect();
    writeln!(text, "[{}]", segments.join(".")).unwrap();
    text
}

#[test]
fn parse_near_definitions_cap_counts_every_implicit_table() {
    Property::new("parse: a deep header at the definitions cap")
        .cases(64)
        .check(
            |source| {
                let depth = source.usize_in(1..=MAX_DEPTH);
                let keys = source.usize_in(MAX_DEFINITIONS - MAX_DEPTH - 2..=MAX_DEFINITIONS);
                (keys, depth)
            },
            |&(keys, depth)| {
                let within = keys + depth <= MAX_DEFINITIONS;
                let input = keys_then_header(keys, depth);
                let outcome = parse(input.as_bytes());
                match outcome {
                    Ok(_) if within => invariants::check(input.as_bytes(), &outcome),
                    Ok(_) => Err(format!("accepted {} definitions", keys + depth)),
                    Err(error) if !within => {
                        if error.kind() == ironafs_config::SyntaxErrorKind::TooManyDefinitions {
                            Ok(())
                        } else {
                            Err(format!("refused for another reason: {error}"))
                        }
                    }
                    Err(error) => Err(format!("refused within the cap: {error}")),
                }
            },
        );
}

/// Comment lines of `line_bytes` bytes each, ending in LF, up to `total`
/// bytes; the last line is shorter if it must be.
fn comments(total: usize, line_bytes: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(total);
    while bytes.len() < total {
        let room = total - bytes.len();
        let line = (line_bytes + 1).min(room);
        bytes.push(b'#');
        bytes.extend(std::iter::repeat_n(b'x', line.saturating_sub(2)));
        if line >= 2 {
            bytes.push(b'\n');
        }
    }
    bytes
}

#[test]
fn invariants_refuse_an_accepted_input_over_the_byte_caps() {
    let empty = parse(b"");
    let at_cap = format!("#{}", "x".repeat(MAX_LINE_BYTES - 1));
    assert_eq!(invariants::check(at_cap.as_bytes(), &empty), Ok(()));
    let crlf = format!("{at_cap}\r\n{at_cap}\r\n");
    assert_eq!(invariants::check(crlf.as_bytes(), &empty), Ok(()));
    let over = format!("{at_cap}x\n");
    assert!(invariants::check(over.as_bytes(), &empty).is_err());

    let file = comments(MAX_FILE_BYTES, 64);
    assert_eq!(file.len(), MAX_FILE_BYTES);
    assert_eq!(invariants::check(&file, &empty), Ok(()));
    let file = comments(MAX_FILE_BYTES + 1, 64);
    assert!(invariants::check(&file, &empty).is_err());
}

#[test]
fn invariants_require_file_too_large_without_a_position_exactly_over_the_file_cap() {
    let over = comments(MAX_FILE_BYTES + 1, 64);
    assert_eq!(invariants::check(&over, &parse(&over)), Ok(()));
    // Another error, or a document, for a file over the cap is refused.
    assert!(invariants::check(&over, &parse(b"=")).is_err());
    // An error without a position for a file within the cap is refused.
    assert!(invariants::check(b"", &parse(&over)).is_err());
}

#[test]
fn invariants_bound_columns_in_unicode_scalar_values() {
    let at = |line, column| Position { line, column };
    // Two characters, three bytes: the last column is 3.
    assert_eq!(
        invariants::position_within("\u{e9}a".as_bytes(), at(1, 3)),
        Ok(())
    );
    assert!(invariants::position_within("\u{e9}a".as_bytes(), at(1, 4)).is_err());
    assert!(invariants::position_within(b"a", at(1, 0)).is_err());
    assert!(invariants::position_within(b"a\n", at(3, 1)).is_err());
    // Invalid UTF-8: the valid prefix, then the position just after it.
    assert_eq!(invariants::position_within(b"a\xffbc", at(1, 2)), Ok(()));
    assert!(invariants::position_within(b"a\xffbc", at(1, 3)).is_err());
}

/// The error for `input`, as its kind and its position's line and column.
fn refused(input: &str) -> (ironafs_config::SyntaxErrorKind, usize, usize) {
    let error = parse(input.as_bytes()).unwrap_err();
    let position = error.position().unwrap();
    (error.kind(), position.line, position.column)
}

#[test]
fn parse_refuses_bidirectional_controls_in_comments_keys_and_values() {
    for control in invariants::BIDI_CONTROLS {
        let kind = ironafs_config::SyntaxErrorKind::BidiControl(control);
        assert_eq!(refused(&format!("# a{control}b\n")), (kind, 1, 4));
        assert_eq!(refused(&format!("a = 1 # {control}")), (kind, 1, 9));
        assert_eq!(refused(&format!("\"k{control}\" = 1")), (kind, 1, 3));
        assert_eq!(refused(&format!("[\"t\".\"{control}\"]")), (kind, 1, 7));
        assert_eq!(refused(&format!("a = \"x{control}y\"")), (kind, 1, 7));
        assert_eq!(
            refused(&format!("a = [\"x\", \"{control}\"]")),
            (kind, 1, 12)
        );
        assert_eq!(refused(&format!("a{control} = 1")), (kind, 1, 2));
        assert_eq!(refused(&format!("a = 1{control}")), (kind, 1, 6));
    }
}
