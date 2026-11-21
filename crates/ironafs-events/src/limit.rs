// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rate limiting per subsystem and loss counters (`docs/specs/events.md`
//! §5.6; ADR 0004, O3, point 3, and O7, points 1 to 3).
//!
//! Every event the limiter refuses is counted, and every loss a caller
//! sees is counted here, so no event is lost without a count. The
//! limiter takes the instant as an argument: this crate reads no clock.

use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use crate::catalogue::{self, EventDef, Role, Stream, Subsystem};
use crate::event::{Event, EventError};
use crate::metrics::Counter;

#[cfg(test)]
mod tests;

/// The largest rate (events per second) and burst a limit takes.
pub const LIMIT_MAX: u32 = 1_000_000;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Why a limit was refused (`docs/specs/events.md` §7, point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitError {
    /// The rate is 0 or above [`LIMIT_MAX`].
    Rate,
    /// The burst is 0 or above [`LIMIT_MAX`].
    Burst,
}

impl fmt::Display for LimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Rate => "the rate must be 1 to 1000000 events per second",
            Self::Burst => "the burst must be 1 to 1000000 events",
        })
    }
}

impl Error for LimitError {}

/// A subsystem's limit: a rate in events per second and a burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LimitConfig {
    rate: u32,
    burst: u32,
}

impl LimitConfig {
    /// The limit of `rate` events per second, with bursts of `burst`.
    ///
    /// # Errors
    ///
    /// [`LimitError::Rate`] or [`LimitError::Burst`] for a value of 0 or
    /// above [`LIMIT_MAX`].
    pub const fn new(rate: u32, burst: u32) -> Result<Self, LimitError> {
        if rate == 0 || rate > LIMIT_MAX {
            Err(LimitError::Rate)
        } else if burst == 0 || burst > LIMIT_MAX {
            Err(LimitError::Burst)
        } else {
            Ok(Self { rate, burst })
        }
    }
}

impl Default for LimitConfig {
    /// 10 events per second, with bursts of 50.
    fn default() -> Self {
        Self {
            rate: 10,
            burst: 50,
        }
    }
}

/// Whether the limiter let an event through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// Send it.
    Admitted,
    /// Do not send it; it was counted.
    Suppressed,
}

/// One bucket per subsystem (`docs/specs/events.md` §5.6, point 3).
///
/// ```
/// use ironafs_events::{Admission, LimitConfig, RateLimiter, catalogue};
///
/// let mut limiter = RateLimiter::new(LimitConfig::new(1, 2)?);
/// let def = &catalogue::SERVER_UNREACHABLE;
/// assert_eq!(limiter.admit(def, 0), Admission::Admitted);
/// assert_eq!(limiter.admit(def, 0), Admission::Admitted);
/// assert_eq!(limiter.admit(def, 0), Admission::Suppressed);
/// // One second later one more event fits.
/// assert_eq!(limiter.admit(def, 1_000_000_000), Admission::Admitted);
/// assert_eq!(limiter.suppressed(def.subsystem()), 1);
/// # Ok::<(), ironafs_events::LimitError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimiter {
    interval: u64,
    tolerance: u64,
    arrival: [u64; Subsystem::COUNT],
    suppressed: [Counter; Subsystem::COUNT],
}

impl RateLimiter {
    /// A limiter with `config` for every subsystem.
    #[must_use]
    pub fn new(config: LimitConfig) -> Self {
        // `rate` is 1 to 1,000,000, so the interval is 1,000 to 10^9
        // nanoseconds, and the tolerance at most 10^9 * (10^6 - 1), well
        // within 64 bits. Rounding up keeps the admitted rate at or below
        // the configured one.
        let interval = NANOS_PER_SECOND.div_ceil(u64::from(config.rate)).max(1);
        let tolerance = interval.saturating_mul(u64::from(config.burst).saturating_sub(1));
        Self {
            interval,
            tolerance,
            arrival: [0; Subsystem::COUNT],
            suppressed: [Counter::new(); Subsystem::COUNT],
        }
    }

