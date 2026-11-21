// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the Linux name codec: Linux shows the bytes.

use super::{decode, encode};
use crate::NameError;

#[test]
fn decode_keeps_the_bytes_linux_gives() {
    assert_eq!(
        decode(b"r\xc3\xa9sum\xc3\xa9").unwrap().as_bytes(),
        b"r\xc3\xa9sum\xc3\xa9"
    );
}

#[test]
fn decode_refuses_what_no_linux_component_holds() {
    assert_eq!(decode(b"").unwrap_err(), NameError::Empty);
    assert_eq!(decode(b"..").unwrap_err(), NameError::DotDot);
    assert_eq!(decode(b"a/b").unwrap_err(), NameError::Slash);
    assert_eq!(decode(b"a\0").unwrap_err(), NameError::Nul);
}

#[test]
fn encode_gives_the_name_bytes_unchanged() {
    let name = decode(b"\xff\x01x").unwrap();
    assert_eq!(encode(&name), b"\xff\x01x");
}
