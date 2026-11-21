// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The RFC 5424 encoding, the fallback when journald is unavailable
//! (`docs/specs/events.md` §5.9; ADR 0004, O8, points 1 and 4; ADR 0005,
//! point 1.7).
//!
//! A message is `<PRI>1 - HOSTNAME APP-NAME PROCID MSGID - MSG`, with no
//! timestamp (the local receiver stamps it), no structured data until a
//! Private Enterprise Number is assigned, and a printable-ASCII `MSG`.

use alloc::string::String;
use core::error::Error;
use core::fmt;

use crate::catalogue::Stream;
use crate::event::Event;
use crate::fit::{EncodeError, Encoded, fit};
use crate::redaction::Policy;
use crate::render::{Limits, is_text, push_decimal, write_message, write_value, written_fields};
use crate::sink::Sink;

#[cfg(test)]
mod tests;

/// The cap on a complete RFC 5424 message: the size RFC 5424, section 6.1,
/// requires every receiver to accept (ADR 0005, point 1.7).
pub const SYSLOG_FALLBACK_MAX: usize = 480;

/// The longest host name written; a longer one is written as the nil value
/// (`docs/specs/events.md` §5.9, point 3).
pub const HOSTNAME_MAX: usize = 64;

/// The longest application name, RFC 5424's `APP-NAME` limit.
pub const APP_NAME_MAX: usize = 48;

/// Facility 3, system daemons, for the diagnostic stream.
const FACILITY_DAEMON: u8 = 3;

/// Facility 13, log audit, for the audit stream.
const FACILITY_AUDIT: u8 = 13;

/// Why an origin was refused (`docs/specs/events.md` §7, point 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginError {
    /// The application name is empty.
    EmptyAppName,
    /// The application name is longer than [`APP_NAME_MAX`] bytes.
    AppNameTooLong,
    /// The application name holds a byte outside printable US-ASCII.
    AppNameNotPrintable,
}

impl fmt::Display for OriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyAppName => "the application name is empty",
            Self::AppNameTooLong => "the application name is longer than 48 bytes",
            Self::AppNameNotPrintable => "the application name is not printable US-ASCII",
        })
    }
}

impl Error for OriginError {}

/// An application name: 1 to 48 bytes of printable US-ASCII. It is
/// syslog's `APP-NAME` and journald's `SYSLOG_IDENTIFIER`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppName(String);

impl AppName {
    /// Checks `name`.
    ///
    /// # Errors
    ///
    /// An [`OriginError`] naming the rule broken.
    pub fn new(name: &str) -> Result<Self, OriginError> {
        if name.is_empty() {
            Err(OriginError::EmptyAppName)
        } else if name.len() > APP_NAME_MAX {
            Err(OriginError::AppNameTooLong)
        } else if !name.bytes().all(is_print_us_ascii) {
            Err(OriginError::AppNameNotPrintable)
        } else {
            Ok(Self(String::from(name)))
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// RFC 5424's `PRINTUSASCII`: `0x21` to `0x7E`.
const fn is_print_us_ascii(byte: u8) -> bool {
    matches!(byte, 0x21..=0x7e)
}

/// Who sends the messages: the header fields that do not change per event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    hostname: Option<String>,
    app_name: AppName,
    procid: Option<u32>,
}

impl Origin {
    /// The origin with this host name, application name and process id. A
    /// host name that is empty, longer than [`HOSTNAME_MAX`] bytes, or not
    /// printable US-ASCII is written as the nil value.
    #[must_use]
    pub fn new(hostname: &str, app_name: AppName, procid: Option<u32>) -> Self {
        let usable = !hostname.is_empty()
            && hostname.len() <= HOSTNAME_MAX
            && hostname.bytes().all(is_print_us_ascii);
        Self {
            hostname: usable.then(|| String::from(hostname)),
            app_name,
            procid,
        }
    }
}

/// Encodes events as RFC 5424 messages within a cap.
///
/// ```
/// use ironafs_events::syslog::{AppName, Origin, SyslogEncoder};
/// use ironafs_events::{Event, Policy, Role, catalogue};
///
/// let origin = Origin::new("client1", AppName::new("ironafsd")?, Some(42));
/// let encoder = SyslogEncoder::new(origin, Policy::Routine);
/// let event = Event::builder(&catalogue::EVENTS_LOST, Role::Daemon)?
///     .with("reason", "queue full")?
///     .with("count", 3_u64)?
///     .build()?;
/// let encoded = encoder.encode(&event)?;
/// assert_eq!(
///     encoded.bytes(),
///     b"<28>1 - client1 ironafsd 42 ironafs.diag.1 - event=events.lost v=1 \
///       subsystem=events audience=operational f_reason=\"queue full\" f_count=3 \
///       msg=\"3 events lost: queue full\""
/// );
/// # Ok::<(), Box<dyn core::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyslogEncoder {
    origin: Origin,
    policy: Policy,
    cap: usize,
}

impl SyslogEncoder {
    /// An encoder with the cap [`SYSLOG_FALLBACK_MAX`].
    #[must_use]
    pub fn new(origin: Origin, policy: Policy) -> Self {
        Self {
            origin,
            policy,
            cap: SYSLOG_FALLBACK_MAX,
        }
    }

