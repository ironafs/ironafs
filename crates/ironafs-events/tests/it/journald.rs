// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The journald encoding: golden vectors written by hand from
//! `docs/specs/events.md` §5.8, and properties over arbitrary events.

use ironafs_events::journald::JOURNALD_MAX_DATAGRAM;
use ironafs_events::{EncodeError, Event, Policy, Reference, Role, ServerText, catalogue};
use ironafs_test_support::{Property, Source};

use crate::support::{arbitrary_event, field, is_journald_name, journald_encoder, parse_journald};

fn volume_offline() -> Event {
    Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 536_870_915_u64)
        .unwrap()
        .with("message", ServerText::from_wire(b"down\n\xc3\xa9"))
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn volume_offline_under_the_routine_policy_matches_its_vector() {
    let encoded = journald_encoder(Policy::Routine)
        .encode(&volume_offline())
        .unwrap();
    let expected: &[u8] = b"MESSAGE=volume [omitted] offline: down\\x0a\\xc3\\xa9\n\
MESSAGE_ID=771546f4bcf0d0b7054f7530000100c8\n\
PRIORITY=4\n\
SYSLOG_IDENTIFIER=ironafsd\n\
IRONAFS_EVENT=200\n\
IRONAFS_EVENT_VERSION=1\n\
IRONAFS_EVENT_NAME=volume.offline\n\
IRONAFS_STREAM=diagnostic\n\
IRONAFS_SUBSYSTEM=volume\n\
IRONAFS_AUDIENCE=operational\n\
IRONAFS_F_MESSAGE=down\\x0a\\xc3\\xa9\n";
    assert_eq!(
        String::from_utf8_lossy(encoded.bytes()),
        String::from_utf8_lossy(expected)
    );
    assert!(!encoded.truncated());
}

#[test]
fn volume_offline_revealing_identifiers_matches_its_vector() {
    let encoded = journald_encoder(Policy::RevealIdentifiers)
        .encode(&volume_offline())
        .unwrap();
    let expected: &[u8] = b"MESSAGE=volume 536870915 offline: down\\x0a\\xc3\\xa9\n\
MESSAGE_ID=771546f4bcf0d0b7054f7530000100c8\n\
PRIORITY=4\n\
SYSLOG_IDENTIFIER=ironafsd\n\
IRONAFS_EVENT=200\n\
IRONAFS_EVENT_VERSION=1\n\
IRONAFS_EVENT_NAME=volume.offline\n\
IRONAFS_STREAM=diagnostic\n\
IRONAFS_SUBSYSTEM=volume\n\
IRONAFS_AUDIENCE=operational\n\
IRONAFS_F_VOLUME=536870915\n\
IRONAFS_F_MESSAGE=down\\x0a\\xc3\\xa9\n";
    assert_eq!(
        String::from_utf8_lossy(encoded.bytes()),
        String::from_utf8_lossy(expected)
    );
}

#[test]
fn a_direct_server_reachable_matches_its_vector() {
    let event = Event::builder(&catalogue::SERVER_REACHABLE, Role::Daemon)
        .unwrap()
        .direct()
        .unwrap()
        .with("server", Reference::from_bytes([0x1f; 32]))
        .unwrap()
        .build()
        .unwrap();
    let hex = "1f".repeat(32);
    let expected = format!(
        "MESSAGE=server {hex} reachable\n\
MESSAGE_ID=771546f4bcf0d0b7054f753000010065\n\
PRIORITY=5\n\
SYSLOG_IDENTIFIER=ironafsd\n\
IRONAFS_EVENT=101\n\
IRONAFS_EVENT_VERSION=1\n\
IRONAFS_EVENT_NAME=server.reachable\n\
IRONAFS_STREAM=diagnostic\n\
IRONAFS_SUBSYSTEM=server\n\
IRONAFS_AUDIENCE=admin\n\
IRONAFS_DIRECT=1\n\
IRONAFS_F_SERVER={hex}\n"
    );
    let encoded = journald_encoder(Policy::Routine).encode(&event).unwrap();
    assert_eq!(String::from_utf8_lossy(encoded.bytes()), expected);
}

