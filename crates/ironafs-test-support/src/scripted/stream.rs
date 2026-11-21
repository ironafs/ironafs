// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! One binding's invalidation stream in the scripted core (ADR 0004,
//! C17): a slot per node holding the union of its pending effects and a
//! revision, and one overflow when the slots pass their byte bound.
//!
//! Revisions count up per binding, and every merge takes the next one.
//! An event is delivered once; its acknowledgement clears the slot if
//! nothing merged into it since delivery, and otherwise leaves only the
//! later effects pending, under the slot's higher revision (C17,
//! point 6). The union is [`Effects::merge`], the contract's own rule,
//! so the scripted core widens exactly as the contract says.

use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use ironafs_vfs::{
    Ack, Effects, Entries, Event, Invalidation, NodeId, Overflow, Revision, Rights, Scope, VfsError,
};

#[cfg(test)]
mod tests;

/// The bytes one slot counts toward the bound, besides its names' bytes.
pub(super) const SLOT_BYTES: usize = 64;

/// Every effect: what a purge offered again after a failure asks for.
const EVERYTHING: Effects = Effects {
    data: true,
    attributes: true,
    entries: Some(Entries::All),
    rights: Some(Rights::All),
};

/// A node's pending effects in one binding.
#[derive(Debug)]
struct Slot {
    scope: Scope,
    effects: Effects,
    /// The effects merged since the slot was last delivered.
    since_delivery: Effects,
    revision: Revision,
    delivered: Option<Revision>,
}

/// The binding's pending overflow.
#[derive(Debug)]
struct PendingOverflow {
    revision: Revision,
    delivered: Option<Revision>,
}

#[derive(Debug)]
pub(super) struct Stream {
    bound: usize,
    held: BTreeSet<NodeId>,
    slots: BTreeMap<NodeId, Slot>,
    overflow: Option<PendingOverflow>,
    /// The last revision issued to this binding.
    last: Revision,
}

impl Stream {
    pub(super) const fn new(bound: usize) -> Self {
        Self {
            bound,
            held: BTreeSet::new(),
            slots: BTreeMap::new(),
            overflow: None,
            last: Revision(0),
        }
    }

    pub(super) fn hold(&mut self, node: NodeId) {
        self.held.insert(node);
    }

    pub(super) fn holds(&self, node: NodeId) -> bool {
        self.held.contains(&node)
    }

    /// Merges an event into `node`'s slot, if this binding holds the node,
    /// and overflows if the slots then pass the bound.
    pub(super) fn push(&mut self, node: NodeId, scope: Scope, effects: &Effects) {
        if !self.holds(node) {
            return;
        }
        let revision = self.issue();
        match self.slots.get_mut(&node) {
            Some(slot) => {
                slot.scope = slot.scope.max(scope);
                slot.effects.merge(effects.clone());
                slot.since_delivery.merge(effects.clone());
                slot.revision = revision;
            }
            None => {
                self.slots.insert(
                    node,
                    Slot {
                        scope,
                        effects: effects.clone(),
                        since_delivery: effects.clone(),
                        revision,
                        delivered: None,
                    },
                );
            }
        }
        if self.pending_bytes() > self.bound {
            // Every held node is gated and purged under the overflow, so
            // the slots it replaces lose nothing.
            self.slots.clear();
            let delivered = self.overflow.as_ref().and_then(|pending| pending.delivered);
            self.overflow = Some(PendingOverflow {
                revision,
                delivered,
            });
        }
    }

    pub(super) fn next_event(&mut self) -> Option<Event> {
        if let Some(pending) = self.overflow.as_mut()
            && pending.delivered.is_none()
        {
            pending.delivered = Some(pending.revision);
            return Some(Event::Overflow(Overflow {
                revision: pending.revision,
            }));
        }
        let (node, slot) = self
            .slots
            .iter_mut()
            .find(|(_, slot)| slot.delivered.is_none())?;
        slot.delivered = Some(slot.revision);
        slot.since_delivery = Effects::NONE;
        Some(Event::Node(Invalidation {
            node: *node,
            scope: slot.scope,
            effects: slot.effects.clone(),
            revision: slot.revision,
        }))
    }

    /// Applies `ack`, and says whether it acknowledged a delivered purge
    /// of a node.
    pub(super) fn ack(&mut self, ack: Ack) -> Result<bool, VfsError> {
        let mut applied = false;
        match ack {
            Ack::Node { node, revision } => {
                self.issued(revision)?;
                if let Some(slot) = self.slots.get_mut(&node)
                    && slot.delivered == Some(revision)
                {
                    applied = true;
                    if slot.revision == revision {
                        self.slots.remove(&node);
                    } else {
                        slot.effects = mem::replace(&mut slot.since_delivery, Effects::NONE);
                        slot.delivered = None;
                    }
                }
            }
            Ack::Overflow { revision } => {
                self.issued(revision)?;
                if let Some(pending) = self.overflow.as_mut()
                    && pending.delivered == Some(revision)
                {
                    if pending.revision == revision {
                        self.overflow = None;
                    } else {
                        pending.delivered = None;
                    }
                }
            }
        }
        Ok(applied)
    }

    /// The layer could not apply `node`'s purge: the purge is offered
    /// again, widened to every effect, under a new revision. The failed
    /// delivery is never acknowledged, so it clears nothing.
    pub(super) fn retry(&mut self, node: NodeId) {
        self.push(node, Scope::Object, &EVERYTHING);
        if let Some(slot) = self.slots.get_mut(&node) {
            slot.delivered = None;
            slot.since_delivery = Effects::NONE;
        }
    }

    pub(super) fn gated(&self, node: NodeId) -> bool {
        self.holds(node) && (self.overflow.is_some() || self.slots.contains_key(&node))
    }

    fn issue(&mut self) -> Revision {
        self.last = Revision(self.last.0.saturating_add(1));
        self.last
    }

    fn issued(&self, revision: Revision) -> Result<(), VfsError> {
        if revision > self.last {
            return Err(VfsError::OutOfRange);
        }
        Ok(())
    }

    fn pending_bytes(&self) -> usize {
        self.slots
            .values()
            .map(|slot| {
                let names = match &slot.effects.entries {
                    None | Some(Entries::All) => 0,
                    Some(Entries::Name(name)) => name.as_bytes().len(),
                    Some(Entries::Rename { from, to }) => {
                        from.as_bytes().len() + to.as_bytes().len()
                    }
                };
                SLOT_BYTES + names
            })
            .sum()
    }
}
