// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the catalogue's own rules (`docs/specs/events.md` §5.1): every
//! entry is checked, so a new entry that breaks a rule fails here.

use alloc::vec::Vec;

use super::{ALL, Audience, FieldKind, MAX_FIELDS, Roles, Stream, Subsystem, find};
use crate::limit::Loss;

fn is_name(text: &str, max: usize, extra: &[u8]) -> bool {
    !text.is_empty()
        && text.len() <= max
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || extra.contains(&byte))
}

#[test]
fn every_field_has_an_allowed_class() {
    for def in ALL {
        for field in def.fields {
            assert!(
                field.kind.allows(field.class),
                "{}.{}: {:?} may not be {:?}",
                def.name,
                field.name,
                field.kind,
                field.class
            );
        }
    }
}

#[test]
fn ids_are_unique_per_stream() {
    let mut seen = Vec::new();
    for def in ALL {
        assert!(
            !seen.contains(&(def.stream, def.id)),
            "{} reuses its id",
            def.name
        );
        seen.push((def.stream, def.id));
    }
}

#[test]
fn names_are_unique_and_well_formed() {
    let mut seen = Vec::new();
    for def in ALL {
        assert!(
            is_name(def.name, 32, b"._"),
            "{} is not a well-formed name",
            def.name
        );
        assert!(!seen.contains(&def.name), "{} is used twice", def.name);
        seen.push(def.name);
    }
}

#[test]
fn field_names_are_well_formed_and_unique_within_their_event() {
    for def in ALL {
        assert!(
            def.fields.len() <= MAX_FIELDS,
            "{} has too many fields",
            def.name
        );
        for (index, field) in def.fields.iter().enumerate() {
            assert!(
                is_name(field.name, 24, b"_"),
                "{}.{} is not well formed",
                def.name,
                field.name
            );
            assert_eq!(
                def.field_index(field.name),
                Some(index),
                "{}.{} is used twice",
                def.name,
                field.name
            );
        }
    }
}

#[test]
fn templates_name_only_their_fields() {
    for def in ALL {
        let mut rest = def.template;
        while let Some((before, after)) = rest.split_once('{') {
            assert!(!before.contains('}'), "{}: a stray closing brace", def.name);
            let (name, tail) = after
                .split_once('}')
                .unwrap_or_else(|| panic!("{}: an unclosed brace", def.name));
            assert!(!name.contains('{'), "{}: a nested brace", def.name);
            assert!(
                def.field_index(name).is_some(),
                "{}: no field {name}",
                def.name
            );
            rest = tail;
        }
        assert!(!rest.contains('}'), "{}: a stray closing brace", def.name);
    }
}

#[test]
fn audit_events_come_only_from_the_daemon() {
    for def in ALL {
        assert_eq!(
            def.stream == Stream::Audit,
            def.audience == Audience::Audit,
            "{}",
            def.name
        );
        if def.stream == Stream::Audit {
            assert_eq!(
                def.producers,
                Roles::DAEMON,
                "{} has another producer",
                def.name
            );
        }
    }
}

#[test]
fn every_definition_is_complete() {
    for def in ALL {
        assert!(def.version >= 1, "{}", def.name);
        assert!(!def.producers.is_empty(), "{}", def.name);
        assert!(!def.template.is_empty(), "{}", def.name);
        assert!(!def.meaning.is_empty(), "{}", def.name);
        assert!(!def.remedy.is_empty(), "{}", def.name);
    }
}

#[test]
fn find_returns_the_definition_of_that_stream_and_id() {
    assert_eq!(
        find(Stream::Audit, 1).map(|def| def.name),
        Some("audit.gap")
    );
    assert_eq!(
        find(Stream::Diagnostic, 1).map(|def| def.name),
        Some("events.lost")
    );
    assert_eq!(find(Stream::Diagnostic, 65_535), None);
}

#[test]
fn an_event_has_at_most_one_server_text_field() {
    for def in ALL {
        let count = def
            .fields
            .iter()
            .filter(|field| field.kind == FieldKind::ServerText)
            .count();
        assert!(count <= 1, "{} has {count} server text fields", def.name);
    }
}

#[test]
fn static_fields_and_only_they_have_well_formed_choices() {
    for def in ALL {
        for field in def.fields {
            assert_eq!(
                field.kind == FieldKind::Static,
                !field.choices.is_empty(),
                "{}.{}",
                def.name,
                field.name
            );
            for choice in field.choices {
                assert!(
                    !choice.is_empty()
                        && choice.len() <= 32
                        && choice.bytes().all(|byte| (0x20..=0x7e).contains(&byte)),
                    "{}.{}: {choice:?}",
                    def.name,
                    field.name
                );
            }
        }
    }
}

#[test]
fn the_summary_events_choices_are_the_names_the_code_writes() {
    let choices =
        |def: &super::EventDef, name: &str| def.fields[def.field_index(name).unwrap()].choices;
    let reasons = choices(&super::EVENTS_LOST, "reason");
    for loss in Loss::ALL {
        assert!(reasons.contains(&loss.reason()), "{loss:?}");
    }
    let subsystems = choices(&super::EVENTS_SUPPRESSED, "subsystem");
    for subsystem in Subsystem::ALL {
        assert!(subsystems.contains(&subsystem.name()), "{subsystem:?}");
    }
    let streams = choices(&super::EVENTS_OVER_SIZE, "stream");
    for stream in [Stream::Diagnostic, Stream::Audit] {
        assert!(streams.contains(&stream.name()), "{stream:?}");
    }
}
