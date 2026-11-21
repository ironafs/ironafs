// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rendering an event's message and field values as text, after redaction
//! and under a length limit on server text. Both encodings start from this
//! one rendering, so they cannot differ on what is shown
//! (`docs/specs/events.md` §5.4, §5.8, point 5, and §5.10).

use alloc::string::String;
use alloc::vec::Vec;

use crate::catalogue::FieldKind;
use crate::event::Event;
use crate::field::{Value, push_escaped};
use crate::redaction::{ABSENT_MARKER, Disposition, OMITTED_MARKER, Policy, disposition};

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

/// A field as an encoding writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderedField {
    pub(crate) name: &'static str,
    pub(crate) value: String,
    /// Whether the value is text, which syslog writes in quotes.
    pub(crate) text: bool,
}

/// An event rendered for an encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rendered {
    pub(crate) message: String,
    pub(crate) fields: Vec<RenderedField>,
    /// Whether any value was cut or any optional field dropped.
    pub(crate) truncated: bool,
}

/// Renders `event` under `policy` and `limits`.
pub(crate) fn render(event: &Event, policy: Policy, limits: Limits) -> Rendered {
    let def = event.def();
    let mut truncated = false;
    let mut fields = Vec::new();
    for (index, spec) in def.fields.iter().enumerate() {
        let Some(value) = event.value(index) else {
            continue;
        };
        if limits.drop_optional && !spec.required {
            truncated = true;
            continue;
        }
        if disposition(spec.class, def.stream, policy) == Disposition::Omit {
            continue;
        }
        let mut text = String::new();
        truncated |= push_value(&mut text, value, limits.text);
        fields.push(RenderedField {
            name: spec.name,
            value: text,
            text: matches!(spec.kind, FieldKind::Static | FieldKind::ServerText),
        });
    }
    let mut message = String::new();
    truncated |= render_template(&mut message, event, policy, limits);
    Rendered {
        message,
        fields,
        truncated,
    }
}

/// Appends a value's text; returns whether server text was cut.
fn push_value(out: &mut String, value: &Value, limit: Option<usize>) -> bool {
    match value {
        Value::Unsigned(number) => push_decimal(out, *number),
        Value::Signed(number) => {
            if *number < 0 {
                out.push('-');
            }
            push_decimal(out, number.unsigned_abs());
        }
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Static(text) => push_escaped(out, text),
        Value::ServerText(text) => return text.render(out, limit),
        Value::Reference(reference) => reference.push_hex(out),
    }
    false
}

/// Appends the template with each `{name}` replaced; returns whether
/// server text was cut. A placeholder the definition does not have, or an
/// unclosed brace, renders as the absent marker; the catalogue test
/// refuses both.
fn render_template(out: &mut String, event: &Event, policy: Policy, limits: Limits) -> bool {
    let def = event.def();
    let mut truncated = false;
    let mut rest = def.template;
    while let Some(open) = rest.find('{') {
        let (before, after) = rest.split_at(open);
        push_escaped(out, before);
        let after = after.strip_prefix('{').unwrap_or(after);
        let Some(close) = after.find('}') else {
            out.push_str(ABSENT_MARKER);
            return truncated;
        };
        let (name, tail) = after.split_at(close);
        rest = tail.strip_prefix('}').unwrap_or(tail);
        let Some(index) = def.field_index(name) else {
            out.push_str(ABSENT_MARKER);
            continue;
        };
        let spec = def.fields.get(index);
        let value = event.value(index);
        match (spec, value) {
            (Some(spec), Some(value)) => {
                if limits.drop_optional && !spec.required {
                    out.push_str(ABSENT_MARKER);
                } else if disposition(spec.class, def.stream, policy) == Disposition::Omit {
                    out.push_str(OMITTED_MARKER);
                } else {
                    truncated |= push_value(out, value, limits.text);
                }
            }
            _ => out.push_str(ABSENT_MARKER),
        }
    }
    push_escaped(out, rest);
    truncated
}

/// Appends `number` in decimal.
pub(crate) fn push_decimal(out: &mut String, number: u64) {
    let mut digits = [0_u8; 20];
    let mut used = 0_usize;
    let mut rest = number;
    loop {
        if let Some(slot) = digits.get_mut(used) {
            // `rest % 10` is below 10, so the cast cannot truncate.
            *slot = b'0' + u8::try_from(rest % 10).unwrap_or(0);
        }
        used += 1;
        rest /= 10;
        if rest == 0 || used == digits.len() {
            break;
        }
    }
    for &digit in digits.get(..used).unwrap_or(&[]).iter().rev() {
        out.push(char::from(digit));
    }
}
