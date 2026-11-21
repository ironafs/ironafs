// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The RFC 5424 encoding: golden vectors written by hand from
//! `docs/specs/events.md` §5.9, the size bound of §5.9, point 7, and
//! properties over arbitrary events.

use ironafs_events::syslog::{
    APP_NAME_MAX, AppName, HOSTNAME_MAX, Origin, SYSLOG_FALLBACK_MAX, SyslogEncoder,
};
use ironafs_events::{EncodeError, Event, Policy, Role, ServerText, Stream, catalogue};
use ironafs_test_support::{Property, Source};

use crate::support::{arbitrary_event, parse_syslog, syslog_encoder};

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

/// The origin with the largest header section 5.9 allows.
fn largest_origin() -> Origin {
    Origin::new(
        &"h".repeat(HOSTNAME_MAX),
        AppName::new(&"a".repeat(APP_NAME_MAX)).unwrap(),
        Some(u32::MAX),
    )
}

#[test]
fn volume_offline_under_the_routine_policy_matches_its_vector() {
    let encoded = syslog_encoder(Policy::Routine)
        .encode(&volume_offline())
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(encoded.bytes()),
        "<28>1 - client1 ironafsd 4242 ironafs.diag.200 - event=volume.offline v=1 subsystem=volume \
         audience=operational f_message=\"down\\x0a\\xc3\\xa9\" \
         msg=\"volume [omitted] offline: down\\x0a\\xc3\\xa9\""
    );
}

#[test]
fn volume_offline_revealing_identifiers_matches_its_vector() {
    let encoded = syslog_encoder(Policy::RevealIdentifiers)
        .encode(&volume_offline())
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(encoded.bytes()),
        "<28>1 - client1 ironafsd 4242 ironafs.diag.200 - event=volume.offline v=1 subsystem=volume \
         audience=operational f_volume=536870915 f_message=\"down\\x0a\\xc3\\xa9\" \
         msg=\"volume 536870915 offline: down\\x0a\\xc3\\xa9\""
    );
}

#[test]
fn a_direct_audit_free_event_with_no_procid_and_a_long_host_matches_its_vector() {
    let origin = Origin::new(
        &"h".repeat(HOSTNAME_MAX + 1),
        AppName::new("afs").unwrap(),
        None,
    );
    let event = Event::builder(&catalogue::EVENTS_LOST, Role::Cli)
        .unwrap()
        .direct()
        .unwrap()
        .with("reason", "sink unavailable")
        .unwrap()
        .with("count", 1_u8)
        .unwrap()
        .build()
        .unwrap();
    let encoded = SyslogEncoder::new(origin, Policy::Routine)
        .encode(&event)
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(encoded.bytes()),
        "<28>1 - - afs - ironafs.diag.1 - event=events.lost v=1 subsystem=events \
         audience=operational direct=1 f_reason=\"sink unavailable\" f_count=1 \
         msg=\"1 events lost: sink unavailable\""
    );
}

#[test]
fn an_audit_event_uses_the_log_audit_facility() {
    let event = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon)
        .unwrap()
        .with("first", 1_u64)
        .unwrap()
        .with("last", 1_u64)
        .unwrap()
        .with("kind", "uncertain")
        .unwrap()
        .build()
        .unwrap();
    let encoded = syslog_encoder(Policy::Routine).encode(&event).unwrap();
    // Facility 13, severity 4: 13 * 8 + 4.
    assert!(
        encoded
            .bytes()
            .starts_with(b"<108>1 - client1 ironafsd 4242 ironafs.audit.1 - ")
    );
}

#[test]
fn the_largest_over_size_event_fits_the_fallback_cap() {
    let event = Event::over_size(Stream::Diagnostic, u16::MAX, Role::LoginService).unwrap();
    let encoded = SyslogEncoder::new(largest_origin(), Policy::Routine)
        .encode(&event)
        .unwrap();
    assert!(!encoded.truncated());
    assert!(
        encoded.bytes().len() <= SYSLOG_FALLBACK_MAX,
        "{} octets",
        encoded.bytes().len()
    );
    // Counted by hand from section 5.9: a 150-octet header, 100 octets of
    // pairs, and the 59-octet message with ` msg="` and its closing quote.
    assert_eq!(encoded.bytes().len(), 316);
}

