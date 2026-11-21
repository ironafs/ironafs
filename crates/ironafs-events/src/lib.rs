// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's event catalogue, redaction, rate limiting, metrics and health,
//! and the encodings its log sinks carry (ADR 0004, O1 to O8 and O11; C4,
//! point 1). The specification is `docs/specs/events.md`.
//!
//! Every component reports through events from the one [`catalogue`]:
//! never free-form text. An [`Event`] is built from a definition, for a
//! producer [`Role`] the definition allows, with typed field values. A
//! value can come only from the closed list of types that implement
//! [`FieldSource`], so a token or a key cannot become a field, and text a
//! server sent arrives only as [`ServerText`], which renders escaped.
//!
//! The encoders write a complete message for a sink, the journald native
//! protocol ([`journald`]) or RFC 5424 syslog ([`syslog`]), redacted under
//! a [`Policy`], and fit it to the sink's cap: server text is cut first,
//! then optional fields are dropped, and the result says so. An event whose
//! identity and required fields alone do not fit is refused with
//! [`EncodeError::OverSizeLimit`], and the caller sends
//! [`Event::over_size`] in its place.
//!
//! The crate is pure: no I/O, no clock and no dependencies. The
//! [`RateLimiter`] takes the instant as an argument, and the transport that
//! carries an encoded message to the operating system lives in the
//! operating-system bindings.

#![no_std]

extern crate alloc;

pub mod catalogue;
mod event;
mod field;
mod fit;
pub mod journald;
mod limit;
mod metrics;
mod redaction;
mod render;
mod sink;
pub mod syslog;

pub use catalogue::{
    Audience, CATALOGUE_VERSION, DataClass, EventDef, FieldKind, FieldSpec, Role, Roles, Severity,
    Stream, Subsystem,
};
pub use event::{Event, EventBuilder, EventError};
pub use field::{
    FieldSource, REDACTION_MARKER, Reference, SERVER_TEXT_MAX, ServerText, TRUNCATION_MARKER,
    write_redacted,
};
pub use fit::{EncodeError, Encoded};
pub use limit::{Admission, LIMIT_MAX, LimitConfig, LimitError, Loss, LossCounters, RateLimiter};
pub use metrics::{
    Counter, HISTOGRAM_BUCKETS_MAX, Health, HealthLevel, HealthReason, Histogram, HistogramError,
};
pub use redaction::Policy;
