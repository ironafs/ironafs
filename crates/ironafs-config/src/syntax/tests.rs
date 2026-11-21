// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;

use super::*;

#[track_caller]
fn accepted(input: &str) -> Document {
    parse(input.as_bytes()).unwrap()
}

/// The error for `input`, as its kind and its position's line and column.
#[track_caller]
fn refused(input: &[u8]) -> (SyntaxErrorKind, usize, usize) {
    let error = parse(input).unwrap_err();
    let position = error.position().unwrap();
    (error.kind(), position.line, position.column)
}

#[track_caller]
fn refused_str(input: &str) -> (SyntaxErrorKind, usize, usize) {
    refused(input.as_bytes())
}

fn unsupported(feature: Feature, line: usize, column: usize) -> (SyntaxErrorKind, usize, usize) {
    (SyntaxErrorKind::Unsupported(feature), line, column)
}

fn unexpected(
    found: Option<char>,
    expected: Expected,
    line: usize,
    column: usize,
) -> (SyntaxErrorKind, usize, usize) {
    (
        SyntaxErrorKind::Unexpected { found, expected },
        line,
        column,
    )
}

fn at(line: usize, column: usize) -> Position {
    Position { line, column }
}

/// Every entry as its table path, key and value.
fn entries(document: &Document) -> Vec<(Vec<String>, String, Value)> {
    document
        .entries()
        .map(|(path, entry)| {
            (
                path.to_vec(),
                entry.key().to_string(),
                entry.value().clone(),
            )
        })
        .collect()
}

fn string(text: &str) -> Value {
    Value::Scalar(Scalar::String(text.to_string()))
}

fn integer(value: i64) -> Value {
    Value::Scalar(Scalar::Integer(value))
}

fn boolean(value: bool) -> Value {
    Value::Scalar(Scalar::Boolean(value))
}

fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}

/// The single value of `key = <value>`.
#[track_caller]
fn value_of(value_text: &str) -> Value {
    let document = accepted(&format!("key = {value_text}\n"));
    let found = entries(&document);
    assert_eq!(found.len(), 1);
    found.into_iter().next().unwrap().2
}

#[test]
fn parse_empty_file_gives_empty_document() {
    let document = accepted("");
    assert_eq!(document.entries().count(), 0);
    assert_eq!(document.tables().count(), 0);
}

#[test]
fn parse_scalars_in_root_table() {
    let document = accepted("a = \"x\"\nb = 42\nc = -7\nd = true\ne = false\n");
    assert_eq!(
        entries(&document),
        vec![
            (vec![], "a".to_string(), string("x")),
            (vec![], "b".to_string(), integer(42)),
            (vec![], "c".to_string(), integer(-7)),
            (vec![], "d".to_string(), boolean(true)),
            (vec![], "e".to_string(), boolean(false)),
        ]
    );
}

#[test]
fn parse_records_key_and_value_positions() {
    let document = accepted("\n  key\t=  1\n");
    let (_, entry) = document.entries().next().unwrap();
    assert_eq!(entry.key_position(), at(2, 3));
    assert_eq!(entry.value_position(), at(2, 10));
}

#[test]
fn parse_ignores_comments_and_blank_lines() {
    let document = accepted("# heading\n\n   \n a = 1 # trailing\n\t# indented\n");
    assert_eq!(
        entries(&document),
        vec![(vec![], "a".to_string(), integer(1))]
    );
}

#[test]
fn parse_crlf_line_ends_match_lf() {
    assert_eq!(
        accepted("a = 1\r\n[t]\r\nb = \"x\"\r\n"),
        accepted("a = 1\n[t]\nb = \"x\"\n")
    );
}

#[test]
fn parse_accepts_last_line_without_line_end() {
    assert_eq!(
        entries(&accepted("a = 1")),
        vec![(vec![], "a".to_string(), integer(1))]
    );
    assert_eq!(
        entries(&accepted("a = 1 # c")),
        vec![(vec![], "a".to_string(), integer(1))]
    );
}

#[test]
fn parse_bare_keys_take_letters_digits_underscore_and_hyphen() {
    let document = accepted("Az09_- = 1\n1234 = 2\ntrue = 3\n");
    let keys: Vec<String> = entries(&document)
        .into_iter()
        .map(|entry| entry.1)
        .collect();
    assert_eq!(keys, vec!["Az09_-", "1234", "true"]);
}

