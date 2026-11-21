// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The object and volume break counters, per context (ADR 0004, C16,
//! point 2; callback specification, 5.6, points 2 and 9, rows B1 and B2;
//! incarnation specification, 5.6, points 3 and 4).
//!
//! - **Records.** A context holds an object's counter from the first fence
//!   tuple it records for the object, and its volume's likewise. A break
//!   raises the counter in every context that holds it, and creates
//!   nothing in a context that does not: such a context has recorded no
//!   tuple the break could need to fence.
//! - **Values.** Every value, made or raised, of either kind, is taken
//!   once from one sequence that only rises. So a recorded value equals a
//!   current one only if that record has existed, unbroken, since the
//!   tuple was recorded: a record evicted and made again, or broken, has a
//!   value no recorded tuple holds.
//! - **Bounds.** At most [`BreakLimits::objects`] object records and
//!   [`BreakLimits::volumes`] volume records. When a kind is full, a new
//!   record evicts that kind's record with the smallest value, the one
//!   made or raised longest ago `[OWN]`. Eviction is always safe: it only
//!   makes the tuples recorded under the evicted record fail their fence
//!   (a revalidation, never a stale serve).
//! - **Exhaustion.** When the sequence has no value left for a break, the
//!   table is exhausted: every check fails on both counters, and every
//!   new record is refused with [`FenceError::Exhausted`], so nothing is
//!   sent under a tuple that can never match. This is the callback
//!   specification's rule for the suspect epoch (section 6, point 3)
//!   applied to these counters `[OWN]`; at one value per nanosecond the
//!   sequence lasts over 500 years.
//!
//! The table is changed and read under the caller's lock: a break is
//! recorded, and every later check sees it, before the caller queues any
//! event (C16, point 1).

use alloc::collections::BTreeMap;
use core::fmt;

use super::{Clocks, FenceTuple, ResumeGeneration, ServerValues, Stamp, Suspicion};
use crate::{CellIndex, ContextKey, FileId, MAX_NODES, VolumeId};

#[cfg(test)]
mod tests;

/// The table's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BreakLimits {
    /// The most object records, over every context: 1 to [`MAX_NODES`].
    pub objects: u32,
    /// The most volume records, over every context: 1 to [`MAX_NODES`].
    pub volumes: u32,
}

/// Why the table refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FenceError {
    /// No counter value is left: nothing may be sent under a new tuple.
    Exhausted,
    /// Limits out of range.
    InvalidLimits,
}

impl fmt::Display for FenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exhausted => "break counters exhausted",
            Self::InvalidLimits => "limits out of range",
        })
    }
}

impl core::error::Error for FenceError {}

/// The per-context object and volume break counters.
#[derive(Debug)]
pub struct BreakCounters {
    /// The next value; `None` once `u64::MAX` is taken.
    next: Option<u64>,
    /// Set when a raise found no value.
    exhausted: bool,
    objects: Counters<FileId>,
    volumes: Counters<(CellIndex, VolumeId)>,
}

/// One kind's records.
#[derive(Debug)]
struct Counters<K> {
    limit: usize,
    /// The records, by key then context, so a break is one range.
    records: BTreeMap<K, BTreeMap<ContextKey, u64>>,
    /// Every record by its value, for eviction. Values are unique.
    order: BTreeMap<u64, (K, ContextKey)>,
}

/// Takes the next value of the sequence.
fn take(next: &mut Option<u64>) -> Option<u64> {
    let value = (*next)?;
    *next = value.checked_add(1);
    Some(value)
}

