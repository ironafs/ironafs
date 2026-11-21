// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fence (ADR 0004, C16, point 2; C25, point 3; C32, point 5; callback
//! specification, 5.6, points 1 to 4): the values a validating or fetching
//! call records when it is sent, and the one check that compares them with
//! the current ones where its reply is installed and where its entry is
//! served.
//!
//! - **The tuple** of an object of server key K, in one context, has six
//!   values: the object's break counter and its volume's break counter
//!   (the [`FenceTable`]'s), K's suspect epoch (taken from
//!   [`EpochSource`]), the pause generation of K's family's socket (Rx's
//!   driver's), the resume generation (the table's), and the context's
//!   local volume epoch for the object's volume (the incarnation
//!   specification's, 5.6, point 3, step 3, which the table keeps a copy
//!   of). A reply or an entry is good only if every value is unchanged.
//! - **What is not in it.** The credential and rights generations are the
//!   request's ticket's, checked at submission (C16, point 3; the contract
//!   specification, 5.9, point 2), not here. The credential generation is
//!   also part of the [`ContextKey`](crate::ContextKey) the table is keyed
//!   by, so a check made with the context's current key after a credential
//!   change finds no record and fails. There is no overload counter: the
//!   seams specification's overload record (its 5.4, point 7) is replaced
//!   by Rx holding what it cannot admit until the callback service has
//!   advanced the affected suspect epochs at its tick (callback
//!   specification, 5.6, point 7; Rx specification, 5.10, point 5), and by
//!   the pause generation where Rx cannot hold (Rx specification, 5.10,
//!   point 6). Both reach this tuple.
//! - **Values never repeat.** The table's counters and the suspect epoch
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

mod table;

pub use table::{
    FenceError, FenceLimits, FenceTable, MAX_FENCE_BYTES, MIN_FENCE_BYTES, SLOT_BYTES,
};

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

/// One value of a fence counter: 1 to `u64::MAX`, or [`Stamp::NONE`].
/// Only this crate makes one, from a sequence that hands out each value
/// once. Values are unique within the one table or source that issued them:
/// every table and every source starts at 1, so values from two of them
/// are never compared (the daemon keeps one [`FenceTable`] and one
/// [`EpochSource`] for its whole life).
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

/// A socket's pause generation, as the [`FenceTable`] loads it from Rx's
/// driver, sequentially consistent, after its clock comparison (Rx
/// specification, 5.12, point 5; callback specification, 5.6, point 2). The
/// driver starts it at 0 and only raises it, so a value is never seen again
/// on that socket while the daemon runs; a socket bound again is a new
/// server key (the local endpoint generation is part of the key), not a
/// reset. It is compared by plain equality, 0 included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PauseGeneration(pub u64);

/// A context's local volume epoch (incarnation specification, 5.1.2; N38,
/// point 8), taken by the incarnation code from its daemon-wide counter,
/// which never repeats a value. The [`FenceTable`] keeps a copy per context
/// and volume, written by [`FenceTable::set_volume_epoch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VolumeEpoch(pub u64);

/// What a fence tuple takes from one server key: its suspect epoch, read
/// from the callback state record, and the driver's pause generation of
/// the key's family's socket, which the [`FenceTable`] itself loads at the
/// right point of a recording or a check, never a value sampled earlier.
#[derive(Clone, Copy, Debug)]
pub struct Server<'a> {
    /// The server key's suspect epoch.
    pub suspect_epoch: Stamp,
    /// The driver's pause generation of the key's family's socket.
    pub pause: &'a AtomicU64,
}

/// The C37 clocks' detector of suspends and jumps (the keepalive
/// specification's rule and baseline), owned by the [`FenceTable`] so that
/// the comparison, the move of its baseline and the raise of the resume
/// generation are one step under the table's lock (callback specification,
/// 5.6, point 2).
pub trait ClockWatch {
    /// Compares the clocks with the baseline. Returns `true`, and moves the
    /// baseline to the current clocks, if they show a suspend or a jump not
    /// seen before; a clock that cannot be read counts as a move.
    fn moved(&mut self) -> bool;
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
    /// The context's local volume epoch, or `None` if the table held none
    /// when the tuple was recorded (see [`FenceTable::adopt_volume_epoch`]).
    pub volume_epoch: Option<VolumeEpoch>,
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
            volume_epoch: !(self.volume_epoch.is_some()
                && current.volume_epoch == self.volume_epoch),
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
    /// The local volume epoch.
    pub volume_epoch: bool,
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
