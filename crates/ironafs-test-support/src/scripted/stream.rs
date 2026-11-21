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

use crate::scripted::PurgeFailed;

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
    /// The latest revision delivered for each node it holds, so a repeat
    /// is told from a forgery (contract specification, 5.10, point 6).
    delivered_for: BTreeMap<NodeId, Revision>,
    /// The latest overflow revision delivered.
    overflow_delivered: Revision,
    /// The nodes held when the pending overflow was last delivered.
    held_at_overflow: BTreeSet<NodeId>,
    /// The nodes reported failed under that delivery.
    reported_under_overflow: BTreeSet<NodeId>,
}

/// Whether a report names a delivery (contract specification, 5.10,
/// point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Validity {
    /// It names a delivery not yet acknowledged or reported.
    Valid,
    /// It names nothing pending, at or below the latest delivery.
    Repeat,
    /// It names nothing ever delivered.
    Fault,
}

impl Validity {
    /// An invalid report's kind: a repeat if `revision` was delivered,
    /// never zero and at or below `latest`, otherwise a fault.
    fn of_invalid(revision: Revision, latest: Revision) -> Self {
        if revision > Revision(0) && revision <= latest {
            Self::Repeat
        } else {
            Self::Fault
        }
    }
}

/// What a report did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Acked {
    /// It applied a node's purge up to this revision.
    Node(NodeId, Revision),
    /// It applied the overflow's purge up to this revision.
    Overflow(Revision),
    /// It repeated an earlier report: nothing changed, and it is not a
    /// fault.
    Repeat,
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
            delivered_for: BTreeMap::new(),
            overflow_delivered: Revision(0),
            held_at_overflow: BTreeSet::new(),
            reported_under_overflow: BTreeSet::new(),
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
            self.overflow_delivered = pending.revision;
            self.held_at_overflow.clone_from(&self.held);
            self.reported_under_overflow.clear();
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
        self.delivered_for.insert(*node, slot.revision);
        Some(Event::Node(Invalidation {
            node: *node,
            scope: slot.scope,
            effects: slot.effects.clone(),
            revision: slot.revision,
        }))
    }

    /// Applies `ack` if it is valid (contract specification, 5.10,
    /// points 3 and 6).
    pub(super) fn ack(&mut self, ack: Ack) -> Acked {
        match ack {
            Ack::Node { node, revision } => match self.node_report(node, revision) {
                Validity::Valid => {
                    if let Some(slot) = self.slots.get_mut(&node) {
                        if slot.revision == revision {
                            self.slots.remove(&node);
                        } else {
                            slot.effects = mem::replace(&mut slot.since_delivery, Effects::NONE);
                            slot.delivered = None;
                        }
                    }
                    Acked::Node(node, revision)
                }
                Validity::Repeat => Acked::Repeat,
                Validity::Fault => Acked::Fault,
            },
            Ack::Overflow { revision } => match self.overflow_report(revision) {
                Validity::Valid => {
                    if let Some(pending) = self.overflow.as_mut() {
                        if pending.revision == revision {
                            self.overflow = None;
                        } else {
                            pending.delivered = None;
                        }
                    }
                    Acked::Overflow(revision)
                }
                Validity::Repeat => Acked::Repeat,
                Validity::Fault => Acked::Fault,
            },
        }
    }

    /// Checks a failure report (5.10, points 5 and 6). A valid one is
    /// marked reported, so the same report again is a repeat, and the
    /// node's purge is offered again: the revision of that offer is
    /// returned.
    pub(super) fn fail(&mut self, report: PurgeFailed) -> Result<Revision, Acked> {
        let (node, validity) = match report {
            PurgeFailed::Node { node, revision } => (node, self.node_report(node, revision)),
            PurgeFailed::OverflowNode { node, revision } => {
                let validity = match self.overflow_report(revision) {
                    Validity::Valid
                        if self.held_at_overflow.contains(&node)
                            && !self.reported_under_overflow.contains(&node) =>
                    {
                        Validity::Valid
                    }
                    Validity::Valid | Validity::Repeat if self.holds(node) => Validity::Repeat,
                    Validity::Valid | Validity::Repeat | Validity::Fault => Validity::Fault,
                };
                (node, validity)
            }
        };
        match validity {
            Validity::Valid => {
                if let PurgeFailed::OverflowNode { .. } = report {
                    self.reported_under_overflow.insert(node);
                }
                Ok(self.retry(node))
            }
            Validity::Repeat => Err(Acked::Repeat),
            Validity::Fault => Err(Acked::Fault),
        }
    }

    /// Whether a report on `node`'s delivery of `revision` names a delivery
    /// not yet acknowledged or reported, repeats one, or is a fault.
    fn node_report(&self, node: NodeId, revision: Revision) -> Validity {
        if !self.holds(node) {
            return Validity::Fault;
        }
        if self
            .slots
            .get(&node)
            .is_some_and(|slot| slot.delivered == Some(revision))
        {
            return Validity::Valid;
        }
        let latest = self
            .delivered_for
            .get(&node)
            .copied()
            .unwrap_or(Revision(0));
        Validity::of_invalid(revision, latest)
    }

    /// As [`Stream::node_report`], for the overflow of `revision`.
    fn overflow_report(&self, revision: Revision) -> Validity {
        if self
            .overflow
            .as_ref()
            .is_some_and(|pending| pending.delivered == Some(revision))
        {
            return Validity::Valid;
        }
        Validity::of_invalid(revision, self.overflow_delivered)
    }

    /// The layer could not apply `node`'s purge: the purge is offered
    /// again, widened to every effect, under a new revision, which it
    /// returns. The failed delivery is never acknowledged, so it clears
    /// nothing. If the re-offer folds into an overflow, it raises the
    /// overflow's revision, so the overflow is offered again after the
    /// binding acknowledges the delivery it holds: the purge is never
    /// lost.
    fn retry(&mut self, node: NodeId) -> Revision {
        self.push(node, Scope::Object, &EVERYTHING);
        if let Some(slot) = self.slots.get_mut(&node) {
            slot.delivered = None;
            slot.since_delivery = Effects::NONE;
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