    /// The same encoder with a lower cap. A cap above
    /// [`SYSLOG_FALLBACK_MAX`] is lowered to it.
    #[must_use]
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = cap.min(SYSLOG_FALLBACK_MAX);
        self
    }

    /// Encodes `event`, fitted to the cap.
    ///
    /// # Errors
    ///
    /// [`EncodeError::OverSizeLimit`] if the event's identity and required
    /// fields alone do not fit.
    pub fn encode(&self, event: &Event) -> Result<Encoded, EncodeError> {
        fit(event, self.policy, self.cap, &|sink, limits, truncated| {
            self.write(sink, event, limits, truncated);
        })
    }

    /// Writes the header, then IronAFS's pairs, then the message last, in
    /// quotes. Every value that can hold server text is escaped and quoted,
    /// and escaping leaves no bare `"`, so server text cannot end its value
    /// or imitate a pair (`docs/specs/events.md` §5.9, point 6).
    fn write(&self, out: &mut dyn Sink, event: &Event, limits: Limits, truncated: bool) {
        let mut out = out;
        let def = event.def();
        let facility = match def.stream {
            Stream::Diagnostic => FACILITY_DAEMON,
            Stream::Audit => FACILITY_AUDIT,
        };
        out.put(b"<");
        push_decimal(
            &mut out,
            u64::from(facility) * 8 + u64::from(def.severity.number()),
        );
        out.put(b">1 - ");
        out.put(self.origin.hostname.as_deref().unwrap_or("-").as_bytes());
        out.put(b" ");
        out.put(self.origin.app_name.as_str().as_bytes());
        out.put(b" ");
        match self.origin.procid {
            Some(procid) => push_decimal(&mut out, u64::from(procid)),
            None => out.put(b"-"),
        }
        out.put(match def.stream {
            Stream::Diagnostic => b" ironafs.diag.",
            Stream::Audit => b" ironafs.audit.",
        });
        push_decimal(&mut out, u64::from(def.id));
        out.put(b" - event=");
        out.put(def.name.as_bytes());
        out.put(b" v=");
        push_decimal(&mut out, u64::from(def.version));
        out.put(b" subsystem=");
        out.put(def.subsystem.name().as_bytes());
        out.put(b" audience=");
        out.put(def.audience.name().as_bytes());
        if event.is_direct() {
            out.put(b" direct=1");
        }
        if truncated {
            out.put(b" truncated=1");
        }
        for (spec, value) in written_fields(event, self.policy, limits) {
            out.put(b" f_");
            out.put(spec.name.as_bytes());
            out.put(b"=");
            if is_text(spec) {
                out.put(b"\"");
                write_value(&mut out, value, limits.text);
                out.put(b"\"");
            } else {
                write_value(&mut out, value, limits.text);
            }
        }
        out.put(b" msg=\"");
        write_message(&mut out, event, self.policy, limits);
        out.put(b"\"");
    }
}
