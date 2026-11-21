// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for server text's escaping and limits (`docs/specs/events.md`
//! §5.3) and for references.

use alloc::format;
use alloc::string::String;

use super::{Reference, SERVER_TEXT_MAX, ServerText, TRUNCATION_MARKER};

fn rendered(text: &ServerText, limit: Option<usize>) -> (String, bool) {
    let mut out = String::new();
    let cut = text.render(&mut out, limit);
    (out, cut)
}

#[test]
fn escape_keeps_printable_ascii() {
    let text = ServerText::from_wire(b"volume 7 = offline!~");
    assert_eq!(
        rendered(&text, None),
        (String::from("volume 7 = offline!~"), false)
    );
}

#[test]
fn escape_backslashes_quote_and_backslash() {
    let text = ServerText::from_wire(br#"a"b\c"#);
    assert_eq!(rendered(&text, None).0, r#"a\"b\\c"#);
}

#[test]
fn escape_writes_controls_delete_and_non_ascii_as_hex() {
    let text = ServerText::from_wire(b"\n\r\x00\x1b\x7f\xc3\xa9");
    assert_eq!(rendered(&text, None).0, r"\x0a\x0d\x00\x1b\x7f\xc3\xa9");
}

#[test]
fn escaped_len_is_the_length_of_the_whole_rendering() {
    let text = ServerText::from_wire(b"a\"\n");
    assert_eq!(text.escaped_len(), 1 + 2 + 4);
    assert_eq!(rendered(&text, None).0.len(), text.escaped_len());
}

#[test]
fn from_wire_keeps_at_most_the_cap_and_marks_it() {
    let long = [b'a'; SERVER_TEXT_MAX + 1];
    let text = ServerText::from_wire(&long);
    assert!(text.was_cut());
    let (out, cut) = rendered(&text, None);
    assert!(cut);
    assert_eq!(out.len(), SERVER_TEXT_MAX + TRUNCATION_MARKER.len());
    assert!(out.ends_with(TRUNCATION_MARKER));

    let exact = ServerText::from_wire(&long[..SERVER_TEXT_MAX]);
    assert!(!exact.was_cut());
    assert_eq!(
        rendered(&exact, None),
        (
            String::from_utf8(long[..SERVER_TEXT_MAX].to_vec()).unwrap(),
            false
        )
    );
}

#[test]
fn render_with_a_limit_never_splits_an_escape() {
    let text = ServerText::from_wire(b"ab\ncd");
    // "ab" is 2 bytes; the newline's escape takes 4 more, so limits 2 to 5
    // all stop before it.
    for limit in 2..6 {
        assert_eq!(
            rendered(&text, Some(limit)),
            (format!("ab{TRUNCATION_MARKER}"), true),
            "limit {limit}"
        );
    }
    assert_eq!(
        rendered(&text, Some(6)),
        (format!(r"ab\x0a{TRUNCATION_MARKER}"), true)
    );
    assert_eq!(rendered(&text, Some(8)), (String::from(r"ab\x0acd"), false));
    assert_eq!(
        rendered(&text, Some(0)),
        (String::from(TRUNCATION_MARKER), true)
    );
}

#[test]
fn debug_shows_only_the_escaped_form() {
    let text = ServerText::from_wire(b"x\ny");
    assert_eq!(format!("{text:?}"), r#"ServerText("x\x0ay")"#);
}

#[test]
fn reference_renders_as_64_lower_case_hex_digits() {
    let mut bytes = [0_u8; 32];
    bytes[0] = 0xab;
    bytes[31] = 0x0f;
    let mut out = String::new();
    Reference::from_bytes(bytes).push_hex(&mut out);
    assert_eq!(out.len(), 64);
    assert!(out.starts_with("ab00"));
    assert!(out.ends_with("000f"));
}
