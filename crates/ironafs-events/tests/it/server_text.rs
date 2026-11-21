// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Server-supplied text reaches an encoding only escaped
//! (`docs/specs/events.md` §5.3; ADR 0004, O5, point 3).

use ironafs_events::{Event, Policy, Role, SERVER_TEXT_MAX, ServerText, catalogue};
use ironafs_test_support::{Property, Source};

use crate::support::{
    escape, field, journald_encoder, parse_journald, parse_journald_forms, syslog_encoder,
};

fn volume_offline(bytes: &[u8]) -> Event {
    Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 1_u64)
        .unwrap()
        .with("message", ServerText::from_wire(bytes))
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn a_newline_in_server_text_cannot_add_a_journal_field() {
    let event = volume_offline(b"x\nPRIORITY=0\n_PID=1\nMESSAGE_ID=0");
    let encoded = journald_encoder(Policy::Routine).encode(&event).unwrap();
    // Escaped text holds no newline, so every field takes the simple form:
    // the length-prefixed form would frame a raw newline safely and hide a
    // missing escape.
    for (name, _, length_prefixed) in parse_journald_forms(encoded.bytes()).unwrap() {
        assert!(!length_prefixed, "{name} took the length-prefixed form");
    }
    let fields = parse_journald(encoded.bytes()).unwrap();
    assert_eq!(field(&fields, "PRIORITY"), Some(&b"4"[..]));
    assert_eq!(field(&fields, "_PID"), None);
    assert_eq!(
        field(&fields, "MESSAGE_ID"),
        Some(&b"771546f4bcf0d0b7054f7530000100c8"[..])
    );
    assert_eq!(
        field(&fields, "IRONAFS_F_MESSAGE"),
        Some(&b"x\\x0aPRIORITY=0\\x0a_PID=1\\x0aMESSAGE_ID=0"[..])
    );
}

#[test]
fn server_text_never_reaches_the_output_raw() {
    Property::new("server text is written only escaped").check(
        |source: &mut Source| source.bytes(0..=SERVER_TEXT_MAX),
        |bytes| {
            let event = volume_offline(bytes);
            let expected = escape(bytes);

            let journald = journald_encoder(Policy::Routine).encode(&event).unwrap();
            let fields = parse_journald(journald.bytes()).unwrap();
            assert_eq!(
                field(&fields, "IRONAFS_F_MESSAGE"),
                Some(expected.as_bytes())
            );
            for (name, value) in &fields {
                assert!(
                    value.iter().all(|byte| (0x20..=0x7e).contains(byte)),
                    "{name}"
                );
            }

            // The syslog cap may cut the text: what is written is a prefix
            // of the escaped form, then the marker.
            let syslog = syslog_encoder(Policy::Routine).encode(&event).unwrap();
            assert!(
                syslog
                    .bytes()
                    .iter()
                    .all(|byte| (0x20..=0x7e).contains(byte))
            );
            let text = String::from_utf8(syslog.bytes().to_vec()).unwrap();
            let after = text.split_once(" f_message=\"").unwrap().1;
            let written = after.split_once("\" msg=\"").unwrap().0;
            let kept = written
                .strip_suffix(ironafs_events::TRUNCATION_MARKER)
                .unwrap_or(written);
            assert!(
                expected.starts_with(kept),
                "{written:?} is not a prefix of {expected:?}"
            );
            let mut whole_escapes = (0..=bytes.len()).map(|count| escape(&bytes[..count]));
            assert!(
                whole_escapes.any(|prefix| prefix == kept),
                "{written:?} splits an escape"
            );
        },
    );
}