#[test]
fn parse_quoted_key_applies_escapes() {
    let document = accepted("\"a.b \\\"c\\\\\" = 1\n");
    assert_eq!(entries(&document)[0].1, "a.b \"c\\");
}

#[test]
fn parse_integers_in_decimal() {
    assert_eq!(value_of("0"), integer(0));
    assert_eq!(value_of("-0"), integer(0));
    assert_eq!(value_of("1_000_000"), integer(1_000_000));
    assert_eq!(value_of("-1_2"), integer(-12));
    assert_eq!(value_of("9223372036854775807"), integer(i64::MAX));
    assert_eq!(value_of("-9223372036854775808"), integer(i64::MIN));
}

#[test]
fn parse_refuses_integers_out_of_range() {
    assert_eq!(
        refused_str("a = 9223372036854775808"),
        (SyntaxErrorKind::IntegerOutOfRange, 1, 5)
    );
    assert_eq!(
        refused_str("a = -9223372036854775809"),
        (SyntaxErrorKind::IntegerOutOfRange, 1, 5)
    );
}

#[test]
fn parse_refuses_leading_zero_and_misplaced_underscore() {
    assert_eq!(refused_str("a = 01"), (SyntaxErrorKind::LeadingZero, 1, 5));
    assert_eq!(
        refused_str("a = -0_1"),
        (SyntaxErrorKind::LeadingZero, 1, 5)
    );
    assert_eq!(
        refused_str("a = 1__0"),
        (SyntaxErrorKind::MisplacedUnderscore, 1, 5)
    );
    assert_eq!(
        refused_str("a = _1"),
        (SyntaxErrorKind::MisplacedUnderscore, 1, 5)
    );
    assert_eq!(
        refused_str("a = 1_"),
        (SyntaxErrorKind::MisplacedUnderscore, 1, 5)
    );
    assert_eq!(
        refused_str("a = -_1"),
        (SyntaxErrorKind::MisplacedUnderscore, 1, 5)
    );
}

#[test]
fn parse_refuses_words_that_are_not_values() {
    assert_eq!(
        refused_str("a = yes"),
        (SyntaxErrorKind::InvalidValue, 1, 5)
    );
    assert_eq!(
        refused_str("a = True"),
        (SyntaxErrorKind::InvalidValue, 1, 5)
    );
    assert_eq!(
        refused_str("a = 12ab"),
        (SyntaxErrorKind::InvalidValue, 1, 5)
    );
    assert_eq!(refused_str("a = -"), (SyntaxErrorKind::InvalidValue, 1, 5));
}

#[test]
fn parse_strings_apply_the_two_escapes() {
    assert_eq!(value_of("\"\""), string(""));
    assert_eq!(value_of("\"a\\\"b\\\\c\""), string("a\"b\\c"));
    assert_eq!(
        value_of("\"caf\u{e9} # not a comment\""),
        string("caf\u{e9} # not a comment")
    );
}

#[test]
fn parse_counts_columns_in_characters() {
    assert_eq!(
        refused_str("a = \"\u{e9}\" x"),
        unexpected(Some('x'), Expected::EndOfLine, 1, 9)
    );
}

#[test]
fn parse_refuses_byte_order_mark() {
    assert_eq!(
        refused(b"\xEF\xBB\xBFa = 1\n"),
        unsupported(Feature::ByteOrderMark, 1, 1)
    );
}

#[test]
fn parse_refuses_invalid_utf8_at_its_position() {
    assert_eq!(
        refused(b"a = 1\nb = \"\xC3\xA9\xFF\"\n"),
        (SyntaxErrorKind::InvalidUtf8, 2, 7)
    );
}

#[test]
fn parse_refuses_carriage_return_without_line_feed() {
    assert_eq!(
        refused_str("a = 1\rb = 2"),
        (SyntaxErrorKind::BareCarriageReturn, 1, 6)
    );
    assert_eq!(
        refused_str("a = 1\r"),
        (SyntaxErrorKind::BareCarriageReturn, 1, 6)
    );
}

