// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fence table: each context's object and volume break counters and
//! local volume epochs, the resume generation and the clock watch that
//! raises it (ADR 0004, C16, points 1 and 2; C25, point 3; C32, point 5;
//! callback specification, 5.6, points 2 and 9, rows B1 and B2;
//! incarnation specification, 5.6, points 3 and 4).
//!
//! - **One lock.** Every operation takes the table by `&mut self` or
//!   `&self`, so the daemon keeps it behind one lock, and every recording,
//!   raise and check is made under it. [`FenceTable::check`] takes
//!   `&mut self`: a serve and a submission cannot check without the lock,
//!   and the clock comparison, the resume generation's raise and the
//!   comparison of the six values are one step no other caller can come
//!   between. The lock is a leaf in C39, point 4's order: taken after any
//!   other lock the caller holds, held for one operation, and no lock is
//!   taken while it is held. A break or an
//!   epoch change is recorded under it before the caller queues any event
//!   (C16, point 1), so every later check sees it. The pause generation is
//!   the one value read outside it: the caller reads it from the driver
//!   with a sequentially consistent load (Rx specification, 5.12, point 5)
//!   and passes it in [`ServerValues`]; the suspect epoch is passed the
//!   same way, from the callback state record.
//! - **Records.** A context holds an object's counter from the first fence
//!   tuple it records for the object, and its volume's likewise. A break
//!   raises the counter in every context that holds it, and creates
//!   nothing in a context that does not: such a context has recorded no
//!   tuple the break could need to fence.
//! - **Values.** Every counter value, made or raised, of either kind, is
//!   taken once from one sequence that only rises. So a recorded value
//!   equals a current one only if that record has existed, unbroken, since
//!   the tuple was recorded: a record evicted and made again, or broken, has
//!   a value no recorded tuple holds.
//! - **The local volume epoch** is a copy, in the context's volume record,
//!   of the epoch of the context's evidence record for the volume. The
//!   incarnation code writes it with [`FenceTable::set_volume_epoch`] in the
//!   same step, under the volume's lock, in which it sets or changes the
//!   evidence record's epoch (incarnation specification, 5.6, points 1 and
//!   3), taking this table's lock inside the volume's. A copy, rather than
//!   an atomic per volume read outside the lock, keeps the six values one
//!   locked read, and keeps the table's memory in its budget. A volume
//!   record evicted loses the copy: tuples recorded under it fail, and new
//!   ones carry none until the next write.
//! - **A tuple recorded with no epoch** fails its check, except at the
//!   install whose own observation wrote the epoch: there the incarnation
//!   code calls [`FenceTable::adopt_volume_epoch`], which takes the epoch
//!   into the tuple if the volume record has existed unbroken since the
//!   tuple was recorded. Such a fetch was sent before any epoch existed in
//!   the context, so it carries no older epoch's view, and its reply has
//!   just been compared with the evidence record (incarnation
//!   specification, 5.4, point 2; 5.6, point 1); an epoch change between
//!   its send and its install is ambiguity, which raises the volume's
//!   counter (its 5.6, point 3, step 2), so it fails anyway. If the
//!   adoption is refused, the fence fails and the fetch is retried within
//!   C16, point 2's bound, and the retry records the current epoch.
//! - **Bounds, in bytes** (coding standards, rule 3). The table holds
//!   records only while their charge, [`OBJECT_RECORD_BYTES`] or
//!   [`VOLUME_RECORD_BYTES`] each, fits in [`FenceLimits::bytes`], at most
//!   [`MAX_FENCE_BYTES`]. A new record that would not fit evicts the
//!   records with the smallest values, of either kind, those made or raised
//!   longest ago `[OWN]`, never one the operation itself uses. Eviction is
//!   always safe: it only makes the tuples recorded under the evicted record
//!   fail their fence (a revalidation, never a stale serve).
//! - **Exhaustion.** When the sequence cannot give the values an operation
//!   needs, or the resume generation passes `u64::MAX`, the table is
//!   exhausted: every check fails on both counters (or on the resume
//!   generation), and every new tuple is refused with
//!   [`FenceError::Exhausted`], so nothing is sent under a tuple that can
//!   never match. This is the callback specification's rule for the suspect
//!   epoch (section 6, point 3) applied to these values `[OWN]`; at one
//!   value per nanosecond the sequence lasts over 500 years.
//! - **Refusals change nothing** but that latch: an operation checks that
//!   it can complete before it evicts or makes anything. A clock move found
//!   on the way is an event, recorded first, and stands.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;