    /// Decides whether an event of `def` is sent at `now_nanos`: the boot
    /// clock's reading in nanoseconds, from the `Clock` seam. Audit is
    /// always admitted and touches no bucket (ADR 0004, O9, point 4). A
    /// refusal is counted against the subsystem.
    pub fn admit(&mut self, def: &EventDef, now_nanos: u64) -> Admission {
        if def.stream == Stream::Audit {
            return Admission::Admitted;
        }
        let index = def.subsystem.index();
        let (Some(arrival), Some(suppressed)) =
            (self.arrival.get_mut(index), self.suppressed.get_mut(index))
        else {
            return Admission::Suppressed;
        };
        let next = now_nanos
            .checked_add(self.tolerance)
            .filter(|limit| *limit >= *arrival)
            .and_then(|_| (*arrival).max(now_nanos).checked_add(self.interval));
        if let Some(next) = next {
            *arrival = next;
            Admission::Admitted
        } else {
            suppressed.increment();
            Admission::Suppressed
        }
    }

    /// The events of `subsystem` refused since the last
    /// [`take_suppressed`](Self::take_suppressed).
    #[must_use]
    pub fn suppressed(&self, subsystem: Subsystem) -> u64 {
        self.suppressed
            .get(subsystem.index())
            .map_or(0, |counter| counter.get())
    }

    /// One `events.suppressed` event per subsystem with refusals, emitted
    /// by `role`; the counts are cleared (ADR 0004, O7, point 1).
    ///
    /// # Errors
    ///
    /// None for any role: every role may emit the summary. The counts are
    /// cleared only when every event is built.
    pub fn take_suppressed(&mut self, role: Role) -> Result<Vec<Event>, EventError> {
        let mut events = Vec::new();
        for subsystem in Subsystem::ALL {
            let count = self.suppressed(subsystem);
            if count > 0 {
                events.push(
                    Event::builder(&catalogue::EVENTS_SUPPRESSED, role)?
                        .with("subsystem", subsystem.name())?
                        .with("count", count)?
                        .build()?,
                );
            }
        }
        for counter in &mut self.suppressed {
            counter.take();
        }
        Ok(events)
    }
}

/// Why events were not delivered, or were delivered cut
/// (`docs/specs/events.md` §5.6, point 6). Rate-limit refusals are counted
/// by the [`RateLimiter`] and summarised as `events.suppressed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Loss {
    /// The identity and required fields did not fit the sink's cap.
    OverSizeLimit,
    /// A bounded queue was full.
    QueueFull,
    /// The sink did not take the message within its bound.
    SinkStalled,
    /// The sink was not there.
    SinkUnavailable,
    /// Delivered, with server text cut or optional fields dropped.
    Truncated,
}

impl Loss {
    /// Every reason, in order.
    pub const ALL: [Self; 5] = [
        Self::OverSizeLimit,
        Self::QueueFull,
        Self::SinkStalled,
        Self::SinkUnavailable,
        Self::Truncated,
    ];

    /// The reason as `events.lost` writes it.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::OverSizeLimit => "over the size limit",
            Self::QueueFull => "queue full",
            Self::SinkStalled => "sink stalled",
            Self::SinkUnavailable => "sink unavailable",
            Self::Truncated => "truncated, delivered in part",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::OverSizeLimit => 0,
            Self::QueueFull => 1,
            Self::SinkStalled => 2,
            Self::SinkUnavailable => 3,
            Self::Truncated => 4,
        }
    }
}

/// A count per [`Loss`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LossCounters([Counter; Loss::ALL.len()]);

impl LossCounters {
    /// Counts one event lost for `loss`.
    pub fn record(&mut self, loss: Loss) {
        if let Some(counter) = self.0.get_mut(loss.index()) {
            counter.increment();
        }
    }

    /// The count for `loss` since the last [`take_lost`](Self::take_lost).
    #[must_use]
    pub fn get(&self, loss: Loss) -> u64 {
        self.0.get(loss.index()).map_or(0, |counter| counter.get())
    }

    /// One `events.lost` event per reason with a count, emitted by `role`;
    /// the counts are cleared (ADR 0004, O3, point 3).
    ///
    /// # Errors
    ///
    /// None for any role: every role may emit it. The counts are cleared
    /// only when every event is built.
    pub fn take_lost(&mut self, role: Role) -> Result<Vec<Event>, EventError> {
        let mut events = Vec::new();
        for loss in Loss::ALL {
            let count = self.get(loss);
            if count > 0 {
                events.push(
                    Event::builder(&catalogue::EVENTS_LOST, role)?
                        .with("reason", loss.reason())?
                        .with("count", count)?
                        .build()?,
                );
            }
        }
        for counter in &mut self.0 {
            counter.take();
        }
        Ok(events)
    }
}
