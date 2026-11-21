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
//!   `&mut self`: a serve and a submission cannot check without the lock.
//!   The clock comparison, the resume generation's raise, the load of the
//!   pause generation and the comparison of the six values are one step,
//!   in that order (callback specification, 5.6, point 2), that no other
//!   caller can come between. The lock is a leaf in C39, point 4's order:
//!   taken after any other lock the caller holds, held for one operation,
//!   and no lock is taken while it is held. A break or an epoch change is
//!   recorded under it before the caller queues any event (C16, point 1),
//!   so every later check sees it.
//! - **Values from outside.** The pause generation is the driver's atomic,
//!   which the table itself loads, sequentially consistent, after the clock
//!   comparison (Rx specification, 5.12, point 5); the caller hands it the
//!   atomic in [`Server`], never a value sampled earlier. The suspect epoch
//!   comes from the callback state record, in [`Server`] too.
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
//!   locked read, inside the table's fixed memory. A volume record evicted
//!   loses the copy: tuples recorded under it fail.
//! - **A tuple recorded with no epoch** fails its check, except at the
//!   install whose own observation wrote the epoch: there the incarnation
//!   code calls [`FenceTable::adopt_volume_epoch`], which takes the epoch
//!   into the tuple only if the table can show that no copy existed for
//!   that context and volume at any time up to now. A copy is only ever
//!   lost by eviction, so the table latches when it first evicts a record
//!   holding one, and from then on refuses every adoption; until then, a
//!   tuple with no epoch whose volume record is unbroken since it was
//!   recorded proves that the context had no copy, so no evidence epoch,
//!   before the fetch was sent, and an epoch written since by anything but
//!   this reply's own observation would have needed ambiguity, which raises
//!   the volume's counter (its 5.6, point 3, step 2). A refused adoption
//!   fails the fence, and the fetch is retried within C16, point 2's bound;
//!   the retry records the epoch this reply's observation wrote. The cost,
//!   once the latch is set, is a refetch whenever a fetch is sent while
//!   its context holds no copy for the volume: on its first fetch of the
//!   volume, and again each time its copy is evicted, even between an
//!   attempt and its retry, within C16, point 2's bound on retries.
//! - **Memory, fixed by construction** (coding standards, rule 3). The
//!   table reserves, once, in [`FenceTable::new`], one slab of records and
//!   two sorted indexes, each with room for [`FenceTable::capacity`]
//!   records, which is [`FenceLimits::bytes`] divided by [`SLOT_BYTES`].
//!   Nothing grows afterwards and no operation allocates: a record is made
//!   only after room is made for it, so what the table holds is
//!   [`FenceTable::bytes`], at most the budget, whatever its contents. A
//!   record made in a full table evicts the record with the smallest
//!   value, of either kind, the one made or raised longest ago `[OWN]`,
//!   never one the operation itself uses. Eviction is always safe: it only
//!   makes the tuples recorded under the evicted record fail their fence
//!   (a revalidation, never a stale serve). The indexes are sorted arrays:
//!   a lookup is a binary search, and making or evicting a record moves at
//!   most the index's entries, a few megabytes at [`MAX_FENCE_BYTES`].
//!   A key's place in the sorted order decides how many entries that
//!   move shifts, so a key read from the network can move the cost within
//!   that bound but never past it; no hash is used, so no key can make a
//!   lookup worse than a binary search.
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

use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use core::sync::atomic::Ordering;

use ironafs_vfs::{ContextId, CredentialGeneration};

use super::{ClockWatch, FenceTuple, PauseGeneration, Server, Stamp, Suspicion, VolumeEpoch};
use crate::{CellIndex, ContextKey, FileId, VolumeId};

#[cfg(test)]
mod tests;

/// The bytes one record's room takes: its slot in the slab and its entry
/// in each index.
pub const SLOT_BYTES: usize =
    size_of::<Record>() + size_of::<ObjectEntry>() + size_of::<VolumeEntry>();

