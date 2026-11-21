// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers: a journald native-protocol reader written from
//! `docs/specs/events.md` §5.8, point 1, an escape written from §5.3, point
//! 2, independently of the crate's, and a generator of arbitrary events.

use std::fmt::Write as _;

use ironafs_events::journald::JournaldEncoder;
use ironafs_events::syslog::{AppName, Origin, SyslogEncoder};
use ironafs_events::{Event, FieldKind, Policy, Reference, Role, ServerText, catalogue};
use ironafs_test_support::Source;

/// One journald field: its name and its value's bytes.
pub(crate) type Field = (String, Vec<u8>);

/// Reads a native-protocol message into its fields, in order.
pub(crate) fn parse_journald(mut bytes: &[u8]) -> Result<Vec<Field>, String> {
    let mut fields = Vec::new();
    while !bytes.is_empty() {
        let end = bytes
            .iter()
            .position(|&byte| byte == b'\n' || byte == b'=')
            .ok_or("a field with no end")?;
        let name =
            String::from_utf8(bytes[..end].to_vec()).map_err(|_| "a name that is not UTF-8")?;
        if bytes[end] == b'=' {
            let rest = &bytes[end + 1..];
            let line = rest
                .iter()
                .position(|&byte| byte == b'\n')
                .ok_or("a value with no newline")?;
            fields.push((name, rest[..line].to_vec()));
            bytes = &rest[line + 1..];
        } else {
            let rest = &bytes[end + 1..];
            let length: [u8; 8] = rest.get(..8).ok_or("a short length")?.try_into().unwrap();
            let length =
                usize::try_from(u64::from_le_bytes(length)).map_err(|_| "a huge length")?;
            let value = rest.get(8..8 + length).ok_or("a short value")?;
            if rest.get(8 + length) != Some(&b'\n') {
                return Err("no newline after a binary value".into());
            }
            fields.push((name, value.to_vec()));
            bytes = &rest[8 + length + 1..];
        }
    }
    Ok(fields)
}

/// The value of the one field called `name`.
pub(crate) fn field<'a>(fields: &'a [Field], name: &str) -> Option<&'a [u8]> {
    let mut found = fields.iter().filter(|(candidate, _)| candidate == name);
    let first = found.next().map(|(_, value)| value.as_slice());
    assert!(found.next().is_none(), "{name} appears twice");
    first
}

/// Whether `name` meets `docs/specs/events.md` §5.8, point 3.
pub(crate) fn is_journald_name(name: &str) -> bool {
    name.len() <= 64
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_uppercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// `docs/specs/events.md` §5.3, point 2, written again for the tests.
pub(crate) fn escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            0x20..=0x7e => out.push(char::from(byte)),
            _ => write!(out, "\\x{byte:02x}").unwrap(),
        }
    }
    out
}

pub(crate) fn journald_encoder(policy: Policy) -> JournaldEncoder {
    JournaldEncoder::new(AppName::new("ironafsd").unwrap(), policy)
}

pub(crate) fn syslog_encoder(policy: Policy) -> SyslogEncoder {
    let origin = Origin::new("client1", AppName::new("ironafsd").unwrap(), Some(4242));
    SyslogEncoder::new(origin, policy)
}

const STATIC_TEXTS: [&str; 4] = ["queue full", "known-loss", "", "a \"quoted\" \\ text"];

/// An arbitrary event of any definition, with arbitrary values, some
/// optional fields left out.
pub(crate) fn arbitrary_event(source: &mut Source) -> Event {
    let def = *source.pick(catalogue::ALL);
    let mut builder = Event::builder(def, Role::Daemon).unwrap();
    for spec in def.fields {
        if !spec.required && source.bool() {
            continue;
        }
        builder = match spec.kind {
            FieldKind::Unsigned => builder.with(spec.name, source.u64()),
            FieldKind::Signed => builder.with(spec.name, source.i64()),
            FieldKind::Bool => builder.with(spec.name, source.bool()),
            FieldKind::Static => builder.with(spec.name, *source.pick(&STATIC_TEXTS)),
            FieldKind::ServerText => {
                builder.with(spec.name, ServerText::from_wire(&source.bytes(0..=300)))
            }
            FieldKind::Reference => {
                let bytes: [u8; 32] = source.bytes(32..=32).try_into().unwrap();
                builder.with(spec.name, Reference::from_bytes(bytes))
            }
        }
        .unwrap();
    }
    if def.stream == catalogue::Stream::Diagnostic && source.bool() {
        builder = builder.direct().unwrap();
    }
    builder.build().unwrap()
}
