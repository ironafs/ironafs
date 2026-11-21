// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Redaction: which fields an encoding writes, from each field's data class
//! and the policy (`docs/specs/events.md` §5.4; ADR 0004, O5, point 4, and
//! O6, points 1 and 5).
//!
//! Redaction looks only at the class. Server text is escaped wherever it is
//! written, under every policy, and no class holds a secret, so no policy
//! can relax either.

use crate::catalogue::{DataClass, Stream};

#[cfg(test)]
mod tests;

/// What routine logs reveal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Policy {
    /// The default: identifiers (paths, file names, native file ids,
    /// volume numbers) are omitted (ADR 0004, O6, point 1).
    #[default]
    Routine,
    /// Identifiers are written. The configuration sets this only under an
    /// administrator-locked policy whose changes are audited (O6, point 5);
    /// this crate cannot check that.
    RevealIdentifiers,
}

/// Whether a field is written, or omitted with a marker in the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Disposition {
    Write,
    Omit,
}

/// What happens to a field of `class` in an event of `stream` under
/// `policy`. Audit fields are catalogued audit fields, which O6, point 5,
/// lets carry native identifiers.
pub(crate) fn disposition(class: DataClass, stream: Stream, policy: Policy) -> Disposition {
    match (class, stream, policy) {
        (DataClass::Identifier, Stream::Diagnostic, Policy::Routine) => Disposition::Omit,
        _ => Disposition::Write,
    }
}

/// What the message shows in place of an omitted field.
pub(crate) const OMITTED_MARKER: &str = "[omitted]";

/// What the message shows in place of an optional field with no value.
pub(crate) const ABSENT_MARKER: &str = "-";
