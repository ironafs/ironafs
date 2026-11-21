// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fence (ADR 0004, C16, point 2; callback specification, 5.6,
//! points 1 to 4): the values a validating or fetching call records when
//! it is sent, and the one check that compares them with the current ones
//! where its reply is installed and where its entry is served.
//!
//! - **The tuple** of an object of server key K, in one context: the
//!   object's break counter, its volume's break counter (both
//!   [`BreakCounters`]), K's suspect epoch (taken from [`EpochSource`]),
//!   the pause generation of K's family's socket (Rx's driver's) and the
//!   [`ResumeGeneration`]. A reply or an entry is good only if every value
//!   is unchanged.
//! - **Values never repeat.** The cache's counters and the suspect epoch
//!   are [`Stamp`]s, each taken once from a sequence that only rises and
//!   never wraps, so a value names one record at one moment: a retired
//!   server record, or an evicted counter record, made again never matches
//!   a tuple recorded under the old one (callback specification, 5.6,
//!   point 1). [`Stamp::NONE`] stands for an absent or exhausted value and
//!   matches nothing, so exhaustion fails every fence (section 6, point 3).
//! - **What a failure says.** [`Suspicion`] names the values that changed;
//!   a change in the object's own counter alone is narrow, any other is
//!   wide (incarnation specification, section 4), and the two are
//!   revalidated differently (its 5.6, point 6).

mod breaks;

pub use breaks::{BreakCounters, BreakLimits, FenceError};

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

/// One value of a fence counter: 1 to `u64::MAX`, or [`Stamp::NONE`].
/// Only this crate makes one, from a sequence that hands out each value
/// once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp(u64);

impl Stamp {
    /// No value: absent, or exhausted. It matches nothing, itself
    /// included.
    pub const NONE: Self = Self(0);

    /// The value as a number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Whether the current value `self` equals `recorded`, neither being
    /// [`Stamp::NONE`].
    #[must_use]
    pub fn matches(self, recorded: Self) -> bool {
        self.0 != 0 && self.0 == recorded.0
    }
}

/// The client-wide counter suspect epochs are taken from (callback
/// specification, 5.6, point 1): every creation and every advance of a
/// callback state record takes its next value, so no value is used twice by
/// any record.
///
/// ```
/// use ironafs_cache::EpochSource;
///
/// let source = EpochSource::new();
/// let first = source.take().unwrap();
/// let second = source.take().unwrap();
/// assert!(!second.matches(first));
/// ```
#[derive(Debug)]
pub struct EpochSource {
    next: AtomicU64,
}

impl EpochSource {
    /// A source whose first value is 1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// The next value, by one sequentially consistent atomic
    /// read-modify-write that rises by 1 and never wraps: each of 1 to
    /// `u64::MAX` is returned once, then `None` forever (section 6, point
    /// 3). A `None` for a new record is the callback specification's
    /// `EpochExhausted`: no call is started; for an advance, the key is
    /// served as if every entry were suspect.
    pub fn take(&self) -> Option<Stamp> {
        self.next
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |next| {
                (next != 0).then(|| next.wrapping_add(1))
            })
            .ok()
            .map(Stamp)
    }
}

impl Default for EpochSource {
    fn default() -> Self {
        Self::new()
    }
}

/// The client-wide resume generation (callback specification, 5.6, point
/// 2): raised by the fence check when the C37 clocks show a suspend or a
/// jump not seen before, so every tuple recorded before it fails.
#[derive(Debug)]
pub struct ResumeGeneration {
    value: AtomicU64,
}

impl ResumeGeneration {
    /// A generation whose first value is 1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(1),
        }
    }

    /// The current value; [`Stamp::NONE`] once exhausted.
    #[must_use]
    pub fn current(&self) -> Stamp {
        Stamp(self.value.load(Ordering::SeqCst))
    }

    /// Raises the generation by 1, atomically. A raise past `u64::MAX`
    /// exhausts it: from then on [`ResumeGeneration::current`] is
    /// [`Stamp::NONE`], which fails every fence.
    pub fn raise(&self) {
        // Past `u64::MAX` the value wraps to 0, which stays: exhausted.
        let _ = self
            .value
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value != 0).then(|| value.wrapping_add(1))
            });
    }
}

impl Default for ResumeGeneration {
    fn default() -> Self {
        Self::new()
    }
}

/// A socket's pause generation, read from Rx's driver (Rx specification,
/// 5.12, point 5). It starts at 0 and is compared by plain equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PauseGeneration(pub u64);

/// The current values of a fence tuple shared by every object of one
/// server key: read from the callback state record and the driver when a
/// tuple is recorded or checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerValues {
    /// The server key's suspect epoch.
    pub suspect_epoch: Stamp,
    /// The pause generation of the key's family's socket.
    pub pause: PauseGeneration,
}

/// What the comparison of the C37 clocks found, which the fence check acts
/// on first (C32, point 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clocks {
    /// No suspend or jump not seen before.
    Steady,
    /// A suspend or a jump not seen before.
    Moved,
}

/// The fence tuple of one object in one context, as a call recorded it
/// when it was sent and its entry keeps it (callback specification, 5.6,
/// point 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FenceTuple {
    /// The object's break counter.
    pub object: Stamp,
    /// The volume's break counter.
    pub volume: Stamp,
    /// The server key's suspect epoch.
    pub suspect_epoch: Stamp,
    /// The pause generation.
    pub pause: PauseGeneration,
    /// The resume generation.
    pub resume: Stamp,
}

impl FenceTuple {
    /// Which values of `current` differ from this recorded tuple. An
    /// absent value on either side counts as changed.
    #[must_use]
    pub fn compare(&self, current: &Self) -> Suspicion {
        Suspicion {
            object: !current.object.matches(self.object),
            volume: !current.volume.matches(self.volume),
            suspect_epoch: !current.suspect_epoch.matches(self.suspect_epoch),
            pause: current.pause != self.pause,
            resume: !current.resume.matches(self.resume),
        }
    }
}

/// Which values of a fence tuple changed since it was recorded. Any
/// change fails the fence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per value of the fence tuple"
)]
pub struct Suspicion {
    /// The object's break counter.
    pub object: bool,
    /// The volume's break counter.
    pub volume: bool,
    /// The suspect epoch.
    pub suspect_epoch: bool,
    /// The pause generation.
    pub pause: bool,
    /// The resume generation.
    pub resume: bool,
}

impl Suspicion {
    /// Whether nothing changed: the fence holds.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        *self == Self::default()
    }

    /// Whether only the object's own break counter changed: a narrow
    /// suspicion (incarnation specification, section 4).
    #[must_use]
    pub fn is_narrow(&self) -> bool {
        *self
            == Self {
                object: true,
                ..Self::default()
            }
    }
}