#[test]
fn an_audit_gap_carries_the_audit_stream_and_its_own_message_id() {
    let event = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon)
        .unwrap()
        .with("first", 10_u64)
        .unwrap()
        .with("last", 12_u64)
        .unwrap()
        .with("kind", "known-loss")
        .unwrap()
        .build()
        .unwrap();
    let encoded = journald_encoder(Policy::Routine).encode(&event).unwrap();
    let fields = parse_journald(encoded.bytes()).unwrap();
    assert_eq!(
        field(&fields, "MESSAGE"),
        Some(&b"audit events 10 to 12: known-loss"[..])
    );
    assert_eq!(
        field(&fields, "MESSAGE_ID"),
        Some(&b"771546f4bcf0d0b7054f753000020001"[..])
    );
    assert_eq!(field(&fields, "IRONAFS_STREAM"), Some(&b"audit"[..]));
}

#[test]
fn every_encoding_parses_names_well_and_stays_within_its_cap() {
    Property::new("journald encodings are well formed and within the cap")
        .cases(4_096)
        .check(
            |source: &mut Source| (arbitrary_event(source), source.usize_in(0..=1_000)),
            |(event, permille)| {
                // The cap is a share of the event's whole encoding, so cuts,
                // drops and refusals all occur.
                let whole_len = journald_encoder(Policy::Routine)
                    .encode(event)
                    .unwrap()
                    .bytes()
                    .len();
                let cap = &(whole_len * permille / 1_000);
                let sink = journald_encoder(Policy::Routine).with_cap(*cap);
                match sink.encode(event) {
                    Ok(encoded) => {
                        assert!(encoded.bytes().len() <= *cap);
                        let fields = parse_journald(encoded.bytes()).unwrap();
                        for (name, value) in &fields {
                            assert!(is_journald_name(name), "{name}");
                            assert!(
                                value.iter().all(|byte| (0x20..=0x7e).contains(byte)),
                                "{name} is not printable"
                            );
                        }
                        let def = event.def();
                        assert_eq!(
                            field(&fields, "IRONAFS_EVENT"),
                            Some(def.id.to_string().as_bytes())
                        );
                        assert_eq!(
                            field(&fields, "IRONAFS_EVENT_NAME"),
                            Some(def.name.as_bytes())
                        );
                        assert_eq!(
                            field(&fields, "IRONAFS_TRUNCATED").is_some(),
                            encoded.truncated()
                        );
                        // A field the whole encoding writes and this one
                        // leaves out was dropped: the message must say so.
                        let whole = journald_encoder(Policy::Routine).encode(event).unwrap();
                        let whole = parse_journald(whole.bytes()).unwrap();
                        let dropped = whole.iter().any(|(name, _)| {
                            name.starts_with("IRONAFS_F_") && field(&fields, name).is_none()
                        });
                        assert!(
                            !dropped || encoded.truncated(),
                            "a field was dropped unmarked"
                        );
                        for spec in def.fields.iter().filter(|spec| spec.required) {
                            let name = format!("IRONAFS_F_{}", spec.name.to_ascii_uppercase());
                            let omitted = spec.class == ironafs_events::DataClass::Identifier
                                && def.stream == catalogue::Stream::Diagnostic;
                            assert_eq!(field(&fields, &name).is_some(), !omitted, "{name}");
                        }
                    }
                    Err(EncodeError::OverSizeLimit { stream, id }) => {
                        assert_eq!((stream, id), (event.def().stream, event.def().id));
                        assert!(*cap < JOURNALD_MAX_DATAGRAM);
                    }
                }
            },
        );
}

#[test]
fn every_event_fits_the_full_journald_cap() {
    Property::new("every event fits 16 KiB").check(arbitrary_event, |event| {
        let encoded = journald_encoder(Policy::RevealIdentifiers)
            .encode(event)
            .unwrap();
        assert!(encoded.bytes().len() <= JOURNALD_MAX_DATAGRAM);
    });
}