use ironafs_vfs::{ContextId, CredentialGeneration};

use super::{ClockWatch, FenceTuple, ServerValues, Stamp, Suspicion, VolumeEpoch};
use crate::{CellIndex, ContextKey, FileId, VolumeId};

#[cfg(test)]
mod tests;

/// The bytes charged for one object record: three times the entries it
/// takes in the table's two ordered maps `[OWN]`. A B-tree node holds at
/// most 11 entries and, except the root, at least 5, so a node's allocation
/// per entry is under 2.4 times the entry's size, and the internal nodes
/// add under a fifth again.
pub const OBJECT_RECORD_BYTES: usize =
    3 * (size_of::<((FileId, ContextKey), u64)>() + size_of::<(u64, Slot)>());

/// The bytes charged for one volume record, by the same rule as
/// [`OBJECT_RECORD_BYTES`].
pub const VOLUME_RECORD_BYTES: usize =
    3 * (size_of::<(VolumeKey, VolumeRecord)>() + size_of::<(u64, Slot)>());

/// The smallest budget: one record of each kind, what one recording needs.
pub const MIN_FENCE_BYTES: usize = OBJECT_RECORD_BYTES + VOLUME_RECORD_BYTES;

/// The largest budget: 256 MiB `[OWN]`, about a million records, far above
/// the objects one machine's contexts fetch between evictions.
pub const MAX_FENCE_BYTES: usize = 256 << 20;

/// The table's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FenceLimits {
    /// The bytes the records may be charged: [`MIN_FENCE_BYTES`] to
    /// [`MAX_FENCE_BYTES`].
    pub bytes: usize,
}

/// Why the table refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FenceError {
    /// No counter value, or no resume generation, is left: nothing may be
    /// sent under a new tuple.
    Exhausted,
    /// Limits out of range.
    InvalidLimits,
}

impl fmt::Display for FenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exhausted => "fence values exhausted",
            Self::InvalidLimits => "limits out of range",
        })
    }
}

impl core::error::Error for FenceError {}

/// A volume record's key: ordered so that one volume's records, in every
/// context, are one range.
type VolumeKey = (CellIndex, VolumeId, ContextKey);

#[derive(Clone, Copy, Debug)]
struct VolumeRecord {
    stamp: u64,
    epoch: Option<VolumeEpoch>,
}

/// A record, as the eviction order names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Object(FileId, ContextKey),
    Volume(VolumeKey),
}

const FIRST_CONTEXT: ContextKey = ContextKey {
    id: ContextId(0),
    generation: CredentialGeneration(0),
};

const LAST_CONTEXT: ContextKey = ContextKey {
    id: ContextId(u64::MAX),
    generation: CredentialGeneration(u64::MAX),
};

/// The fence table. `W` is the clock watch it consults before every
/// recording and every check.
#[derive(Debug)]
pub struct FenceTable<W> {
    limits: FenceLimits,
    /// The next counter value; `None` once `u64::MAX` is taken.
    next: Option<u64>,
    /// Set when an operation found too few values left.
    exhausted: bool,
    /// The resume generation; `None` once raised past `u64::MAX`.
    resume: Option<u64>,
    watch: W,
    objects: BTreeMap<(FileId, ContextKey), u64>,
    volumes: BTreeMap<VolumeKey, VolumeRecord>,
    /// Every record by its value, for eviction. Values are unique.
    order: BTreeMap<u64, Slot>,
    /// The bytes charged for the records held.
    bytes: usize,
}