#[test]
fn parse_refuses_control_characters_everywhere() {
    assert_eq!(
        refused_str("a = 1 # \u{1}"),
        (SyntaxErrorKind::ControlCharacter('\u{1}'), 1, 9)
    );
    assert_eq!(
        refused_str("# \u{85}\n"),
        (SyntaxErrorKind::ControlCharacter('\u{85}'), 1, 3)
    );
    assert_eq!(
        refused_str("a = \"x\u{7f}\""),
        (SyntaxErrorKind::ControlCharacter('\u{7f}'), 1, 7)
    );
    assert_eq!(
        refused_str("\u{0}a = 1"),
        (SyntaxErrorKind::ControlCharacter('\u{0}'), 1, 1)
    );
    assert_eq!(
        refused_str("a = 1\u{c}"),
        (SyntaxErrorKind::ControlCharacter('\u{c}'), 1, 6)
    );
}

#[test]
fn parse_refuses_tab_inside_string() {
    assert_eq!(
        refused_str("a = \"\tx\""),
        (SyntaxErrorKind::ControlCharacter('\t'), 1, 6)
    );
    assert_eq!(
        refused_str("\"\t\" = 1"),
        (SyntaxErrorKind::ControlCharacter('\t'), 1, 2)
    );
}

#[test]
fn parse_refuses_literal_strings() {
    assert_eq!(
        refused_str("a = 'x'"),
        unsupported(Feature::LiteralString, 1, 5)
    );
    assert_eq!(
        refused_str("a = '''x'''"),
        unsupported(Feature::LiteralString, 1, 5)
    );
    assert_eq!(
        refused_str("'a' = 1"),
        unsupported(Feature::LiteralString, 1, 1)
    );
    assert_eq!(
        refused_str("[a.'b']"),
        unsupported(Feature::LiteralString, 1, 4)
    );
    assert_eq!(
        refused_str("a = ['x']"),
        unsupported(Feature::LiteralString, 1, 6)
    );
}

#[test]
fn parse_refuses_multi_line_strings() {
    assert_eq!(
        refused_str("a = \"\"\"x\"\"\""),
        unsupported(Feature::MultiLineString, 1, 5)
    );
    assert_eq!(
        refused_str("\"\"\"a\"\"\" = 1"),
        unsupported(Feature::MultiLineString, 1, 1)
    );
}

#[test]
fn parse_refuses_inline_tables() {
    assert_eq!(
        refused_str("a = { b = 1 }"),
        unsupported(Feature::InlineTable, 1, 5)
    );
    assert_eq!(
        refused_str("a = [{}]"),
        unsupported(Feature::InlineTable, 1, 6)
    );
}

#[test]
fn parse_refuses_arrays_of_tables() {
    assert_eq!(
        refused_str("[[a]]"),
        unsupported(Feature::ArrayOfTables, 1, 1)
    );
    assert_eq!(
        refused_str("  [[a]]"),
        unsupported(Feature::ArrayOfTables, 1, 3)
    );
}

#[test]
fn parse_refuses_dotted_keys() {
    assert_eq!(
        refused_str("a.b = 1"),
        unsupported(Feature::DottedKey, 1, 2)
    );
    assert_eq!(
        refused_str("a . b = 1"),
        unsupported(Feature::DottedKey, 1, 3)
    );
    assert_eq!(
        refused_str("\"a\".b = 1"),
        unsupported(Feature::DottedKey, 1, 4)
    );
}

#[test]
fn parse_refuses_floats() {
    for value in [
        "1.5", "1e3", "1E3", "-2.0", "inf", "nan", "-inf", "-nan", "3.0e-2",
    ] {
        assert_eq!(
            refused_str(&format!("a = {value}")),
            unsupported(Feature::Float, 1, 5),
            "{value}"
        );
    }
}

#[test]
fn parse_refuses_dates_and_times() {
    for value in [
        "1979-05-27",
        "07:32:00",
        "1979-05-27T07:32:00Z",
        "1979-05-27 07:32:00",
        "1979-05-27T00:32:00.999999-07:00",
    ] {
        assert_eq!(
            refused_str(&format!("a = {value}")),
            unsupported(Feature::DateTime, 1, 5),
            "{value}"
        );
    }
}

#[test]
fn parse_refuses_non_decimal_integers() {
    for value in ["0x1F", "0o17", "0b101", "-0x1"] {
        assert_eq!(
            refused_str(&format!("a = {value}")),
            unsupported(Feature::NonDecimalInteger, 1, 5),
            "{value}"
        );
    }
}