#[test]
fn the_largest_one_range_gap_fits_the_fallback_cap() {
    let event = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon)
        .unwrap()
        .with("first", u64::MAX)
        .unwrap()
        .with("last", u64::MAX)
        .unwrap()
        .with("kind", "possible-loss")
        .unwrap()
        .build()
        .unwrap();
    let encoded = SyslogEncoder::new(largest_origin(), Policy::Routine)
        .encode(&event)
        .unwrap();
    assert!(!encoded.truncated());
    assert!(
        encoded.bytes().len() <= SYSLOG_FALLBACK_MAX,
        "{} octets",
        encoded.bytes().len()
    );
    // Counted by hand from section 5.9: a 152-octet header, 131 octets of
    // pairs, and the 72-octet message with ` msg="` and its closing quote.
    assert_eq!(encoded.bytes().len(), 362);
}

#[test]
fn every_encoding_is_printable_ascii_within_its_cap() {
    Property::new("syslog encodings are printable ASCII and within the cap").check(
        |source: &mut Source| {
            (
                arbitrary_event(source),
                source.usize_in(0..=SYSLOG_FALLBACK_MAX),
            )
        },
        |(event, cap)| match syslog_encoder(Policy::Routine).with_cap(*cap).encode(event) {
            Ok(encoded) => {
                let bytes = encoded.bytes();
                assert!(bytes.len() <= *cap);
                assert!(bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)));
                let def = event.def();
                let text = String::from_utf8_lossy(bytes);
                let msgid = match def.stream() {
                    Stream::Diagnostic => format!("ironafs.diag.{}", def.id()),
                    Stream::Audit => format!("ironafs.audit.{}", def.id()),
                };
                let (header, pairs) = parse_syslog(&text).unwrap();
                assert_eq!(header[5], msgid, "{text}");
                assert_eq!(header[6], "-", "{text}");
                let keys: Vec<&str> = pairs.iter().map(|(key, _)| key.as_str()).collect();
                assert_eq!(keys[..4], ["event", "v", "subsystem", "audience"], "{text}");
                assert_eq!(keys.last(), Some(&"msg"), "{text}");
                assert_eq!(pairs[0].1, def.name(), "{text}");
                assert_eq!(pairs[1].1, def.version().to_string(), "{text}");
                assert_eq!(keys.contains(&"truncated"), encoded.truncated(), "{text}");
            }
            Err(EncodeError::OverSizeLimit { stream, id }) => {
                assert_eq!((stream, id), (event.def().stream(), event.def().id()));
            }
        },
    );
}

#[test]
fn every_event_fits_the_fallback_cap_with_the_usual_origin() {
    Property::new("every event fits 480 octets").check(arbitrary_event, |event| {
        let encoded = syslog_encoder(Policy::RevealIdentifiers)
            .encode(event)
            .unwrap();
        assert!(encoded.bytes().len() <= SYSLOG_FALLBACK_MAX);
    });
}

#[test]
fn server_text_cannot_imitate_the_key_value_suffix() {
    let event = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 1_u64)
        .unwrap()
        .with(
            "message",
            ServerText::from_wire(b"x truncated=1 event=forged direct=1"),
        )
        .unwrap()
        .build()
        .unwrap();
    let encoded = syslog_encoder(Policy::Routine).encode(&event).unwrap();
    let (header, pairs) = parse_syslog(std::str::from_utf8(encoded.bytes()).unwrap()).unwrap();
    assert_eq!(header[5], "ironafs.diag.200");
    let keys: Vec<&str> = pairs.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        ["event", "v", "subsystem", "audience", "f_message", "msg"]
    );
    assert_eq!(pairs[0].1, "volume.offline");
    assert_eq!(
        pairs[5].1,
        "volume [omitted] offline: x truncated=1 event=forged direct=1"
    );
}
