// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the shared JSON reader.

use super::{MAX_DEPTH, ParseError, Value, parse};

#[test]
fn parse_reads_nested_objects_arrays_and_scalars() {
    let value = parse(r#" {"a": [1, -2.5e3, true, false, null], "b": {"c": "d"}} "#).unwrap();

    assert_eq!(
        value,
        Value::Object(vec![
            (
                "a".into(),
                Value::Array(vec![
                    Value::Number("1".into()),
                    Value::Number("-2.5e3".into()),
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Null,
                ])
            ),
            (
                "b".into(),
                Value::Object(vec![("c".into(), Value::String("d".into()))])
            ),
        ])
    );
}

#[test]
fn parse_decodes_escapes_and_surrogate_pairs() {
    let value = parse(r#""q\"b\\s\/n\nt\tu\u00e9p\ud83d\ude00""#).unwrap();

    assert_eq!(
        value,
        Value::String("q\"b\\s/n\nt\tu\u{e9}p\u{1f600}".into())
    );
}

#[test]
fn parse_rejects_malformed_input_with_its_offset() {
    for (text, offset, reason) in [
        ("", 0, "expected a value"),
        ("{", 1, "expected a string"),
        ("[1,]", 3, "expected a value"),
        ("[1 2]", 3, "expected ',' or ']'"),
        ("{\"a\" 1}", 5, "expected ':'"),
        ("{\"a\": 1", 7, "expected ',' or '}'"),
        ("\"abc", 4, "unterminated string"),
        ("\"\\x\"", 2, "invalid escape"),
        ("\"\\ud800\"", 7, "unpaired surrogate"),
        ("\"a\nb\"", 2, "control character in string"),
        ("tru", 0, "expected a value"),
        ("1 2", 2, "trailing characters"),
        ("-", 1, "expected a digit"),
    ] {
        assert_eq!(
            parse(text),
            Err(ParseError { offset, reason }),
            "input {text:?}"
        );
    }
}

#[test]
fn parse_rejects_nesting_beyond_the_depth_cap() {
    let at_cap = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    assert!(parse(&at_cap).is_ok());

    let over = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
    assert_eq!(
        parse(&over),
        Err(ParseError {
            offset: MAX_DEPTH,
            reason: "nesting too deep"
        })
    );
}
