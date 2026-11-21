// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Field values: the closed list of types that can become a field, the
//! refusal of every other type, and server-supplied text, which renders
//! only escaped (`docs/specs/events.md` §5.2 and §5.3; ADR 0004, O5,
//! points 1 and 3).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::catalogue::FieldKind;

#[cfg(test)]
mod tests;

/// What a secret type's `Debug` and `Display` print (ADR 0004, O5, point
/// 1).
pub const REDACTION_MARKER: &str = "[redacted]";

/// What follows server text that was cut (`docs/specs/events.md` §5.3,
/// point 3).
pub const TRUNCATION_MARKER: &str = "[truncated]";

/// The most bytes of server text an event keeps (`docs/specs/events.md`
/// §5.3, point 1).
pub const SERVER_TEXT_MAX: usize = 256;

/// Writes [`REDACTION_MARKER`]: the whole of a secret type's `Debug` and
/// `Display` (`docs/specs/events.md` §5.2, point 5).
///
/// ```
/// use core::fmt;
///
/// struct SessionKey([u8; 16]);
///
/// impl fmt::Debug for SessionKey {
///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
///         ironafs_events::write_redacted(formatter)
///     }
/// }
///
/// assert_eq!(format!("{:?}", SessionKey([7; 16])), "[redacted]");
/// ```
///
/// # Errors
///
/// Returns the formatter's error.
pub fn write_redacted(formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(REDACTION_MARKER)
}

mod sealed {
    use super::{Reference, ServerText};

    /// A field value inside an event. It is not part of the public
    /// interface: values are built only through
    /// [`FieldSource`](super::FieldSource).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Value {
        Unsigned(u64),
        Signed(i64),
        Bool(bool),
        Static(&'static str),
        ServerText(ServerText),
        Reference(Reference),
    }

    /// Seals [`FieldSource`](super::FieldSource): no other crate can name
    /// this trait, so none can add a type to the list.
    pub trait Sealed {
        fn into_value(self) -> Value;
    }
}

pub(crate) use sealed::Value;

/// A type that can become an event field: exactly `u8`, `u16`, `u32`,
/// `u64`, `i32`, `i64`, `bool`, `&'static str`, [`ServerText`] and
/// [`Reference`] (`docs/specs/events.md` §5.2).
///
/// The trait is sealed, so the compiler refuses every other type as a
/// field. A secret (a token, a key, a session key, a Kerberos ticket) is
/// refused:
///
/// ```compile_fail,E0277
/// use ironafs_events::{Event, Role, catalogue};
///
/// struct SessionKey([u8; 16]);
///
/// let _ = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)
///     .and_then(|builder| builder.with("volume", SessionKey([0; 16])));
/// ```
///
/// and no other crate can add one to the list:
///
/// ```compile_fail,E0277
/// struct SessionKey([u8; 16]);
///
/// impl ironafs_events::FieldSource for SessionKey {}
/// ```
pub trait FieldSource: sealed::Sealed {}

macro_rules! unsigned_source {
    ($($type:ty),*) => {$(
        impl sealed::Sealed for $type {
            fn into_value(self) -> Value {
                Value::Unsigned(u64::from(self))
            }
        }
        impl FieldSource for $type {}
    )*};
}

macro_rules! signed_source {
    ($($type:ty),*) => {$(
        impl sealed::Sealed for $type {
            fn into_value(self) -> Value {
                Value::Signed(i64::from(self))
            }
        }
        impl FieldSource for $type {}
    )*};
}

unsigned_source!(u8, u16, u32, u64);
signed_source!(i32, i64);

impl sealed::Sealed for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}
impl FieldSource for bool {}

impl sealed::Sealed for &'static str {
    fn into_value(self) -> Value {
        Value::Static(self)
    }
}
impl FieldSource for &'static str {}

impl sealed::Sealed for ServerText {
    fn into_value(self) -> Value {
        Value::ServerText(self)
    }
}
impl FieldSource for ServerText {}

impl sealed::Sealed for Reference {
    fn into_value(self) -> Value {
        Value::Reference(self)
    }
}
impl FieldSource for Reference {}

impl Value {
    pub(crate) fn kind(&self) -> FieldKind {
        match self {
            Self::Unsigned(_) => FieldKind::Unsigned,
            Self::Signed(_) => FieldKind::Signed,
            Self::Bool(_) => FieldKind::Bool,
            Self::Static(_) => FieldKind::Static,
            Self::ServerText(_) => FieldKind::ServerText,
            Self::Reference(_) => FieldKind::Reference,
        }
    }
}