#[test]
fn parse_refuses_plus_sign() {
    assert_eq!(refused_str("a = +1"), unsupported(Feature::PlusSign, 1, 5));
    assert_eq!(
        refused_str("a = +inf"),
        unsupported(Feature::PlusSign, 1, 5)
    );
}

#[test]
fn parse_refuses_other_escapes_at_the_backslash() {
    assert_eq!(
        refused_str("a = \"x\\n\""),
        unsupported(Feature::Escape('n'), 1, 7)
    );
    assert_eq!(
        refused_str("a = \"\\u0041\""),
        unsupported(Feature::Escape('u'), 1, 6)
    );
    assert_eq!(
        refused_str("\"\\t\" = 1"),
        unsupported(Feature::Escape('t'), 1, 2)
    );
}

#[test]
fn parse_refuses_empty_key() {
    assert_eq!(
        refused_str("\"\" = 1"),
        unsupported(Feature::EmptyKey, 1, 1)
    );
    assert_eq!(
        refused_str("[a.\"\"]"),
        unsupported(Feature::EmptyKey, 1, 4)
    );
}

#[test]
fn parse_refuses_text_after_a_value_or_header() {
    assert_eq!(
        refused_str("a = 1 2"),
        unexpected(Some('2'), Expected::EndOfLine, 1, 7)
    );
    assert_eq!(
        refused_str("a = \"x\" y"),
        unexpected(Some('y'), Expected::EndOfLine, 1, 9)
    );
    assert_eq!(
        refused_str("a = \"x\"\"y\""),
        unexpected(Some('"'), Expected::EndOfLine, 1, 8)
    );
    assert_eq!(
        refused_str("a = 1,"),
        unexpected(Some(','), Expected::EndOfLine, 1, 6)
    );
    assert_eq!(
        refused_str("[a] b = 1"),
        unexpected(Some('b'), Expected::EndOfLine, 1, 5)
    );
    assert_eq!(
        refused_str("a = [1] x"),
        unexpected(Some('x'), Expected::EndOfLine, 1, 9)
    );
}

#[test]
fn parse_refuses_missing_parts_of_a_pair() {
    assert_eq!(refused_str("a ="), unexpected(None, Expected::Value, 1, 4));
    assert_eq!(
        refused_str("a =\nb = 1"),
        unexpected(Some('\n'), Expected::Value, 1, 4)
    );
    assert_eq!(
        refused_str("a = # c"),
        unexpected(Some('#'), Expected::Value, 1, 5)
    );
    assert_eq!(
        refused_str("a 1"),
        unexpected(Some('1'), Expected::Equals, 1, 3)
    );
    assert_eq!(refused_str("a"), unexpected(None, Expected::Equals, 1, 2));
    assert_eq!(
        refused_str("= 1"),
        unexpected(Some('='), Expected::LineStart, 1, 1)
    );
    assert_eq!(
        refused_str("a = ="),
        unexpected(Some('='), Expected::Value, 1, 5)
    );
}

#[test]
fn parse_refuses_unterminated_strings_at_the_opening_quote() {
    assert_eq!(
        refused_str("a = \"x"),
        (SyntaxErrorKind::UnterminatedString, 1, 5)
    );
    assert_eq!(
        refused_str("a = \"x\nb = 1"),
        (SyntaxErrorKind::UnterminatedString, 1, 5)
    );
    assert_eq!(
        refused_str("a = \"x\\"),
        (SyntaxErrorKind::UnterminatedString, 1, 5)
    );
    assert_eq!(
        refused_str("\"a = 1"),
        (SyntaxErrorKind::UnterminatedString, 1, 1)
    );
}

#[test]
fn parse_refuses_malformed_headers() {
    assert_eq!(
        refused_str("[a"),
        unexpected(None, Expected::DotOrCloseBracket, 1, 3)
    );
    assert_eq!(
        refused_str("[]"),
        unexpected(Some(']'), Expected::Key, 1, 2)
    );
    assert_eq!(
        refused_str("[a.]"),
        unexpected(Some(']'), Expected::Key, 1, 4)
    );
    assert_eq!(
        refused_str("[a b]"),
        unexpected(Some('b'), Expected::DotOrCloseBracket, 1, 4)
    );
    assert_eq!(
        refused_str("[a]]"),
        unexpected(Some(']'), Expected::EndOfLine, 1, 4)
    );
}

