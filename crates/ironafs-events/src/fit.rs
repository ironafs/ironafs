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
use crate::render::{Limits, truncated};
use crate::sink::{Count, Sink};

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

/// Writes one message into a sink: the event, the limits it is rendered
/// under, and whether those limits cut it.
pub(crate) type Writer<'a> = dyn Fn(&mut dyn Sink, Limits, bool) + 'a;

/// Encodes `event` with `write` within `cap` bytes, cutting as section
/// 5.10 says. Every candidate is measured by writing it into a [`Count`];
/// the one buffer is allocated at the chosen message's exact size, after it
/// is known to fit.
pub(crate) fn fit(
    event: &Event,
    policy: Policy,
    cap: usize,
    write: &Writer<'_>,
) -> Result<Encoded, EncodeError> {
    let fits = |limits: Limits| {
        let truncated = truncated(event, policy, limits);
        let mut count = Count::default();
        write(&mut count, limits, truncated);
        (count.0 <= cap).then_some((limits, truncated, count.0))
    };
    let (limits, truncated, size) = choose(event, fits).ok_or_else(|| {
        let def = event.def();
        EncodeError::OverSizeLimit {
            stream: def.stream,
            id: def.id,
        }
    })?;
    let mut bytes = Vec::with_capacity(size);
    write(&mut bytes, limits, truncated);
    Ok(Encoded { bytes, truncated })
}

/// The limits of the message to send: the whole event if it fits; else the
/// largest server text rendering that fits with every field; else, with
/// optional fields dropped, the whole text or the largest rendering that
/// fits; else none.
fn choose<T>(event: &Event, fits: impl Fn(Limits) -> Option<T>) -> Option<T> {
    if let Some(whole) = fits(Limits::NONE) {
        return Some(whole);
    }
    let longest = longest_text(event);
    for drop_optional in [false, true] {
        if drop_optional {
            let whole_text = Limits {
                text: None,
                drop_optional,
            };
            if let Some(found) = fits(whole_text) {
                return Some(found);
            }
        }
        let limits = |text| Limits {
            text: Some(text),
            drop_optional,
        };
        let Some(mut best) = fits(limits(0)) else {
            continue;
        };
        // The largest length that fits lies in 0..longest: bisect, keeping
        // `low` a length that fits and `high` one past the last candidate.
        // The size grows with the length because an event has at most one
        // server text field (section 5.1, point 4).
        let mut low = 0_usize;
        let mut high = longest;
        while high.saturating_sub(low) > 1 {
            let middle = low + (high - low) / 2;
            if let Some(found) = fits(limits(middle)) {
                low = middle;
                best = found;
            } else {
                high = middle;
            }
        }
        return Some(best);
    }
    None
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
