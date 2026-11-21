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
    Ack, Effects, Entries, Event, Invalidation, NodeId, Overflow, Revision, Rights, Scope,
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
    /// The highest revision delivered to it.
    latest_delivered: Revision,
}

/// What an acknowledgement did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Acked {
    /// It applied a node's purge up to this revision.
    Node(NodeId, Revision),
    /// It applied the overflow's purge up to this revision.
    Overflow(Revision),
    /// It repeated an earlier delivery's acknowledgement: nothing changed.
    Nothing,
    /// It named nothing delivered: nothing changed, and it is a layer
    /// fault (contract specification, 5.10, point 6).
    Fault,
}

impl Stream {
    pub(super) const fn new(bound: usize) -> Self {
        Self {
            bound,
            held: BTreeSet::new(),
            slots: BTreeMap::new(),
            overflow: None,
            last: Revision(0),
            latest_delivered: Revision(0),
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
            self.latest_delivered = self.latest_delivered.max(pending.revision);
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
        self.latest_delivered = self.latest_delivered.max(slot.revision);
        Some(Event::Node(Invalidation {
            node: *node,
            scope: slot.scope,
            effects: slot.effects.clone(),
            revision: slot.revision,
        }))
    }

    /// Applies `ack` (contract specification, 5.10, points 3 and 6).
    pub(super) fn ack(&mut self, ack: Ack) -> Acked {
        let revision = match ack {
            Ack::Node { revision, .. } | Ack::Overflow { revision } => revision,
        };
        if !self.delivered_once(revision) {
            return Acked::Fault;
        }
        match ack {
            Ack::Node { node, revision } => {
                let Some(slot) = self.slots.get_mut(&node) else {
                    return Acked::Fault;
                };
                if slot.delivered != Some(revision) {
                    // An earlier delivery again: idempotent.
                    return Acked::Nothing;
                }
                if slot.revision == revision {
                    self.slots.remove(&node);
                } else {
                    slot.effects = mem::replace(&mut slot.since_delivery, Effects::NONE);
                    slot.delivered = None;
                }
                Acked::Node(node, revision)
            }
            Ack::Overflow { revision } => {
                let Some(pending) = self.overflow.as_mut() else {
                    return Acked::Fault;
                };
                if pending.delivered != Some(revision) {
                    return Acked::Nothing;
                }
                if pending.revision == revision {
                    self.overflow = None;
                } else {
                    pending.delivered = None;
                }
                Acked::Overflow(revision)
            }
        }
    }

    /// Whether a report naming `node` and `revision` can be the failure of
    /// a purge this binding was delivered: it holds the node, and the
    /// revision was delivered to it.
    pub(super) fn may_have_failed(&self, node: NodeId, revision: Revision) -> bool {
        self.holds(node) && self.delivered_once(revision)
    }

    /// The layer could not apply `node`'s purge: the purge is offered
    /// again, widened to every effect, under a new revision, which it
    /// returns. The failed delivery is never acknowledged, so it clears
    /// nothing. If the re-offer folds into an overflow, the overflow is
    /// offered again instead, so the purge is never lost.
    pub(super) fn retry(&mut self, node: NodeId) -> Revision {
        self.push(node, Scope::Object, &EVERYTHING);
        if let Some(slot) = self.slots.get_mut(&node) {
            slot.delivered = None;
            slot.since_delivery = Effects::NONE;
        } else if let Some(pending) = self.overflow.as_mut() {
            pending.delivered = None;
        }
        self.last
    }

    pub(super) fn gated(&self, node: NodeId) -> bool {
        self.holds(node) && (self.overflow.is_some() || self.slots.contains_key(&node))
    }

    fn issue(&mut self) -> Revision {
        self.last = Revision(self.last.0.saturating_add(1));
        self.last
    }

    /// Whether `revision` is one this binding was delivered: never zero,
    /// which is never issued, and never above the latest delivery.
    fn delivered_once(&self, revision: Revision) -> bool {
        revision > Revision(0) && revision <= self.latest_delivered
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
