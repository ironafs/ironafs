// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for what a contract name may hold.

use alloc::format;
use alloc::vec;

use super::{MAX_NAME_BYTES, Name, NameError};

#[test]
fn name_keeps_its_bytes() {
    assert_eq!(Name::new(b"file.txt").unwrap().as_bytes(), b"file.txt");
}

#[test]
fn name_refuses_empty() {
    assert_eq!(Name::new(b"").unwrap_err(), NameError::Empty);
}

#[test]
fn name_refuses_dot_and_dot_dot() {
    assert_eq!(Name::new(b".").unwrap_err(), NameError::Dot);
    assert_eq!(Name::new(b"..").unwrap_err(), NameError::DotDot);
}

#[test]
fn name_accepts_other_dot_names() {
    for bytes in [&b"..."[..], b".a", b"a.", b".hidden"] {
        assert_eq!(Name::new(bytes).unwrap().as_bytes(), bytes);
    }
}

#[test]
fn name_refuses_a_slash_anywhere() {
    for bytes in [&b"/"[..], b"a/b", b"/a", b"a/"] {
        assert_eq!(Name::new(bytes).unwrap_err(), NameError::Slash, "{bytes:?}");
    }
}

#[test]
fn name_refuses_a_nul_anywhere() {
    for bytes in [&b"\0"[..], b"a\0b", b"\0a", b"a\0"] {
        assert_eq!(Name::new(bytes).unwrap_err(), NameError::Nul, "{bytes:?}");
    }
}

#[test]
fn name_at_the_cap_is_accepted_and_one_more_byte_is_refused() {
    assert_eq!(
        Name::new(&vec![b'a'; MAX_NAME_BYTES])
            .unwrap()
            .as_bytes()
            .len(),
        MAX_NAME_BYTES
    );
    assert_eq!(
        Name::new(&vec![b'a'; MAX_NAME_BYTES + 1]).unwrap_err(),
        NameError::TooLong
    );
}

#[test]
fn name_keeps_bytes_that_are_not_utf8() {
    assert_eq!(Name::new(b"\xff\xfe").unwrap().as_bytes(), b"\xff\xfe");
}

#[test]
fn name_never_normalises_unicode() {
    let composed = Name::new("\u{e9}".as_bytes()).unwrap();
    let decomposed = Name::new("e\u{301}".as_bytes()).unwrap();
    assert_ne!(composed, decomposed);
    assert_eq!(composed.as_bytes(), "\u{e9}".as_bytes());
    assert_eq!(decomposed.as_bytes(), "e\u{301}".as_bytes());
}

#[test]
fn name_debug_escapes_its_bytes() {
    let name = Name::new(b"a\xff\n").unwrap();
    assert_eq!(format!("{name:?}"), "Name(\"a\\xff\\n\")");
}
