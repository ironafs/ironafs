// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rendering an event's message and field values, after redaction and
//! under a length limit on server text, straight into a [`Sink`]. Both
//! encodings use these functions, so they cannot differ on what is shown
//! (`docs/specs/events.md` §5.4, §5.8, point 5, and §5.10).

use crate::catalogue::{FieldKind, FieldSpec};
use crate::event::Event;
use crate::field::{Value, push_escaped};
use crate::redaction::{ABSENT_MARKER, Disposition, OMITTED_MARKER, Policy, disposition};
use crate::sink::{Count, Sink};

/// How far an encoder cuts the event to fit its cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Limits {
    /// The most bytes of escapes each server text value renders, or none.
    pub(crate) text: Option<usize>,
    /// Whether every optional field is left out.
    pub(crate) drop_optional: bool,
}

impl Limits {
    pub(crate) const NONE: Self = Self {
        text: None,
        drop_optional: false,
    };
}

/// The fields an encoding writes, in catalogue order, with their values:
/// set, not dropped by `limits`, and not omitted by redaction.
pub(crate) fn written_fields(
    event: &Event,
    policy: Policy,
    limits: Limits,
) -> impl Iterator<Item = (&'static FieldSpec, &Value)> {
    let def = event.def();
    def.fields
        .iter()
        .enumerate()
        .filter_map(move |(index, spec)| Some((spec, event.value(index)?)))
        .filter(move |(spec, _)| {
            (!limits.drop_optional || spec.required)
                && disposition(spec.class, def.stream, policy) == Disposition::Write
        })
}

/// Whether a value is text, which syslog writes in quotes.
pub(crate) fn is_text(spec: &FieldSpec) -> bool {
    matches!(spec.kind, FieldKind::Static | FieldKind::ServerText)
}

/// Whether rendering under `limits` cuts any server text or drops any
/// optional field. It renders into a [`Count`], so it allocates nothing.
pub(crate) fn truncated(event: &Event, policy: Policy, limits: Limits) -> bool {
    let def = event.def();
    let dropped = limits.drop_optional
        && def
            .fields
            .iter()
            .enumerate()
            .any(|(index, spec)| !spec.required && event.value(index).is_some());
    let mut count = Count::default();
    let mut cut = write_message(&mut count, event, policy, limits);
    for (_, value) in written_fields(event, policy, limits) {
        cut |= write_value(&mut count, value, limits.text);
    }
    dropped || cut
}

/// Writes a value's text; returns whether server text was cut.
pub(crate) fn write_value(out: &mut impl Sink, value: &Value, limit: Option<usize>) -> bool {
    match value {
        Value::Unsigned(number) => push_decimal(out, *number),
        Value::Signed(number) => {
            if *number < 0 {
                out.put(b"-");
            }
            push_decimal(out, number.unsigned_abs());
        }
        Value::Bool(flag) => out.put(if *flag { b"true" } else { b"false" }),
        Value::Static(text) => push_escaped(out, text),
        Value::ServerText(text) => return text.render(out, limit),
        Value::Reference(reference) => reference.push_hex(out),
    }
    false
}

/// Writes the template with each `{name}` replaced; returns whether server
/// text was cut. A placeholder the definition does not have, or an unclosed
/// brace, renders as the absent marker; the catalogue test refuses both.
pub(crate) fn write_message(
    out: &mut impl Sink,
    event: &Event,
    policy: Policy,
    limits: Limits,
) -> bool {
    let def = event.def();
    let mut truncated = false;
    let mut rest = def.template;
    while let Some((before, after)) = rest.split_once('{') {
        push_escaped(out, before);
        let Some((name, tail)) = after.split_once('}') else {
            out.put(ABSENT_MARKER.as_bytes());
            return truncated;
        };
        rest = tail;
        let field = def
            .field_index(name)
            .and_then(|index| Some((def.fields.get(index)?, event.value(index)?)));
        match field {
            Some((spec, _)) if limits.drop_optional && !spec.required => {
                out.put(ABSENT_MARKER.as_bytes());
            }
            Some((spec, _)) if disposition(spec.class, def.stream, policy) == Disposition::Omit => {
                out.put(OMITTED_MARKER.as_bytes());
            }
            Some((_, value)) => truncated |= write_value(out, value, limits.text),
            None => out.put(ABSENT_MARKER.as_bytes()),
        }
    }
    push_escaped(out, rest);
    truncated
}

/// Writes `number` in decimal.
pub(crate) fn push_decimal(out: &mut impl Sink, number: u64) {
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    let mut rest = number;
    loop {
        start -= 1;
        if let Some(slot) = digits.get_mut(start) {
            // `rest % 10` is below 10, so the conversion cannot fail.
            *slot = b'0' + u8::try_from(rest % 10).unwrap_or(0);
        }
        rest /= 10;
        if rest == 0 || start == 0 {
            break;
        }
    }
    out.put(digits.get(start..).unwrap_or(&[]));
}
