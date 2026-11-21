// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for building events (`docs/specs/events.md` §5.5 and §7, point
//! 1).

use alloc::string::ToString;

use super::{Event, EventError};
use crate::catalogue::{self, Role, Stream};
use crate::field::{ServerText, Value};

#[test]
fn builder_refuses_a_role_the_definition_does_not_list() {
    assert_eq!(
        Event::builder(&catalogue::VOLUME_OFFLINE, Role::Cli).err(),
        Some(EventError::RoleNotAllowed {
            event: "volume.offline",
            role: Role::Cli
        })
    );
    for role in [
        Role::Adapter,
        Role::SmbFrontEnd,
        Role::WindowsHelper,
        Role::Cli,
        Role::LoginService,
    ] {
        assert_eq!(
            Event::builder(&catalogue::AUDIT_GAP, role).err(),
            Some(EventError::RoleNotAllowed {
                event: "audit.gap",
                role
            })
        );
    }
}

#[test]
fn direct_audit_is_refused() {
    let builder = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon).unwrap();
    assert_eq!(
        builder.direct().err(),
        Some(EventError::DirectAudit { event: "audit.gap" })
    );
}

#[test]
fn direct_diagnostic_is_marked() {
    let event = Event::builder(&catalogue::EVENTS_LOST, Role::Adapter)
        .unwrap()
        .direct()
        .unwrap()
        .with("reason", "queue full")
        .unwrap()
        .with("count", 1_u64)
        .unwrap()
        .build()
        .unwrap();
    assert!(event.is_direct());
}

#[test]
fn with_refuses_an_unknown_field() {
    let builder = Event::builder(&catalogue::EVENTS_LOST, Role::Daemon).unwrap();
    assert_eq!(
        builder.with("token", 1_u64).err(),
        Some(EventError::UnknownField {
            event: "events.lost"
        })
    );
}

#[test]
fn with_refuses_a_value_of_another_kind() {
    let builder = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon).unwrap();
    assert_eq!(
        builder.clone().with("message", "static text").err(),
        Some(EventError::KindMismatch {
            event: "volume.offline",
            field: "message"
        })
    );
    assert_eq!(
        builder.with("volume", ServerText::from_wire(b"7")).err(),
        Some(EventError::KindMismatch {
            event: "volume.offline",
            field: "volume"
        })
    );
}

#[test]
fn with_refuses_a_second_value() {
    let builder = Event::builder(&catalogue::EVENTS_LOST, Role::Daemon)
        .unwrap()
        .with("count", 1_u64)
        .unwrap();
    assert_eq!(
        builder.with("count", 2_u64).err(),
        Some(EventError::DuplicateField {
            event: "events.lost",
            field: "count"
        })
    );
}

#[test]
fn build_refuses_a_missing_required_field() {
    let builder = Event::builder(&catalogue::EVENTS_LOST, Role::Daemon)
        .unwrap()
        .with("count", 1_u64)
        .unwrap();
    assert_eq!(
        builder.build().err(),
        Some(EventError::MissingField {
            event: "events.lost",
            field: "reason"
        })
    );
}

#[test]
fn build_accepts_a_missing_optional_field() {
    let event = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 7_u32)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(event.value(0), Some(&Value::Unsigned(7)));
    assert_eq!(event.value(1), None);
}

#[test]
fn over_size_names_the_dropped_event() {
    let event = Event::over_size(Stream::Audit, 1, Role::Daemon).unwrap();
    assert_eq!(event.def().name, "events.over_size");
    assert_eq!(event.value(0), Some(&Value::Static("audit")));
    assert_eq!(event.value(1), Some(&Value::Unsigned(1)));
}

#[test]
fn errors_display_in_lower_case_with_no_value() {
    let error = EventError::KindMismatch {
        event: "volume.offline",
        field: "volume",
    };
    assert_eq!(
        error.to_string(),
        "field volume of event volume.offline has another kind"
    );
}

#[test]
fn with_refuses_a_static_value_outside_the_fields_choices() {
    let builder = Event::builder(&catalogue::AUDIT_GAP, Role::Daemon).unwrap();
    assert_eq!(
        builder.with("kind", "anything at all").err(),
        Some(EventError::ValueNotAllowed {
            event: "audit.gap",
            field: "kind"
        })
    );
}

#[test]
fn debug_shows_no_field_value() {
    let event = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
        .unwrap()
        .with("volume", 123_456_u64)
        .unwrap()
        .with("message", ServerText::from_wire(b"sensitive"))
        .unwrap()
        .build()
        .unwrap();
    let shown = alloc::format!("{event:?}");
    assert!(!shown.contains("123456"), "{shown}");
    assert!(!shown.contains("sensitive"), "{shown}");
    assert!(shown.contains("volume.offline"), "{shown}");
}
