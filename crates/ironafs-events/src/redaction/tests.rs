// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the redaction table (`docs/specs/events.md` §5.4).

use super::{Disposition, Policy, disposition};
use crate::catalogue::{DataClass, Stream};

#[test]
fn identifiers_are_omitted_from_routine_diagnostics() {
    assert_eq!(
        disposition(DataClass::Identifier, Stream::Diagnostic, Policy::Routine),
        Disposition::Omit
    );
}

#[test]
fn identifiers_are_written_under_reveal_identifiers() {
    assert_eq!(
        disposition(
            DataClass::Identifier,
            Stream::Diagnostic,
            Policy::RevealIdentifiers
        ),
        Disposition::Write
    );
}

#[test]
fn audit_identifiers_are_written_under_every_policy() {
    for policy in [Policy::Routine, Policy::RevealIdentifiers] {
        assert_eq!(
            disposition(DataClass::Identifier, Stream::Audit, policy),
            Disposition::Write
        );
    }
}

#[test]
fn other_classes_are_written_under_every_policy() {
    for class in [
        DataClass::Public,
        DataClass::ServerText,
        DataClass::Reference,
    ] {
        for stream in [Stream::Diagnostic, Stream::Audit] {
            for policy in [Policy::Routine, Policy::RevealIdentifiers] {
                assert_eq!(
                    disposition(class, stream, policy),
                    Disposition::Write,
                    "{class:?} {stream:?} {policy:?}"
                );
            }
        }
    }
}

#[test]
fn the_default_policy_is_routine() {
    assert_eq!(Policy::default(), Policy::Routine);
}
