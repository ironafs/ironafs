// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the journald field forms and identifiers
//! (`docs/specs/events.md` §5.8).

use alloc::vec::Vec;

use super::{JOURNALD_MAX_DATAGRAM, JournaldEncoder, message_id, push_field};
use crate::catalogue::Stream;
use crate::redaction::Policy;
use crate::syslog::AppName;

#[test]
fn message_id_is_the_prefix_then_the_stream_and_the_id() {
    assert_eq!(
        message_id(Stream::Diagnostic, 200),
        "771546f4bcf0d0b7054f7530000100c8"
    );
    assert_eq!(
        message_id(Stream::Audit, 0xffff),
        "771546f4bcf0d0b7054f75300002ffff"
    );
}

#[test]
fn a_value_with_no_newline_takes_the_simple_form() {
    let mut out = Vec::new();
    push_field(&mut out, "PRIORITY", b"4");
    assert_eq!(out, b"PRIORITY=4\n");
}

#[test]
fn a_value_with_a_newline_takes_the_length_prefixed_form() {
    let mut out = Vec::new();
    push_field(&mut out, "MESSAGE", b"a\nb");
    assert_eq!(out, b"MESSAGE\n\x03\x00\x00\x00\x00\x00\x00\x00a\nb\n");
}

#[test]
fn with_cap_never_raises_the_cap() {
    let encoder = JournaldEncoder::new(AppName::new("ironafsd").unwrap(), Policy::Routine);
    assert_eq!(
        encoder.clone().with_cap(usize::MAX).cap,
        JOURNALD_MAX_DATAGRAM
    );
    assert_eq!(encoder.with_cap(100).cap, 100);
}
