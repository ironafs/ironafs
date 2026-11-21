// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fitting to the cap and marking truncation (`docs/specs/events.md`
//! §5.10; ADR 0005, point 1.7).

use ironafs_events::syslog::SYSLOG_FALLBACK_MAX;
use ironafs_events::{
    EncodeError, Event, Policy, Role, SERVER_TEXT_MAX, ServerText, Stream, TRUNCATION_MARKER,
    catalogue,
};

use crate::support::{field, journald_encoder, parse_journald, syslog_encoder};

fn volume_offline(message: Option<&[u8]>) -> Event {
    let builder = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 7_u64)
        .unwrap();
    match message {
        Some(bytes) => builder
            .with("message", ServerText::from_wire(bytes))
            .unwrap(),
        None => builder,
    }
    .build()
    .unwrap()
}

#[test]
fn server_text_is_cut_to_fit_and_marked() {
    let event = volume_offline(Some(&[b'a'; SERVER_TEXT_MAX]));
    let encoded = syslog_encoder(Policy::Routine).encode(&event).unwrap();
    let text = String::from_utf8(encoded.bytes().to_vec()).unwrap();
    assert!(encoded.truncated());
    assert!(text.len() <= SYSLOG_FALLBACK_MAX);
    assert!(text.contains(" truncated=1"), "{text}");
    assert!(text.contains(&format!("{TRUNCATION_MARKER}\"")), "{text}");
    assert!(text.contains(" f_message=\"a"), "{text}");
}

#[test]
fn the_cut_keeps_as_much_text_as_fits() {
    let event = volume_offline(Some(&[b'a'; SERVER_TEXT_MAX]));
    let encoded = syslog_encoder(Policy::Routine).encode(&event).unwrap();
    // One more byte of text appears twice (message and field), so a result
    // within two bytes of the cap is the largest that fits.
    assert!(
        encoded.bytes().len() + 2 > SYSLOG_FALLBACK_MAX,
        "{} octets",
        encoded.bytes().len()
    );
}

#[test]
fn optional_fields_are_dropped_when_cutting_text_is_not_enough() {
    let without = journald_encoder(Policy::Routine)
        .encode(&volume_offline(None))
        .unwrap();
    let truncated_line = b"IRONAFS_TRUNCATED=1\n".len();
    let cap = without.bytes().len() + truncated_line;
    let event = volume_offline(Some(b"server message"));
    let encoded = journald_encoder(Policy::Routine)
        .with_cap(cap)
        .encode(&event)
        .unwrap();
    assert!(encoded.truncated());
    let fields = parse_journald(encoded.bytes()).unwrap();
    assert_eq!(field(&fields, "IRONAFS_F_MESSAGE"), None);
    assert_eq!(field(&fields, "IRONAFS_TRUNCATED"), Some(&b"1"[..]));
    assert_eq!(
        field(&fields, "MESSAGE"),
        Some(&b"volume [omitted] offline: -"[..])
    );
    assert_eq!(field(&fields, "IRONAFS_EVENT"), Some(&b"200"[..]));
}

#[test]
fn an_event_whose_required_fields_do_not_fit_is_refused() {
    let event = volume_offline(None);
    assert_eq!(
        syslog_encoder(Policy::Routine).with_cap(50).encode(&event),
        Err(EncodeError::OverSizeLimit {
            stream: Stream::Diagnostic,
            id: 200
        })
    );
    let exact = syslog_encoder(Policy::Routine)
        .encode(&event)
        .unwrap()
        .bytes()
        .len();
    assert!(
        syslog_encoder(Policy::Routine)
            .with_cap(exact)
            .encode(&event)
            .is_ok()
    );
    assert_eq!(
        syslog_encoder(Policy::Routine)
            .with_cap(exact - 1)
            .encode(&event),
        Err(EncodeError::OverSizeLimit {
            stream: Stream::Diagnostic,
            id: 200
        })
    );
}

#[test]
fn text_cut_when_it_arrived_is_marked_even_when_it_fits() {
    let event = volume_offline(Some(&[b'b'; SERVER_TEXT_MAX + 10]));
    let encoded = journald_encoder(Policy::Routine).encode(&event).unwrap();
    assert!(encoded.truncated());
    let fields = parse_journald(encoded.bytes()).unwrap();
    let value = field(&fields, "IRONAFS_F_MESSAGE").unwrap();
    assert_eq!(value.len(), SERVER_TEXT_MAX + TRUNCATION_MARKER.len());
    assert!(value.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[test]
fn the_replacement_for_a_refused_event_names_it() {
    let refused = syslog_encoder(Policy::Routine)
        .with_cap(10)
        .encode(&volume_offline(None))
        .unwrap_err();
    let EncodeError::OverSizeLimit { stream, id } = refused;
    let replacement = Event::over_size(stream, id, Role::Daemon).unwrap();
    let encoded = syslog_encoder(Policy::Routine)
        .encode(&replacement)
        .unwrap();
    let text = String::from_utf8(encoded.bytes().to_vec()).unwrap();
    assert!(
        text.contains("event dropped: over the size limit (diagnostic event 200)"),
        "{text}"
    );
}
