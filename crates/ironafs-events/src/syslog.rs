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
use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use crate::catalogue::Stream;
use crate::event::Event;
use crate::fit::{EncodeError, Encoded, fit};
use crate::redaction::Policy;
use crate::render::{Rendered, push_decimal};

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
///     b"<28>1 - client1 ironafsd 42 ironafs.diag.1 - 3 events lost: queue full \
///       event=events.lost v=1 subsystem=events audience=operational \
///       f_reason=\"queue full\" f_count=3"
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
        fit(event, self.policy, self.cap, |rendered| {
            self.write(event, rendered)
        })
    }

    fn write(&self, event: &Event, rendered: &Rendered) -> Vec<u8> {
        let def = event.def();
        let facility = match def.stream {
            Stream::Diagnostic => FACILITY_DAEMON,
            Stream::Audit => FACILITY_AUDIT,
        };
        let mut out = String::new();
        out.push('<');
        push_decimal(
            &mut out,
            u64::from(facility) * 8 + u64::from(def.severity.number()),
        );
        out.push_str(">1 - ");
        out.push_str(self.origin.hostname.as_deref().unwrap_or("-"));
        out.push(' ');
        out.push_str(self.origin.app_name.as_str());
        out.push(' ');
        match self.origin.procid {
            Some(procid) => push_decimal(&mut out, u64::from(procid)),
            None => out.push('-'),
        }
        out.push_str(match def.stream {
            Stream::Diagnostic => " ironafs.diag.",
            Stream::Audit => " ironafs.audit.",
        });
        push_decimal(&mut out, u64::from(def.id));
        out.push_str(" - ");
        out.push_str(&rendered.message);
        out.push_str(" event=");
        out.push_str(def.name);
        out.push_str(" v=");
        push_decimal(&mut out, u64::from(def.version));
        out.push_str(" subsystem=");
        out.push_str(def.subsystem.name());
        out.push_str(" audience=");
        out.push_str(def.audience.name());
        if event.is_direct() {
            out.push_str(" direct=1");
        }
        if rendered.truncated {
            out.push_str(" truncated=1");
        }
        for field in &rendered.fields {
            out.push_str(" f_");
            out.push_str(field.name);
            out.push('=');
            if field.text {
                out.push('"');
                out.push_str(&field.value);
                out.push('"');
            } else {
                out.push_str(&field.value);
            }
        }
        out.into_bytes()
    }
}
