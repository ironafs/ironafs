// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fitting an encoding to its sink's cap: cut server text, then drop
//! optional fields, and refuse an event whose identity and required fields
//! alone do not fit (`docs/specs/events.md` §5.10; ADR 0005, point 1.7).

use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use crate::catalogue::Stream;
use crate::event::Event;
use crate::field::Value;
use crate::redaction::Policy;
use crate::render::{Limits, Rendered, render};

/// A complete encoded message, within its sink's cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoded {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Encoded {
    /// The message's bytes, as the sink's transport sends them.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The message's bytes, by value.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Whether server text was cut or an optional field dropped to fit, or
    /// server text was cut when it arrived. The message is marked; the
    /// caller counts it ([`Loss::Truncated`](crate::Loss::Truncated)).
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Why an event was not encoded (`docs/specs/events.md` §7, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The event's identity and required fields alone, encoded, exceed the
    /// cap. The event is not delivered: the caller sends
    /// [`Event::over_size`] in its place and counts the loss.
    OverSizeLimit {
        /// The refused event's stream.
        stream: Stream,
        /// The refused event's id.
        id: u16,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverSizeLimit { stream, id } => write!(
                formatter,
                "{} event {id} is over the size limit",
                stream.name()
            ),
        }
    }
}

impl Error for EncodeError {}

/// Encodes `event` with `encode` within `cap` bytes, cutting as section
/// 5.10 says.
pub(crate) fn fit(
    event: &Event,
    policy: Policy,
    cap: usize,
    encode: impl Fn(&Rendered) -> Vec<u8>,
) -> Result<Encoded, EncodeError> {
    let attempt = |limits: Limits| {
        let rendered = render(event, policy, limits);
        let bytes = encode(&rendered);
        (bytes.len() <= cap).then_some(Encoded {
            bytes,
            truncated: rendered.truncated,
        })
    };
    if let Some(whole) = attempt(Limits::NONE) {
        return Ok(whole);
    }
    let longest = longest_text(event);
    for drop_optional in [false, true] {
        if drop_optional {
            let whole_text = Limits {
                text: None,
                drop_optional,
            };
            if let Some(encoded) = attempt(whole_text) {
                return Ok(encoded);
            }
        }
        let limits = |text| Limits {
            text: Some(text),
            drop_optional,
        };
        let Some(mut best) = attempt(limits(0)) else {
            continue;
        };
        // The largest length that fits lies in 0..longest: bisect, keeping
        // `low` a length that fits and `high` one past the last candidate.
        let mut low = 0_usize;
        let mut high = longest;
        while high.saturating_sub(low) > 1 {
            let middle = low + (high - low) / 2;
            if let Some(encoded) = attempt(limits(middle)) {
                low = middle;
                best = encoded;
            } else {
                high = middle;
            }
        }
        return Ok(best);
    }
    let def = event.def();
    Err(EncodeError::OverSizeLimit {
        stream: def.stream,
        id: def.id,
    })
}

/// The longest escaped rendering of any server text value in `event`.
fn longest_text(event: &Event) -> usize {
    (0..event.def().fields.len())
        .filter_map(|index| match event.value(index) {
            Some(Value::ServerText(text)) => Some(text.escaped_len()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}