/// Takes the next value of the sequence.
fn take(next: &mut Option<u64>) -> Option<u64> {
    let value = (*next)?;
    *next = value.checked_add(1);
    Some(value)
}

const fn cost(slot: &Slot) -> usize {
    match slot {
        Slot::Object(..) => OBJECT_RECORD_BYTES,
        Slot::Volume(..) => VOLUME_RECORD_BYTES,
    }
}

impl<W: ClockWatch> FenceTable<W> {
    /// An empty table, with the resume generation at 1.
    ///
    /// # Errors
    ///
    /// [`FenceError::InvalidLimits`] if the budget is out of range.
    pub fn new(limits: FenceLimits, watch: W) -> Result<Self, FenceError> {
        if !(MIN_FENCE_BYTES..=MAX_FENCE_BYTES).contains(&limits.bytes) {
            return Err(FenceError::InvalidLimits);
        }
        Ok(Self {
            limits,
            next: Some(1),
            exhausted: false,
            resume: Some(1),
            watch,
            objects: BTreeMap::new(),
            volumes: BTreeMap::new(),
            order: BTreeMap::new(),
            bytes: 0,
        })
    }

    /// The fence tuple of `file` in `context`, recorded by a call that can
    /// validate an entry or grant a promise, when it is sent (callback
    /// specification, 5.6, point 3). `context` is the request's current
    /// context and credential generation. Consults the clock watch first,
    /// then makes the context's object and volume records if absent.
    ///
    /// # Errors
    ///
    /// [`FenceError::Exhausted`] if the table is exhausted, or the values
    /// the absent records need are not left (which exhausts it): the call
    /// is not sent. Nothing else changes.
    pub fn record(
        &mut self,
        context: ContextKey,
        file: FileId,
        server: ServerValues,
    ) -> Result<FenceTuple, FenceError> {
        self.observe_clocks();
        let resume = self.resume.filter(|_| !self.exhausted);
        let Some(resume) = resume else {
            return Err(FenceError::Exhausted);
        };
        let object_key = (file, context);
        let volume_key = (file.cell, file.volume, context);
        let object = self.objects.get(&object_key).copied();
        let volume = self.volumes.get(&volume_key).copied();
        let mut needed = 0;
        let mut keep = Vec::new();
        match object {
            Some(_) => keep.push(Slot::Object(file, context)),
            None => needed += OBJECT_RECORD_BYTES,
        }
        match volume {
            Some(_) => keep.push(Slot::Volume(volume_key)),
            None => needed += VOLUME_RECORD_BYTES,
        }
        let values = u64::from(object.is_none()) + u64::from(volume.is_none());
        if !self.values_left(values) {
            self.exhausted = true;
            return Err(FenceError::Exhausted);
        }
        self.make_room(needed, &keep);
        let object = match object {
            Some(value) => value,
            None => self.make(Slot::Object(file, context), None)?,
        };
        let volume = match volume {
            Some(record) => record,
            None => VolumeRecord {
                stamp: self.make(Slot::Volume(volume_key), None)?,
                epoch: None,
            },
        };
        Ok(FenceTuple {
            object: Stamp(object),
            volume: Stamp(volume.stamp),
            suspect_epoch: server.suspect_epoch,
            pause: server.pause,
            resume: Stamp(resume),
            volume_epoch: volume.epoch,
        })
    }

    /// Raises `file`'s break counter in every context that holds it (row
    /// B1; for a source that matches no known server, row B2, the caller
    /// names the file in each cell). Returns how many contexts hold it: 0
    /// is the callback specification's `break_unknown_fid` for this cell
    /// unless the volume is broken too.
    pub fn break_file(&mut self, file: &FileId) -> usize {
        let range = (*file, FIRST_CONTEXT)..=(*file, LAST_CONTEXT);
        let mut reached = 0;
        for (&(_, context), value) in self.objects.range_mut(range) {
            reached += 1;
            let Some(fresh) = take(&mut self.next) else {
                self.exhausted = true;
                continue;
            };
            self.order.remove(value);
            self.order.insert(fresh, Slot::Object(*file, context));
            *value = fresh;
        }
        reached
    }