#[test]
fn parse_headers_with_quoted_segments_and_whitespace() {
    let document = accepted("top = 1\n[ cells . \"Example.ORG.\" ]\ndiscovery = false\n");
    let tables: Vec<(Vec<String>, Position)> = document
        .tables()
        .map(|table| (table.path().to_vec(), table.position()))
        .collect();
    assert_eq!(tables, vec![(path(&["cells", "Example.ORG."]), at(2, 1))]);
    assert_eq!(
        entries(&document),
        vec![
            (vec![], "top".to_string(), integer(1)),
            (
                path(&["cells", "Example.ORG."]),
                "discovery".to_string(),
                boolean(false)
            ),
        ]
    );
}

#[test]
fn parse_same_key_in_two_tables_is_two_keys() {
    let document = accepted("[a]\nx = 1\n[b]\nx = 2\n");
    assert_eq!(
        entries(&document),
        vec![
            (path(&["a"]), "x".to_string(), integer(1)),
            (path(&["b"]), "x".to_string(), integer(2)),
        ]
    );
}

#[test]
fn parse_refuses_duplicate_key_however_spelt() {
    assert_eq!(
        refused_str("a = 1\n\"a\" = 2"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 2, 1)
    );
    assert_eq!(
        refused_str("[t]\n  k = 1\nk = \"x\""),
        (SyntaxErrorKind::Redefined { first: at(2, 3) }, 3, 1)
    );
}

#[test]
fn parse_refuses_table_defined_twice() {
    assert_eq!(
        refused_str("[a]\n[a]"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 2, 1)
    );
    assert_eq!(
        refused_str("[a]\n[a.b]\n[\"a\"]"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 3, 1)
    );
}

#[test]
fn parse_accepts_super_table_after_sub_table() {
    let document = accepted("[a.b]\nx = 1\n[a]\ny = 2\n");
    let tables: Vec<Vec<String>> = document
        .tables()
        .map(|table| table.path().to_vec())
        .collect();
    assert_eq!(tables, vec![path(&["a", "b"]), path(&["a"])]);
    assert_eq!(
        entries(&document),
        vec![
            (path(&["a", "b"]), "x".to_string(), integer(1)),
            (path(&["a"]), "y".to_string(), integer(2)),
        ]
    );
}

#[test]
fn parse_refuses_table_where_a_value_is() {
    assert_eq!(
        refused_str("a = 1\n[a]"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 2, 1)
    );
    assert_eq!(
        refused_str("a = 1\n[a.b]"),
        (SyntaxErrorKind::NotATable { first: at(1, 1) }, 2, 2)
    );
    assert_eq!(
        refused_str("[t]\nb = 1\n[ t . b . c ]"),
        (SyntaxErrorKind::NotATable { first: at(2, 1) }, 3, 7)
    );
}

#[test]
fn parse_refuses_value_where_a_table_is() {
    assert_eq!(
        refused_str("[a.b]\n[a]\nb = 1"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 3, 1)
    );
    assert_eq!(
        refused_str("[a.b.c]\n[a]\nb = 1"),
        (SyntaxErrorKind::Redefined { first: at(1, 1) }, 3, 1)
    );
}

#[test]
fn parse_arrays_across_lines_with_comments() {
    let document = accepted("a = [\n  \"x\", # first\n\n  # between\n  \"y\",\n]\nb = 1\n");
    let found = entries(&document);
    assert_eq!(
        found[0].2,
        Value::Array(vec![
            Element {
                value: Scalar::String("x".to_string()),
                position: at(2, 3)
            },
            Element {
                value: Scalar::String("y".to_string()),
                position: at(5, 3)
            },
        ])
    );
    assert_eq!(found[1], (vec![], "b".to_string(), integer(1)));
}