/// Text a server sent, kept to [`SERVER_TEXT_MAX`] bytes and rendered only
/// escaped (`docs/specs/events.md` §5.3; ADR 0004, O5, point 3). Its
/// `Debug` shows the escaped form too.
///
/// ```
/// use ironafs_events::ServerText;
///
/// let text = ServerText::from_wire(b"offline\nPRIORITY=0");
/// assert_eq!(format!("{text:?}"), r#"ServerText("offline\x0aPRIORITY=0")"#);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ServerText {
    bytes: Vec<u8>,
    cut: bool,
}

impl ServerText {
    /// Keeps at most [`SERVER_TEXT_MAX`] bytes of `bytes`, as they arrived.
    #[must_use]
    pub fn from_wire(bytes: &[u8]) -> Self {
        let kept = bytes.get(..SERVER_TEXT_MAX).unwrap_or(bytes);
        Self {
            bytes: kept.to_vec(),
            cut: kept.len() < bytes.len(),
        }
    }

    /// Whether the text was longer than [`SERVER_TEXT_MAX`] and was cut.
    #[must_use]
    pub fn was_cut(&self) -> bool {
        self.cut
    }

    /// The length of the whole escaped rendering, without a marker.
    pub(crate) fn escaped_len(&self) -> usize {
        self.bytes.iter().map(|&byte| escape(byte).len()).sum()
    }

    /// Appends the escaped rendering to `out`: at most `limit` bytes of
    /// escapes, never splitting one, and [`TRUNCATION_MARKER`] if anything
    /// was left out here or when the text arrived. Returns whether the
    /// marker was written.
    pub(crate) fn render(&self, out: &mut String, limit: Option<usize>) -> bool {
        let mut written = 0_usize;
        let mut cut = self.cut;
        for &byte in &self.bytes {
            let escaped = escape(byte);
            let next = written.saturating_add(escaped.len());
            if limit.is_some_and(|limit| next > limit) {
                cut = true;
                break;
            }
            escaped.push_to(out);
            written = next;
        }
        if cut {
            out.push_str(TRUNCATION_MARKER);
        }
        cut
    }
}

impl fmt::Debug for ServerText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut escaped = String::new();
        self.render(&mut escaped, None);
        formatter
            .debug_tuple("ServerText")
            .field(&format_args!("\"{escaped}\""))
            .finish()
    }
}

/// One byte's escape (`docs/specs/events.md` §5.3, point 2): itself, a
/// backslash and itself, or `\x` and two lower-case hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Escape {
    Plain(u8),
    Backslashed(u8),
    Hex(u8),
}

impl Escape {
    pub(crate) const fn len(self) -> usize {
        match self {
            Self::Plain(_) => 1,
            Self::Backslashed(_) => 2,
            Self::Hex(_) => 4,
        }
    }

    fn push_to(self, out: &mut String) {
        match self {
            Self::Plain(byte) => out.push(char::from(byte)),
            Self::Backslashed(byte) => {
                out.push('\\');
                out.push(char::from(byte));
            }
            Self::Hex(byte) => {
                out.push_str("\\x");
                out.push(hex_digit(byte >> 4));
                out.push(hex_digit(byte & 0x0f));
            }
        }
    }
}

pub(crate) const fn escape(byte: u8) -> Escape {
    match byte {
        b'\\' | b'"' => Escape::Backslashed(byte),
        0x20..=0x7e => Escape::Plain(byte),
        _ => Escape::Hex(byte),
    }
}

/// Appends `text` escaped, with no limit. Static text is IronAFS's own,
/// but is escaped the same way so no field ever holds a control byte.
pub(crate) fn push_escaped(out: &mut String, text: &str) {
    for &byte in text.as_bytes() {
        escape(byte).push_to(out);
    }
}

pub(crate) fn hex_digit(nibble: u8) -> char {
    char::from_digit(u32::from(nibble & 0x0f), 16).unwrap_or('0')
}

/// An opaque reference: the daemon's keyed hash of an object (ADR 0004,
/// O6, point 2). This crate carries the 32 bytes; it never computes them.
/// It renders as 64 lower-case hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reference([u8; 32]);

impl Reference {
    /// The reference with these bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) fn push_hex(&self, out: &mut String) {
        for &byte in &self.0 {
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte));
        }
    }
}
