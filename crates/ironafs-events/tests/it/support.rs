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
pub(crate) fn parse_journald(bytes: &[u8]) -> Result<Vec<Field>, String> {
    parse_journald_forms(bytes).map(|fields| {
        fields
            .into_iter()
            .map(|(name, value, _)| (name, value))
            .collect()
    })
}

/// Reads a native-protocol message into its fields, each with whether it
/// took the length-prefixed form.
pub(crate) fn parse_journald_forms(
    mut bytes: &[u8],
) -> Result<Vec<(String, Vec<u8>, bool)>, String> {
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
            fields.push((name, rest[..line].to_vec(), false));
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
            fields.push((name, value.to_vec(), true));
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

/// An arbitrary event of any definition, with arbitrary values, some
/// optional fields left out.
pub(crate) fn arbitrary_event(source: &mut Source) -> Event {
    let def = *source.pick(catalogue::ALL);
    let mut builder = Event::builder(def, Role::Daemon).unwrap();
    for spec in def.fields() {
        if !spec.required() && source.bool() {
            continue;
        }
        builder = match spec.kind() {
            FieldKind::Unsigned => builder.with(spec.name(), source.u64()),
            FieldKind::Signed => builder.with(spec.name(), source.i64()),
            FieldKind::Bool => builder.with(spec.name(), source.bool()),
            FieldKind::Static => builder.with(spec.name(), *source.pick(spec.choices())),
            FieldKind::ServerText => {
                builder.with(spec.name(), ServerText::from_wire(&source.bytes(0..=300)))
            }
            FieldKind::Reference => {
                let bytes: [u8; 32] = source.bytes(32..=32).try_into().unwrap();
                builder.with(spec.name(), Reference::from_bytes(bytes))
            }
        }
        .unwrap();
    }
    if def.stream() == catalogue::Stream::Diagnostic && source.bool() {
        builder = builder.direct().unwrap();
    }
    builder.build().unwrap()
}

/// The `key=value` pairs of an RFC 5424 `MSG`, values unescaped.
pub(crate) type Pairs = Vec<(String, String)>;

/// Splits an RFC 5424 message into its seven header fields and the pairs
/// of its `MSG`, read strictly as `docs/specs/events.md` §5.9, point 6,
/// lays it out: `key=value` pairs separated by one space, a value bare
/// (no space or quote) or in double quotes with `\"` and `\\` escapes,
/// and nothing else.
pub(crate) fn parse_syslog(text: &str) -> Result<(Vec<String>, Pairs), String> {
    let mut parts = text.splitn(8, ' ');
    let header: Vec<String> = parts.by_ref().take(7).map(String::from).collect();
    let mut rest = parts.next().ok_or("no MSG")?;
    let mut pairs = Vec::new();
    while !rest.is_empty() {
        let (key, after) = rest
            .split_once('=')
            .ok_or_else(|| format!("no `=` in {rest:?}"))?;
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!("not a key: {key:?}"));
        }
        let (value, tail) = if let Some(quoted) = after.strip_prefix('"') {
            let mut value = String::new();
            let mut chars = quoted.char_indices();
            let end = loop {
                match chars.next() {
                    Some((_, '\\')) => value.push(chars.next().ok_or("a trailing backslash")?.1),
                    Some((index, '"')) => break index + 1,
                    Some((_, other)) => value.push(other),
                    None => return Err("an unclosed quote".into()),
                }
            };
            (value, quoted.get(end..).ok_or("a split character")?)
        } else {
            let (bare, tail) = after.split_at(after.find(' ').unwrap_or(after.len()));
            if bare.contains('"') {
                return Err("a quote in a bare value".into());
            }
            (bare.to_string(), tail)
        };
        pairs.push((key.to_string(), value));
        rest = match tail.strip_prefix(' ') {
            Some(next) if !next.is_empty() => next,
            Some(_) => return Err("a trailing space".into()),
            None if tail.is_empty() => tail,
            None => return Err(format!("no space before {tail:?}")),
        };
    }
    Ok((header, pairs))
}
