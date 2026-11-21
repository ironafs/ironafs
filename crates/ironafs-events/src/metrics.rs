// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Metric values and health (`docs/specs/events.md` §5.7; ADR 0004, O11,
//! points 1 and 6): saturating counters, fixed-bucket histograms, and a
//! health state with its reasons.

use core::error::Error;
use core::fmt;

#[cfg(test)]
mod tests;

/// A 64-bit count that stops at `u64::MAX`: a count is a clamped value
/// (coding standards rule 2), and a counter at its maximum still means "at
/// least this many".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Counter(u64);

impl Counter {
    /// A counter at zero.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Adds one.
    pub fn increment(&mut self) {
        self.add(1);
    }

    /// Adds `amount`, stopping at `u64::MAX`.
    pub fn add(&mut self, amount: u64) {
        self.0 = self.0.saturating_add(amount);
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the count and sets it to zero.
    pub fn take(&mut self) -> u64 {
        core::mem::take(&mut self.0)
    }
}

/// The most buckets a histogram has.
pub const HISTOGRAM_BUCKETS_MAX: usize = 32;

/// Why a histogram's bounds were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistogramError {
    /// No bounds, or more than [`HISTOGRAM_BUCKETS_MAX`].
    BucketCount,
    /// The bounds are not strictly increasing.
    NotIncreasing,
}

impl fmt::Display for HistogramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BucketCount => "a histogram has 1 to 32 buckets",
            Self::NotIncreasing => "a histogram's bounds must be strictly increasing",
        })
    }
}

impl Error for HistogramError {}

/// A histogram with `N` fixed upper bounds (ADR 0004, O11, point 1). A
/// value goes in the first bucket whose bound is at least the value, or in
/// the overflow count past the last bound.
///
/// ```
/// use ironafs_events::Histogram;
///
/// let mut latency = Histogram::new([1_000, 10_000, 100_000])?;
/// latency.observe(800);
/// latency.observe(10_000);
/// latency.observe(250_000);
/// assert_eq!(latency.bucket_counts(), [1, 1, 0]);
/// assert_eq!(latency.overflow(), 1);
/// assert_eq!(latency.count(), 3);
/// assert_eq!(latency.sum(), 260_800);
/// # Ok::<(), ironafs_events::HistogramError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Histogram<const N: usize> {
    bounds: [u64; N],
    buckets: [Counter; N],
    overflow: Counter,
    count: Counter,
    sum: Counter,
}

impl<const N: usize> Histogram<N> {
    /// A histogram with these upper bounds.
    ///
    /// # Errors
    ///
    /// [`HistogramError::BucketCount`] if `N` is 0 or above
    /// [`HISTOGRAM_BUCKETS_MAX`]; [`HistogramError::NotIncreasing`] if the
    /// bounds are not strictly increasing.
    pub fn new(bounds: [u64; N]) -> Result<Self, HistogramError> {
        if N == 0 || N > HISTOGRAM_BUCKETS_MAX {
            return Err(HistogramError::BucketCount);
        }
        if bounds
            .windows(2)
            .any(|pair| matches!(pair, [low, high] if low >= high))
        {
            return Err(HistogramError::NotIncreasing);
        }
        Ok(Self {
            bounds,
            buckets: [Counter::new(); N],
            overflow: Counter::new(),
            count: Counter::new(),
            sum: Counter::new(),
        })
    }

    /// Records `value`.
    pub fn observe(&mut self, value: u64) {
        let bucket = self
            .bounds
            .iter()
            .zip(self.buckets.iter_mut())
            .find_map(|(bound, bucket)| (value <= *bound).then_some(bucket));
        match bucket {
            Some(bucket) => bucket.increment(),
            None => self.overflow.increment(),
        }
        self.count.increment();
        self.sum.add(value);
    }

    /// The upper bounds.
    #[must_use]
    pub fn bounds(&self) -> [u64; N] {
        self.bounds
    }

    /// The count in each bucket (not cumulative).
    #[must_use]
    pub fn bucket_counts(&self) -> [u64; N] {
        self.buckets.map(Counter::get)
    }

    /// Values above the last bound.
    #[must_use]
    pub fn overflow(&self) -> u64 {
        self.overflow.get()
    }

    /// Values recorded.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.get()
    }

    /// The sum of the values recorded, stopping at `u64::MAX`.
    #[must_use]
    pub fn sum(&self) -> u64 {
        self.sum.get()
    }
}

/// The health levels, from best to worst (ADR 0004, O11, point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HealthLevel {
    /// Working normally.
    Healthy,
    /// Working, with reasons an operator should see.
    Degraded,
    /// Not working.
    Failed,
}

/// Why health is not healthy (`docs/specs/events.md` §5.7, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HealthReason {
    /// The audit sink is stalled (ADR 0004, O10, point 6).
    AuditSinkStalled,
    /// Durable audit admission is unavailable (ADR 0004, O10, point 10).
    AuditAdmissionUnavailable,
    /// A log sink is stalled.
    LogSinkStalled,
    /// Events were lost.
    EventsLost,
    /// The daemon's state cannot be used.
    StateUnusable,
}

impl HealthReason {
    /// Every reason, in order.
    pub const ALL: [Self; 5] = [
        Self::AuditSinkStalled,
        Self::AuditAdmissionUnavailable,
        Self::LogSinkStalled,
        Self::EventsLost,
        Self::StateUnusable,
    ];

    /// The level this reason sets.
    #[must_use]
    pub const fn level(self) -> HealthLevel {
        match self {
            Self::AuditSinkStalled
            | Self::AuditAdmissionUnavailable
            | Self::LogSinkStalled
            | Self::EventsLost => HealthLevel::Degraded,
            Self::StateUnusable => HealthLevel::Failed,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::AuditSinkStalled => 1,
            Self::AuditAdmissionUnavailable => 1 << 1,
            Self::LogSinkStalled => 1 << 2,
            Self::EventsLost => 1 << 3,
            Self::StateUnusable => 1 << 4,
        }
    }
}

/// Health: a set of reasons, whose worst level is the health's level, and
/// healthy with none.
///
/// ```
/// use ironafs_events::{Health, HealthLevel, HealthReason};
///
/// let mut health = Health::default();
/// health.raise(HealthReason::AuditSinkStalled);
/// assert_eq!(health.level(), HealthLevel::Degraded);
/// health.clear(HealthReason::AuditSinkStalled);
/// assert_eq!(health.level(), HealthLevel::Healthy);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Health(u8);

impl Health {
    /// Adds `reason`.
    pub fn raise(&mut self, reason: HealthReason) {
        self.0 |= reason.bit();
    }

    /// Removes `reason`.
    pub fn clear(&mut self, reason: HealthReason) {
        self.0 &= !reason.bit();
    }

    /// Whether `reason` is present.
    #[must_use]
    pub const fn has(self, reason: HealthReason) -> bool {
        self.0 & reason.bit() != 0
    }

    /// The worst level of the reasons present, or healthy.
    #[must_use]
    pub fn level(self) -> HealthLevel {
        self.reasons()
            .map(HealthReason::level)
            .max()
            .unwrap_or(HealthLevel::Healthy)
    }

    /// The reasons present, in [`HealthReason::ALL`]'s order.
    pub fn reasons(self) -> impl Iterator<Item = HealthReason> {
        HealthReason::ALL
            .into_iter()
            .filter(move |reason| self.has(*reason))
    }
}
