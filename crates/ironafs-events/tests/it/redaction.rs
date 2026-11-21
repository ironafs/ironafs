// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Redaction through the encoders (`docs/specs/events.md` §5.2, point 5,
//! and §5.4; ADR 0004, O5 and O6, point 1).

use core::fmt;

use ironafs_events::{
    Event, Policy, REDACTION_MARKER, Role, ServerText, catalogue, write_redacted,
};

use crate::support::{field, journald_encoder, parse_journald, syslog_encoder};

/// A stand-in for a secret type: its owner writes both formats with the
/// marker, as section 5.2, point 5, asks.
struct SessionKey([u8; 16]);

impl fmt::Debug for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_redacted(formatter)
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_redacted(formatter)
    }
}

#[test]
fn formatting_a_secret_prints_the_marker() {
    let key = SessionKey([0x5a; 16]);
    assert_eq!(key.0.len(), 16);
    assert_eq!(format!("{key:?}"), REDACTION_MARKER);
    assert_eq!(format!("{key}"), "[redacted]");
    assert_eq!(format!("{:?}", Some(&key)), "Some([redacted])");
}

fn volume_offline() -> Event {
    Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 1_234_u64)
        .unwrap()
        .with("message", ServerText::from_wire(b"x\ny"))
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn routine_logs_omit_identifiers_from_both_encodings() {
    let journald = journald_encoder(Policy::Routine)
        .encode(&volume_offline())
        .unwrap();
    let fields = parse_journald(journald.bytes()).unwrap();
    assert_eq!(field(&fields, "IRONAFS_F_VOLUME"), None);
    assert!(!journald.bytes().windows(4).any(|window| window == b"1234"));
    let syslog = syslog_encoder(Policy::Routine)
        .encode(&volume_offline())
        .unwrap();
    assert!(!syslog.bytes().windows(4).any(|window| window == b"1234"));
}

#[test]
fn no_policy_changes_how_server_text_is_written() {
    let mut values = Vec::new();
    for policy in [Policy::Routine, Policy::RevealIdentifiers] {
        let encoded = journald_encoder(policy).encode(&volume_offline()).unwrap();
        let fields = parse_journald(encoded.bytes()).unwrap();
        values.push(field(&fields, "IRONAFS_F_MESSAGE").unwrap().to_vec());
    }
    assert_eq!(values, [b"x\\x0ay".to_vec(), b"x\\x0ay".to_vec()]);
}

#[test]
fn audit_fields_are_written_under_the_routine_policy() {
    let event = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon)
        .unwrap()
        .with("first", 5_u64)
        .unwrap()
        .with("last", 9_u64)
        .unwrap()
        .with("kind", "uncertain")
        .unwrap()
        .build()
        .unwrap();
    let fields = parse_journald(
        journald_encoder(Policy::Routine)
            .encode(&event)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    assert_eq!(field(&fields, "IRONAFS_F_FIRST"), Some(&b"5"[..]));
    assert_eq!(field(&fields, "IRONAFS_F_LAST"), Some(&b"9"[..]));
}