/// The smallest budget: room for one record of each kind, what one
/// recording needs.
pub const MIN_FENCE_BYTES: usize = 2 * SLOT_BYTES;

/// The largest budget: 16 MiB `[OWN]`, room for about a hundred thousand
/// records, and small enough that making or evicting a record moves at most
/// a few megabytes of index under the lock.
pub const MAX_FENCE_BYTES: usize = 16 << 20;

/// The table's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FenceLimits {
    /// The bytes the table may hold: [`MIN_FENCE_BYTES`] to
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
    /// The table's memory could not be reserved.
    NoMemory,
}

impl fmt::Display for FenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exhausted => "fence values exhausted",
            Self::InvalidLimits => "limits out of range",
            Self::NoMemory => "fence table memory not reserved",
        })
    }
}

impl core::error::Error for FenceError {}

/// A volume record's key: ordered so that one volume's records, in every
/// context, are one range.
type VolumeKey = (CellIndex, VolumeId, ContextKey);

type ObjectEntry = ((FileId, ContextKey), u32);
type VolumeEntry = (VolumeKey, u32);

/// No slot: the end of a list.
const NIL: u32 = u32::MAX;

/// What a record is the counter of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Object(FileId, ContextKey),
    Volume(VolumeKey),
}

/// A record in the slab, linked into the list of live records in value
/// order, or into the free list by `newer`.
#[derive(Clone, Copy, Debug)]
struct Record {
    slot: Slot,
    value: u64,
    epoch: Option<VolumeEpoch>,
    older: u32,
    newer: u32,
}

const FIRST_CONTEXT: ContextKey = ContextKey {
    id: ContextId(0),
    generation: CredentialGeneration(0),
};

/// The fence table. `W` is the clock watch it consults before every
/// recording and every check.
#[derive(Debug)]
pub struct FenceTable<W> {
    capacity: usize,
    /// The next counter value; `None` once `u64::MAX` is taken.
    next: Option<u64>,
    /// Set when an operation found too few values left.
    exhausted: bool,
    /// Set when the table first evicted a record holding an epoch copy.
    copy_lost: bool,
    /// The resume generation; `None` once raised past `u64::MAX`.
    resume: Option<u64>,
    watch: W,
    /// The slab; it never holds more than `capacity` slots.
    records: Vec<Record>,
    /// The live records, oldest value first.
    oldest: u32,
    newest: u32,
    /// Freed slots, linked by `newer`.
    free: u32,
    live: usize,
    /// Object records by key, sorted.
    objects: Vec<ObjectEntry>,
    /// Volume records by key, sorted.
    volumes: Vec<VolumeEntry>,
}

/// Takes the next value of the sequence.
fn take(next: &mut Option<u64>) -> Option<u64> {
    let value = (*next)?;
    *next = value.checked_add(1);
    Some(value)
}

fn index(slot: u32) -> Option<usize> {
    usize::try_from(slot).ok()
}

/// A vector with room for exactly `capacity` elements, or `NoMemory`.
fn reserved<T>(capacity: usize) -> Result<Vec<T>, FenceError> {
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(capacity)
        .map_err(|_| FenceError::NoMemory)?;
    Ok(vector)
}

impl<W: ClockWatch> FenceTable<W> {
    /// An empty table, with its memory reserved and the resume generation
    /// at 1.
    ///
    /// # Errors
    ///
    /// [`FenceError::InvalidLimits`] if the budget is out of range;
    /// [`FenceError::NoMemory`] if the memory cannot be reserved within it.
    pub fn new(limits: FenceLimits, watch: W) -> Result<Self, FenceError> {
        if !(MIN_FENCE_BYTES..=MAX_FENCE_BYTES).contains(&limits.bytes) {
            return Err(FenceError::InvalidLimits);
        }
        let capacity = limits.bytes / SLOT_BYTES;
        let table = Self {
            capacity,
            next: Some(1),
            exhausted: false,
            copy_lost: false,
            resume: Some(1),
            watch,
            records: reserved(capacity)?,
            oldest: NIL,
            newest: NIL,
            free: NIL,
            live: 0,
            objects: reserved(capacity)?,
            volumes: reserved(capacity)?,
        };
        if table.bytes() > limits.bytes {
            return Err(FenceError::NoMemory);
        }
        Ok(table)
    }

