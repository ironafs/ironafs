// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The journald native-protocol encoding (`docs/specs/events.md` §5.8;
//! ADR 0004, O8, point 1; ADR 0005, point 1.7).
//!
//! A message is a list of fields. A value with no newline is written as
//! `NAME=value` and a newline; a value with one, as the name, a newline,
//! the value's length as a little-endian 64-bit integer, the value and a
//! newline (systemd's "Native Journal Protocol"). This encoder writes only
//! names of upper-case letters, digits and `_` that start with a letter,
//! so it never writes a trusted field.

use alloc::string::String;
use alloc::vec::Vec;

use crate::catalogue::Stream;
use crate::event::Event;
use crate::field::hex_digit;
use crate::fit::{EncodeError, Encoded, fit};
use crate::redaction::Policy;
use crate::render::{Rendered, push_decimal};
use crate::syslog::AppName;

#[cfg(test)]
mod tests;

/// The cap on a complete journald message (ADR 0005, point 1.7).
pub const JOURNALD_MAX_DATAGRAM: usize = 16 * 1024;

/// The first 24 hex digits of every `MESSAGE_ID`: a constant chosen for
/// IronAFS. The last 8 are the stream and the id.
pub const MESSAGE_ID_PREFIX: &str = "771546f4bcf0d0b7054f7530";

/// The longest field name written.
pub const FIELD_NAME_MAX: usize = 64;

/// Encodes events as journald native-protocol messages within a cap.
///
/// ```
/// use ironafs_events::journald::JournaldEncoder;
/// use ironafs_events::syslog::AppName;
/// use ironafs_events::{Event, Policy, Role, catalogue};
///
/// let encoder = JournaldEncoder::new(AppName::new("ironafsd")?, Policy::Routine);
/// let event = Event::builder(&catalogue::EVENTS_LOST, Role::Daemon)?
///     .with("reason", "queue full")?
///     .with("count", 3_u64)?
///     .build()?;
/// let encoded = encoder.encode(&event)?;
/// assert!(encoded.bytes().starts_with(b"MESSAGE=3 events lost: queue full\n"));
/// # Ok::<(), Box<dyn core::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournaldEncoder {
    identifier: AppName,
    policy: Policy,
    cap: usize,
}

impl JournaldEncoder {
    /// An encoder with the cap [`JOURNALD_MAX_DATAGRAM`], writing
    /// `identifier` as `SYSLOG_IDENTIFIER`.
    #[must_use]
    pub fn new(identifier: AppName, policy: Policy) -> Self {
        Self {
            identifier,
            policy,
            cap: JOURNALD_MAX_DATAGRAM,
        }
    }

    /// The same encoder with a lower cap. A cap above
    /// [`JOURNALD_MAX_DATAGRAM`] is lowered to it.
    #[must_use]
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = cap.min(JOURNALD_MAX_DATAGRAM);
        self
    }

    /// Encodes `event`, fitted to the cap.
    ///
    /// # Errors
    ///
    /// [`EncodeError::OverSizeLimit`] if the event's identity and required
    /// fields alone do not fit.
    pub fn encode(&self, event: &Event) -> Result<Encoded, EncodeError> {
        fit(event, self.policy, self.cap, |rendered| {
            self.write(event, rendered)
        })
    }

    fn write(&self, event: &Event, rendered: &Rendered) -> Vec<u8> {
        let def = event.def();
        let mut out = Vec::new();
        let mut number = String::new();
        push_field(&mut out, "MESSAGE", rendered.message.as_bytes());
        push_field(
            &mut out,
            "MESSAGE_ID",
            message_id(def.stream, def.id).as_bytes(),
        );
        number.clear();
        push_decimal(&mut number, u64::from(def.severity.number()));
        push_field(&mut out, "PRIORITY", number.as_bytes());
        push_field(
            &mut out,
            "SYSLOG_IDENTIFIER",
            self.identifier.as_str().as_bytes(),
        );
        number.clear();
        push_decimal(&mut number, u64::from(def.id));
        push_field(&mut out, "IRONAFS_EVENT", number.as_bytes());
        number.clear();
        push_decimal(&mut number, u64::from(def.version));
        push_field(&mut out, "IRONAFS_EVENT_VERSION", number.as_bytes());
        push_field(&mut out, "IRONAFS_EVENT_NAME", def.name.as_bytes());
        push_field(&mut out, "IRONAFS_STREAM", def.stream.name().as_bytes());
        push_field(
            &mut out,
            "IRONAFS_SUBSYSTEM",
            def.subsystem.name().as_bytes(),
        );
        push_field(&mut out, "IRONAFS_AUDIENCE", def.audience.name().as_bytes());
        if event.is_direct() {
            push_field(&mut out, "IRONAFS_DIRECT", b"1");
        }
        if rendered.truncated {
            push_field(&mut out, "IRONAFS_TRUNCATED", b"1");
        }
        let mut name = String::new();
        for field in &rendered.fields {
            name.clear();
            name.push_str("IRONAFS_F_");
            name.push_str(&field.name.to_ascii_uppercase());
            push_field(&mut out, &name, field.value.as_bytes());
        }
        out
    }
}

/// The `MESSAGE_ID` of an event: the prefix, then the stream and the id as
/// four lower-case hex digits each. The same for every version, so the
/// journal catalogue keys on the event's meaning.
#[must_use]
pub fn message_id(stream: Stream, id: u16) -> String {
    let stream_number: u16 = match stream {
        Stream::Diagnostic => 1,
        Stream::Audit => 2,
    };
    let mut out = String::from(MESSAGE_ID_PREFIX);
    for value in [stream_number, id] {
        for shift in [12_u16, 8, 4, 0] {
            // Each nibble is below 16, so the cast cannot truncate.
            out.push(hex_digit(
                u8::try_from((value >> shift) & 0x0f).unwrap_or(0),
            ));
        }
    }
    out
}

/// Appends one field, in the simple form when the value holds no newline
/// and in the length-prefixed form when it does.
pub(crate) fn push_field(out: &mut Vec<u8>, name: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    if value.contains(&b'\n') {
        out.push(b'\n');
        // A `usize` length always fits in 64 bits on the targets IronAFS
        // builds for; on overflow the length written would be wrong, so the
        // field is left empty instead.
        let length = u64::try_from(value.len()).unwrap_or(0);
        out.extend_from_slice(&length.to_le_bytes());
        if length != 0 {
            out.extend_from_slice(value);
        }
    } else {
        out.push(b'=');
        out.extend_from_slice(value);
    }
    out.push(b'\n');
}