#[test]
fn parse_arrays_of_each_scalar_type() {
    assert_eq!(value_of("[]"), Value::Array(vec![]));
    assert_eq!(value_of("[ ]"), Value::Array(vec![]));
    assert_eq!(
        value_of("[1,-2]"),
        Value::Array(vec![
            Element {
                value: Scalar::Integer(1),
                position: at(1, 8)
            },
            Element {
                value: Scalar::Integer(-2),
                position: at(1, 10)
            },
        ])
    );
    assert_eq!(
        value_of("[true]"),
        Value::Array(vec![Element {
            value: Scalar::Boolean(true),
            position: at(1, 8)
        }])
    );
}

#[test]
fn parse_refuses_mixed_and_nested_arrays() {
    assert_eq!(
        refused_str("a = [1, \"x\"]"),
        unsupported(Feature::MixedArray, 1, 9)
    );
    assert_eq!(
        refused_str("a = [true, 1]"),
        unsupported(Feature::MixedArray, 1, 12)
    );
    assert_eq!(
        refused_str("a = [[1]]"),
        unsupported(Feature::NestedArray, 1, 6)
    );
    assert_eq!(
        refused_str("a = [1, []]"),
        unsupported(Feature::NestedArray, 1, 9)
    );
}

#[test]
fn parse_refuses_malformed_arrays() {
    assert_eq!(
        refused_str("a = [1 2]"),
        unexpected(Some('2'), Expected::CommaOrCloseBracket, 1, 8)
    );
    assert_eq!(
        refused_str("a = [,]"),
        unexpected(Some(','), Expected::ValueOrCloseBracket, 1, 6)
    );
    assert_eq!(
        refused_str("a = [1,,]"),
        unexpected(Some(','), Expected::ValueOrCloseBracket, 1, 8)
    );
    assert_eq!(
        refused_str("a = [1,"),
        unexpected(None, Expected::ValueOrCloseBracket, 1, 8)
    );
    assert_eq!(
        refused_str("a = [1"),
        unexpected(None, Expected::CommaOrCloseBracket, 1, 7)
    );
    assert_eq!(
        refused_str("a = [yes]"),
        (SyntaxErrorKind::InvalidValue, 1, 6)
    );
}

/// `lines` lines of comments, each exactly `line_bytes` long with its LF.
fn comment_lines(lines: usize, line_bytes: usize) -> String {
    let mut text = String::new();
    for _ in 0..lines {
        text.push('#');
        text.push_str(&"x".repeat(line_bytes - 2));
        text.push('\n');
    }
    text
}

#[test]
fn parse_file_cap_at_and_above() {
    let at_cap = comment_lines(MAX_FILE_BYTES / 4_096, 4_096);
    assert_eq!(at_cap.len(), MAX_FILE_BYTES);
    assert_eq!(accepted(&at_cap).entries().count(), 0);
    let above = format!("{at_cap}#");
    let error = parse(above.as_bytes()).unwrap_err();
    assert_eq!(error.kind(), SyntaxErrorKind::FileTooLarge);
    assert_eq!(error.position(), None);
}

#[test]
fn parse_line_cap_at_and_above() {
    let at_cap = format!("#{}", "x".repeat(MAX_LINE_BYTES - 1));
    accepted(&at_cap);
    accepted(&format!("{at_cap}\r\n"));
    assert_eq!(
        refused_str(&format!("a = 1\n{at_cap}x\n")),
        (SyntaxErrorKind::LineTooLong, 2, 1)
    );
    let multibyte = format!("#{}", "\u{e9}".repeat(MAX_LINE_BYTES / 2));
    assert_eq!(
        refused_str(&multibyte),
        (SyntaxErrorKind::LineTooLong, 1, 1)
    );
}

#[test]
fn parse_key_cap_at_and_above() {
    let at_cap = "k".repeat(MAX_KEY_BYTES);
    accepted(&format!("{at_cap} = 1"));
    accepted(&format!("\"{at_cap}\" = 1"));
    accepted(&format!("[\"{at_cap}\"]"));
    assert_eq!(
        refused_str(&format!("{at_cap}k = 1")),
        (SyntaxErrorKind::KeyTooLong, 1, 1)
    );
    assert_eq!(
        refused_str(&format!("\"{at_cap}k\" = 1")),
        (SyntaxErrorKind::KeyTooLong, 1, 1)
    );
    assert_eq!(
        refused_str(&format!("[a.{at_cap}k]")),
        (SyntaxErrorKind::KeyTooLong, 1, 4)
    );
    // The cap counts bytes after escapes: 255 escaped backslashes fit.
    accepted(&format!("\"{}\" = 1", "\\\\".repeat(MAX_KEY_BYTES)));
}