    /// The fence tuple of `file` in `context`, recorded by a call that can
    /// validate an entry or grant a promise, when it is sent (callback
    /// specification, 5.6, point 3). `context` is the request's current
    /// context and credential generation. Consults the clock watch first,
    /// loads the pause generation, then makes the context's object and
    /// volume records if absent.
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
        server: Server<'_>,
    ) -> Result<FenceTuple, FenceError> {
        self.observe_clocks();
        let resume = self.resume.filter(|_| !self.exhausted);
        let Some(resume) = resume else {
            return Err(FenceError::Exhausted);
        };
        let pause = PauseGeneration(server.pause.load(Ordering::SeqCst));
        let volume_key = (file.cell, file.volume, context);
        let object = self.find_object(file, context);
        let volume = self.find_volume(volume_key);
        let values = u64::from(object.is_none()) + u64::from(volume.is_none());
        if !self.values_left(values) {
            self.exhausted = true;
            return Err(FenceError::Exhausted);
        }
        let needed = usize::from(object.is_none()) + usize::from(volume.is_none());
        self.make_room(needed, [object, volume]);
        let object = match object {
            Some(slot) => slot,
            None => self.make(Slot::Object(file, context), None)?,
        };
        let volume = match volume {
            Some(slot) => slot,
            None => self.make(Slot::Volume(volume_key), None)?,
        };
        let value = |slot| self.record_at(slot).map_or(0, |record| record.value);
        Ok(FenceTuple {
            object: Stamp(value(object)),
            volume: Stamp(value(volume)),
            suspect_epoch: server.suspect_epoch,
            pause,
            resume: Stamp(resume),
            volume_epoch: self.record_at(volume).and_then(|record| record.epoch),
        })
    }

    /// Raises `file`'s break counter in every context that holds it (row
    /// B1; for a source that matches no known server, row B2, the caller
    /// names the file in each cell). Returns how many contexts hold it: 0
    /// is the callback specification's `break_unknown_fid` for this cell
    /// unless the volume is broken too.
    pub fn break_file(&mut self, file: &FileId) -> usize {
        let mut at = self
            .objects
            .partition_point(|entry| entry.0 < (*file, FIRST_CONTEXT));
        let mut reached = 0;
        while let Some(&((held, _), slot)) = self.objects.get(at) {
            if held != *file {
                break;
            }
            reached += 1;
            self.raise(slot);
            at += 1;
        }
        reached
    }

    /// Raises the break counter of `volume` in `cell` in every context that
    /// holds it: a volume break (callback specification, 5.4, point 6), or
    /// a context's ambiguity (incarnation specification, 5.6, point 3, step
    /// 2). Returns how many contexts hold it.
    pub fn break_volume(&mut self, cell: CellIndex, volume: VolumeId) -> usize {
        let mut at = self
            .volumes
            .partition_point(|entry| entry.0 < (cell, volume, FIRST_CONTEXT));
        let mut reached = 0;
        while let Some(&((held_cell, held_volume, _), slot)) = self.volumes.get(at) {
            if (held_cell, held_volume) != (cell, volume) {
                break;
            }
            reached += 1;
            self.raise(slot);
            at += 1;
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
        if let Some(slot) = self.find_volume(key) {
            if let Some(record) = self.record_at_mut(slot) {
                record.epoch = Some(epoch);
            }
            return Ok(());
        }
        if self.exhausted || !self.values_left(1) {
            self.exhausted = true;
            return Err(FenceError::Exhausted);
        }
        self.make_room(1, [None, None]);
        self.make(Slot::Volume(key), Some(epoch)).map(|_| ())
    }

    /// Takes `context`'s current local volume epoch for `file`'s volume into
    /// `ticket`, a tuple recorded with none, at the install whose own
    /// observation wrote that epoch (see the module's documentation).
    /// Returns whether it did: only if no epoch copy has ever been evicted
    /// from the table, the ticket has no epoch, the volume record has the
    /// ticket's counter value (it has existed, unbroken, since the ticket
    /// was recorded), and it has an epoch. Otherwise the ticket is
    /// unchanged and fails its check.
    #[must_use]
    pub fn adopt_volume_epoch(
        &self,
        context: ContextKey,
        file: FileId,
        ticket: &mut FenceTuple,
    ) -> bool {
        if self.copy_lost || self.exhausted || ticket.volume_epoch.is_some() {
            return false;
        }
        let Some(record) = self
            .find_volume((file.cell, file.volume, context))
            .and_then(|slot| self.record_at(slot))
        else {
            return false;
        };
        if !Stamp(record.value).matches(ticket.volume) {
            return false;
        }
        ticket.volume_epoch = record.epoch;
        record.epoch.is_some()
    }

    /// The fence check (callback specification, 5.6, point 2), the one
    /// function used where a reply is installed and where an entry is
    /// served. It consults the clock watch and, on a move, raises the
    /// resume generation; then it loads the pause generation, and compares
    /// every value of the current tuple of `file` in `context`, with
    /// `server`'s current values, with `recorded`. Anything but a clear
    /// result fails the fence. `context` must be the request's current
    /// context and credential generation, so that a credential change fails
    /// the check.
    #[must_use]
    pub fn check(
        &mut self,
        context: ContextKey,
        file: FileId,
        server: Server<'_>,
        recorded: &FenceTuple,
    ) -> Suspicion {
        self.observe_clocks();
        let pause = PauseGeneration(server.pause.load(Ordering::SeqCst));
        let object = self
            .find_object(file, context)
            .and_then(|slot| self.record_at(slot));
        let volume = self
            .find_volume((file.cell, file.volume, context))
            .and_then(|slot| self.record_at(slot));
        let current = |record: Option<&Record>| match record {
            Some(record) if !self.exhausted => Stamp(record.value),
            _ => Stamp::NONE,
        };
        recorded.compare(&FenceTuple {
            object: current(object),
            volume: current(volume),
            suspect_epoch: server.suspect_epoch,
            pause,
            resume: self.resume.map_or(Stamp::NONE, Stamp),
            volume_epoch: volume.and_then(|record| record.epoch),
        })
    }

    /// Whether `context` holds `file`'s object record.
    #[must_use]
    pub fn holds_object(&self, context: ContextKey, file: FileId) -> bool {
        self.find_object(file, context).is_some()
    }

    /// Whether `context` holds the volume record of `volume` in `cell`.
    #[must_use]
    pub fn holds_volume(&self, context: ContextKey, cell: CellIndex, volume: VolumeId) -> bool {
        self.find_volume((cell, volume, context)).is_some()
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

    /// The most records the table holds, of both kinds together.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The bytes the table's slab and indexes hold, reserved once: at most
    /// the budget.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.records.capacity() * size_of::<Record>()
            + self.objects.capacity() * size_of::<ObjectEntry>()
            + self.volumes.capacity() * size_of::<VolumeEntry>()
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

    fn find_object(&self, file: FileId, context: ContextKey) -> Option<u32> {
        let at = self
            .objects
            .binary_search_by(|entry| entry.0.cmp(&(file, context)))
            .ok()?;
        self.objects.get(at).map(|entry| entry.1)
    }

    fn find_volume(&self, key: VolumeKey) -> Option<u32> {
        let at = self
            .volumes
            .binary_search_by(|entry| entry.0.cmp(&key))
            .ok()?;
        self.volumes.get(at).map(|entry| entry.1)
    }

    fn record_at(&self, slot: u32) -> Option<&Record> {
        self.records.get(index(slot)?)
    }

    fn record_at_mut(&mut self, slot: u32) -> Option<&mut Record> {
        self.records.get_mut(index(slot)?)
    }

    /// Evicts the records with the smallest values, other than `keep`,
    /// until `needed` more fit. It always succeeds: the table has room for
    /// at least two records, an operation needs at most two, and `keep`
    /// holds only records the operation does not need to make.
    fn make_room(&mut self, needed: usize, keep: [Option<u32>; 2]) {
        while self.live + needed > self.capacity {
            let mut victim = self.oldest;
            while victim != NIL && keep.contains(&Some(victim)) {
                victim = self.record_at(victim).map_or(NIL, |record| record.newer);
            }
            if victim == NIL {
                return;
            }
            self.evict(victim);
        }
    }

    fn evict(&mut self, slot: u32) {
        let Some(record) = self.record_at(slot).copied() else {
            return;
        };
        self.unlink(slot);
        match record.slot {
            Slot::Object(file, context) => {
                if let Ok(at) = self
                    .objects
                    .binary_search_by(|entry| entry.0.cmp(&(file, context)))
                {
                    self.objects.remove(at);
                }
            }
            Slot::Volume(key) => {
                if let Ok(at) = self.volumes.binary_search_by(|entry| entry.0.cmp(&key)) {
                    self.volumes.remove(at);
                }
                self.copy_lost |= record.epoch.is_some();
            }
        }
        let free = self.free;
        if let Some(freed) = self.record_at_mut(slot) {
            freed.newer = free;
        }
        self.free = slot;
        self.live = self.live.saturating_sub(1);
    }

    /// Makes a record with a fresh value; the caller has checked that one
    /// is left and made room for it.
    fn make(&mut self, kind: Slot, epoch: Option<VolumeEpoch>) -> Result<u32, FenceError> {
        let value = take(&mut self.next).ok_or(FenceError::Exhausted)?;
        let record = Record {
            slot: kind,
            value,
            epoch,
            older: NIL,
            newer: NIL,
        };
        let slot = if self.free == NIL {
            // Within the reserved room: `push` does not reallocate.
            let slot = u32::try_from(self.records.len()).map_err(|_| FenceError::NoMemory)?;
            if self.records.len() >= self.capacity {
                return Err(FenceError::NoMemory);
            }
            self.records.push(record);
            slot
        } else {
            let slot = self.free;
            let freed = self.record_at_mut(slot).ok_or(FenceError::NoMemory)?;
            let next_free = freed.newer;
            *freed = record;
            self.free = next_free;
            slot
        };
        match kind {
            Slot::Object(file, context) => {
                let at = self
                    .objects
                    .partition_point(|entry| entry.0 < (file, context));
                self.objects.insert(at, ((file, context), slot));
            }
            Slot::Volume(key) => {
                let at = self.volumes.partition_point(|entry| entry.0 < key);
                self.volumes.insert(at, (key, slot));
            }
        }
        self.link_newest(slot);
        self.live += 1;
        Ok(slot)
    }

    /// Gives a record a fresh value, the newest; without one, exhausts the
    /// table.
    fn raise(&mut self, slot: u32) {
        let Some(fresh) = take(&mut self.next) else {
            self.exhausted = true;
            return;
        };
        self.unlink(slot);
        if let Some(record) = self.record_at_mut(slot) {
            record.value = fresh;
        }
        self.link_newest(slot);
    }

    fn unlink(&mut self, slot: u32) {
        let Some(&Record { older, newer, .. }) = self.record_at(slot) else {
            return;
        };
        match self.record_at_mut(older) {
            Some(record) => record.newer = newer,
            None => self.oldest = newer,
        }
        match self.record_at_mut(newer) {
            Some(record) => record.older = older,
            None => self.newest = older,
        }
    }

    fn link_newest(&mut self, slot: u32) {
        let newest = self.newest;
        if let Some(record) = self.record_at_mut(slot) {
            record.older = newest;
            record.newer = NIL;
        }
        match self.record_at_mut(newest) {
            Some(record) => record.newer = slot,
            None => self.oldest = slot,
        }
        self.newest = slot;
    }
}