    /// Raises the break counter of `volume` in `cell` in every context that
    /// holds it: a volume break (callback specification, 5.4, point 6), or
    /// a context's ambiguity (incarnation specification, 5.6, point 3, step
    /// 2). Returns how many contexts hold it.
    pub fn break_volume(&mut self, cell: CellIndex, volume: VolumeId) -> usize {
        let range = (cell, volume, FIRST_CONTEXT)..=(cell, volume, LAST_CONTEXT);
        let mut reached = 0;
        for (key, record) in self.volumes.range_mut(range) {
            reached += 1;
            let Some(fresh) = take(&mut self.next) else {
                self.exhausted = true;
                continue;
            };
            self.order.remove(&record.stamp);
            self.order.insert(fresh, Slot::Volume(*key));
            record.stamp = fresh;
        }
        reached
    }

    /// Writes `epoch` as `context`'s local volume epoch for `volume` in
    /// `cell`, making the context's volume record if absent. The
    /// incarnation code calls it in the step that sets or changes the
    /// evidence record's epoch (outcomes "new", "confirmed" and
    /// "ambiguous"), under the volume's lock. A changed epoch fails every
    /// tuple recorded under the old one.
    ///
    /// # Errors
    ///
    /// [`FenceError::Exhausted`] if the record is absent and no value is
    /// left to make it (which exhausts the table). Nothing else changes.
    pub fn set_volume_epoch(
        &mut self,
        context: ContextKey,
        cell: CellIndex,
        volume: VolumeId,
        epoch: VolumeEpoch,
    ) -> Result<(), FenceError> {
        let key = (cell, volume, context);
        if let Some(record) = self.volumes.get_mut(&key) {
            record.epoch = Some(epoch);
            return Ok(());
        }
        if self.exhausted || !self.values_left(1) {
            self.exhausted = true;
            return Err(FenceError::Exhausted);
        }
        self.make_room(VOLUME_RECORD_BYTES, &[]);
        self.make(Slot::Volume(key), Some(epoch)).map(|_| ())
    }

    /// Takes `context`'s current local volume epoch for `file`'s volume into
    /// `ticket`, a tuple recorded with none, at the install whose own
    /// observation wrote that epoch (see the module's documentation).
    /// Returns whether it did: only if the ticket has no epoch, the volume
    /// record has the ticket's counter value (it has existed, unbroken,
    /// since the ticket was recorded), and it has an epoch. Otherwise the
    /// ticket is unchanged and fails its check.
    #[must_use]
    pub fn adopt_volume_epoch(
        &self,
        context: ContextKey,
        file: FileId,
        ticket: &mut FenceTuple,
    ) -> bool {
        if ticket.volume_epoch.is_some() || self.exhausted {
            return false;
        }
        let Some(record) = self.volumes.get(&(file.cell, file.volume, context)) else {
            return false;
        };
        if !Stamp(record.stamp).matches(ticket.volume) {
            return false;
        }
        ticket.volume_epoch = record.epoch;
        record.epoch.is_some()
    }