#[test]
fn parse_string_cap_at_and_above() {
    let at_cap = "s".repeat(MAX_STRING_BYTES);
    assert_eq!(value_of(&format!("\"{at_cap}\"")), string(&at_cap));
    assert_eq!(
        refused_str(&format!("a = \"{at_cap}s\"")),
        (SyntaxErrorKind::StringTooLong, 1, 5)
    );
    assert_eq!(
        refused_str(&format!("a = [\"{at_cap}s\"]")),
        (SyntaxErrorKind::StringTooLong, 1, 6)
    );
}

#[test]
fn parse_depth_cap_at_and_above() {
    let document = accepted("[a.b.c.d.e.f.g.h]\n");
    assert_eq!(
        document.tables().next().unwrap().path(),
        path(&["a", "b", "c", "d", "e", "f", "g", "h"]).as_slice()
    );
    assert_eq!(
        refused_str("[a.b.c.d.e.f.g.h.i]"),
        (SyntaxErrorKind::TooDeep, 1, 18)
    );
}

#[test]
fn parse_array_cap_at_and_above() {
    let at_cap = vec!["1"; MAX_ARRAY_ELEMENTS].join(",");
    match value_of(&format!("[{at_cap}]")) {
        Value::Array(elements) => assert_eq!(elements.len(), MAX_ARRAY_ELEMENTS),
        Value::Scalar(other) => panic!("not an array: {other:?}"),
    }
    let column = 6 + at_cap.len() + 1;
    assert_eq!(
        refused_str(&format!("a = [{at_cap},2]")),
        (SyntaxErrorKind::TooManyElements, 1, column)
    );
}

#[test]
fn parse_definitions_cap_at_and_above() {
    let mut text = String::new();
    for index in 0..MAX_DEFINITIONS {
        writeln!(text, "k{index} = 1").unwrap();
    }
    assert_eq!(accepted(&text).entries().count(), MAX_DEFINITIONS);
    let over = format!("{text}last = 1\n");
    assert_eq!(
        refused_str(&over),
        (SyntaxErrorKind::TooManyDefinitions, MAX_DEFINITIONS + 1, 1)
    );
}

#[test]
fn parse_definitions_cap_counts_tables() {
    // Two tables per header (an implicit one and an explicit one), then the
    // remaining definitions as keys.
    let mut text = String::new();
    for index in 0..10 {
        writeln!(text, "[t{index}.u]").unwrap();
    }
    text.push_str("[keys]\n");
    for index in 0..(MAX_DEFINITIONS - 21) {
        writeln!(text, "k{index} = 1").unwrap();
    }
    accepted(&text);
    let over = format!("{text}last = 1\n");
    let lines = 11 + (MAX_DEFINITIONS - 21) + 1;
    assert_eq!(
        refused_str(&over),
        (SyntaxErrorKind::TooManyDefinitions, lines, 1)
    );
}

#[test]
fn parse_definitions_cap_refuses_table_over_it() {
    let mut text = String::new();
    for index in 0..(MAX_DEFINITIONS - 1) {
        writeln!(text, "k{index} = 1").unwrap();
    }
    // One definition left: `[a]` fits, `[b.c]` needs two.
    accepted(&format!("{text}[a]\n"));
    assert_eq!(
        refused_str(&format!("{text}[b.c]\n")),
        (SyntaxErrorKind::TooManyDefinitions, MAX_DEFINITIONS, 4)
    );
}

#[test]
fn error_message_names_position_and_hint() {
    let error = parse(b"a = 1.5").unwrap_err();
    assert_eq!(
        error.to_string(),
        "line 1, column 5: floats are not supported; write a whole number"
    );
    let error = parse(b"a = 1\na = 2").unwrap_err();
    assert_eq!(
        error.to_string(),
        "line 2, column 1: already defined at line 1, column 1"
    );
    let error = parse(b"a 1").unwrap_err();
    assert_eq!(
        error.to_string(),
        "line 1, column 3: found '1', expected '='"
    );
    let error = parse(&vec![b'#'; MAX_FILE_BYTES + 1]).unwrap_err();
    assert_eq!(error.to_string(), "file is larger than 262144 bytes");
}