impl<K: Ord + Copy> Counters<K> {
    fn new(limit: u32) -> Self {
        Self {
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
            records: BTreeMap::new(),
            order: BTreeMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.order.len()
    }

    fn get(&self, key: &K, context: ContextKey) -> Option<u64> {
        self.records.get(key)?.get(&context).copied()
    }

    /// The record's value, made with a fresh value if absent; `None` if
    /// none is left.
    fn ensure(&mut self, key: K, context: ContextKey, next: &mut Option<u64>) -> Option<u64> {
        if let Some(value) = self.get(&key, context) {
            return Some(value);
        }
        let value = take(next)?;
        if self.len() >= self.limit
            && let Some((_, (old_key, old_context))) = self.order.pop_first()
        {
            self.remove(&old_key, old_context);
        }
        self.records.entry(key).or_default().insert(context, value);
        self.order.insert(value, (key, context));
        Some(value)
    }

    fn remove(&mut self, key: &K, context: ContextKey) {
        if let Some(contexts) = self.records.get_mut(key) {
            contexts.remove(&context);
            if contexts.is_empty() {
                self.records.remove(key);
            }
        }
    }

    /// Gives every context's record of `key` a fresh value; returns how
    /// many records `key` has, and `false` if the values ran out.
    fn raise(&mut self, key: &K, next: &mut Option<u64>) -> (usize, bool) {
        let Some(contexts) = self.records.get_mut(key) else {
            return (0, true);
        };
        let reached = contexts.len();
        for (context, value) in contexts.iter_mut() {
            let Some(fresh) = take(next) else {
                return (reached, false);
            };
            self.order.remove(value);
            self.order.insert(fresh, (*key, *context));
            *value = fresh;
        }
        (reached, true)
    }
}

impl BreakCounters {
    /// An empty table.
    ///
    /// # Errors
    ///
    /// [`FenceError::InvalidLimits`] if a limit is out of range.
    pub fn new(limits: BreakLimits) -> Result<Self, FenceError> {
        if !(1..=MAX_NODES).contains(&limits.objects) || !(1..=MAX_NODES).contains(&limits.volumes)
        {
            return Err(FenceError::InvalidLimits);
        }
        Ok(Self {
            next: Some(1),
            exhausted: false,
            objects: Counters::new(limits.objects),
            volumes: Counters::new(limits.volumes),
        })
    }

    /// The fence tuple of `file` in `context`, recorded by a call that can
    /// validate an entry or grant a promise, when it is sent (callback
    /// specification, 5.6, point 3). Makes the context's object and volume
    /// records if absent.
    ///
    /// # Errors
    ///
    /// [`FenceError::Exhausted`] if the table is exhausted or a record
    /// cannot be made: the call is not sent.
    pub fn record(
        &mut self,
        context: ContextKey,
        file: FileId,
        server: ServerValues,
        resume: &ResumeGeneration,
    ) -> Result<FenceTuple, FenceError> {
        if self.exhausted {
            return Err(FenceError::Exhausted);
        }
        let object = self
            .objects
            .ensure(file, context, &mut self.next)
            .ok_or(FenceError::Exhausted)?;
        let volume = self
            .volumes
            .ensure((file.cell, file.volume), context, &mut self.next)
            .ok_or(FenceError::Exhausted)?;
        Ok(FenceTuple {
            object: Stamp(object),
            volume: Stamp(volume),
            suspect_epoch: server.suspect_epoch,
            pause: server.pause,
            resume: resume.current(),
        })
    }

    /// Raises `file`'s break counter in every context that holds it (row
    /// B1; for a source that matches no known server, row B2, the caller
    /// names the file in each cell). Returns how many contexts hold it: 0
    /// is the callback specification's `break_unknown_fid` for this cell
    /// unless the volume is broken too.
    pub fn break_file(&mut self, file: &FileId) -> usize {
        let (reached, raised) = self.objects.raise(file, &mut self.next);
        self.exhausted |= !raised;
        reached
    }

    /// Raises the break counter of `volume` in `cell` in every context that
    /// holds it: a volume break (callback specification, 5.4, point 6), or
    /// another context's ambiguity (incarnation specification, 5.6, point
    /// 3, step 2). Returns how many contexts hold it.
    pub fn break_volume(&mut self, cell: CellIndex, volume: VolumeId) -> usize {
        let (reached, raised) = self.volumes.raise(&(cell, volume), &mut self.next);
        self.exhausted |= !raised;
        reached
    }

    /// The fence check (callback specification, 5.6, point 2), the one
    /// function used where a reply is installed and where an entry is
    /// served. If `clocks` is [`Clocks::Moved`] it raises the resume
    /// generation first; then it compares every value of the current
    /// tuple of `file` in `context`, with `server`'s current values, with
    /// `recorded`. Anything but a clear result fails the fence.
    ///
    /// `clocks` is the C37 comparison's result, reported once per suspend
    /// or jump by its detector, which moves its own baseline (the keepalive
    /// specification's), so the resume generation rises once per event.
    #[must_use]
    pub fn check(
        &self,
        resume: &ResumeGeneration,
        clocks: Clocks,
        context: ContextKey,
        file: FileId,
        server: ServerValues,
        recorded: &FenceTuple,
    ) -> Suspicion {
        if clocks == Clocks::Moved {
            resume.raise();
        }
        let current = |value: Option<u64>| match value {
            Some(value) if !self.exhausted => Stamp(value),
            _ => Stamp::NONE,
        };
        recorded.compare(&FenceTuple {
            object: current(self.objects.get(&file, context)),
            volume: current(self.volumes.get(&(file.cell, file.volume), context)),
            suspect_epoch: server.suspect_epoch,
            pause: server.pause,
            resume: resume.current(),
        })
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

    /// Whether a break found no value left: every check fails from then
    /// on.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}