    /// The fence check (callback specification, 5.6, point 2), the one
    /// function used where a reply is installed and where an entry is
    /// served. It consults the clock watch and, on a move, raises the
    /// resume generation; then it compares every value of the current
    /// tuple of `file` in `context`, with `server`'s current values, with
    /// `recorded`. Anything but a clear result fails the fence. `context`
    /// must be the request's current context and credential generation, so
    /// that a credential change fails the check.
    #[must_use]
    pub fn check(
        &mut self,
        context: ContextKey,
        file: FileId,
        server: ServerValues,
        recorded: &FenceTuple,
    ) -> Suspicion {
        self.observe_clocks();
        let current = |value: Option<u64>| match value {
            Some(value) if !self.exhausted => Stamp(value),
            _ => Stamp::NONE,
        };
        let volume = self.volumes.get(&(file.cell, file.volume, context));
        recorded.compare(&FenceTuple {
            object: current(self.objects.get(&(file, context)).copied()),
            volume: current(volume.map(|record| record.stamp)),
            suspect_epoch: server.suspect_epoch,
            pause: server.pause,
            resume: self.resume.map_or(Stamp::NONE, Stamp),
            volume_epoch: volume.and_then(|record| record.epoch),
        })
    }

    /// Whether `context` holds `file`'s object record.
    #[must_use]
    pub fn holds_object(&self, context: ContextKey, file: FileId) -> bool {
        self.objects.contains_key(&(file, context))
    }

    /// Whether `context` holds the volume record of `volume` in `cell`.
    #[must_use]
    pub fn holds_volume(&self, context: ContextKey, cell: CellIndex, volume: VolumeId) -> bool {
        self.volumes.contains_key(&(cell, volume, context))
    }

    /// The object records held, over every context.
    #[must_use]
    pub fn objects(&self) -> usize {
        self.objects.len()
    }

    /// The volume records held, over every context.
    #[must_use]
    pub fn volumes(&self) -> usize {
        self.volumes.len()
    }

    /// The bytes charged for the records held: at most the budget.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Whether the table is exhausted: every check fails from then on, and
    /// every recording is refused.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.exhausted || self.resume.is_none()
    }

    /// Consults the clock watch; on a move, raises the resume generation,
    /// which past `u64::MAX` is exhausted.
    fn observe_clocks(&mut self) {
        if self.watch.moved() {
            self.resume = self.resume.and_then(|value| value.checked_add(1));
        }
    }

    /// Whether `count` more values can be taken.
    fn values_left(&self, count: u64) -> bool {
        count == 0
            || self
                .next
                .is_some_and(|next| next.checked_add(count - 1).is_some())
    }

    /// Evicts the records with the smallest values, other than `keep`,
    /// until `needed` more bytes fit in the budget. It always succeeds: the
    /// budget holds at least one record of each kind, an operation needs at
    /// most that, and `keep` holds at most the one record of the kind it
    /// does not need.
    fn make_room(&mut self, needed: usize, keep: &[Slot]) {
        let victims: Vec<u64> = self
            .order
            .iter()
            .filter(|(_, slot)| !keep.contains(slot))
            .scan(self.bytes, |held, (&value, slot)| {
                let fits = held
                    .checked_add(needed)
                    .is_some_and(|total| total <= self.limits.bytes);
                (!fits).then(|| {
                    *held = held.saturating_sub(cost(slot));
                    value
                })
            })
            .collect();
        for value in victims {
            self.evict(value);
        }
    }

    fn evict(&mut self, value: u64) {
        let Some(slot) = self.order.remove(&value) else {
            return;
        };
        match slot {
            Slot::Object(file, context) => {
                self.objects.remove(&(file, context));
            }
            Slot::Volume(key) => {
                self.volumes.remove(&key);
            }
        }
        self.bytes = self.bytes.saturating_sub(cost(&slot));
    }

    /// Makes a record with a fresh value; the caller has checked that one
    /// is left and that it fits.
    fn make(&mut self, slot: Slot, epoch: Option<VolumeEpoch>) -> Result<u64, FenceError> {
        let value = take(&mut self.next).ok_or(FenceError::Exhausted)?;
        match slot {
            Slot::Object(file, context) => {
                self.objects.insert((file, context), value);
            }
            Slot::Volume(key) => {
                self.volumes.insert(
                    key,
                    VolumeRecord {
                        stamp: value,
                        epoch,
                    },
                );
            }
        }
        self.order.insert(value, slot);
        self.bytes = self.bytes.saturating_add(cost(&slot));
        Ok(value)
    }
}
