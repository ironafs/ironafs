// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! One binding's invalidation stream in the scripted core (ADR 0004,
//! C17; contract specification, 5.10): a slot per node holding the union
//! of its pending effects, each with the revision it was last merged at,
//! and an overflow when the slots pass their byte bound.
//!
//! Revisions count up per binding, and every merge and every overflow
//! takes the next one. A slot has at most one delivery unanswered: merges
//! that arrive meanwhile wait in the slot. An overflow is delivered once
//! and stays open until it is acknowledged; slots that pass the bound
//! while it is open fold into the next overflow, delivered after it. A
//! delivery stays unanswered when its slot folds into an overflow, so the
//! binding's answer to it still counts. The union is [`Effects::merge`],
//! the contract's own rule, so the scripted core widens exactly as the
//! contract says.

use std::collections::{BTreeMap, BTreeSet};

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

/// A node's pending effects in one binding, each with the revision it
/// was last merged at (5.10, point 1).
#[derive(Debug)]
struct Slot {
    scope: Scope,
    effects: Effects,
    data: Option<Revision>,
    attributes: Option<Revision>,
    entries: Option<Revision>,
    rights: Option<Revision>,
    /// The revision of the latest merge.
    revision: Revision,
}

impl Slot {
    fn new(scope: Scope) -> Self {
        Self {
            scope,
            effects: Effects::NONE,
            data: None,
            attributes: None,
            entries: None,
            rights: None,
            revision: Revision(0),
        }
    }

    fn merge(&mut self, scope: Scope, effects: &Effects, revision: Revision) {
        self.scope = self.scope.max(scope);
        if effects.data {
            self.data = Some(revision);
        }
        if effects.attributes {
            self.attributes = Some(revision);
        }
        if effects.entries.is_some() {
            self.entries = Some(revision);
        }
        if effects.rights.is_some() {
            self.rights = Some(revision);
        }
        self.effects.merge(effects.clone());
        self.revision = revision;
    }

    /// Clears each effect merged at or below `revision` (5.10, point 3),
    /// and says whether any is left.
    fn clear_through(&mut self, revision: Revision) -> bool {
        let applied = |merged: &mut Option<Revision>| {
            if merged.is_some_and(|at| at <= revision) {
                *merged = None;
                true
            } else {
                false
            }
        };
        if applied(&mut self.data) {
            self.effects.data = false;
        }
        if applied(&mut self.attributes) {
            self.effects.attributes = false;
        }
        if applied(&mut self.entries) {
            self.effects.entries = None;
        }
        if applied(&mut self.rights) {
            self.effects.rights = None;
        }
        self.data.is_some()
            || self.attributes.is_some()
            || self.entries.is_some()
            || self.rights.is_some()
    }

    fn bytes(&self) -> usize {
        let names = match &self.effects.entries {
            None | Some(Entries::All) => 0,
            Some(Entries::Name(name)) => name.as_bytes().len(),
            Some(Entries::Rename { from, to }) => from.as_bytes().len() + to.as_bytes().len(),
        };
        SLOT_BYTES + names
    }
}

/// The overflow delivered and not yet acknowledged.
#[derive(Debug)]
struct OpenOverflow {
    revision: Revision,
    /// The nodes held when it was delivered.
    held: BTreeSet<NodeId>,
    /// The nodes reported failed under it.
    reported: BTreeSet<NodeId>,
}

/// Where the purge offered again after a failure is (5.10, point 5): the
/// node's staleness ends with the acknowledgement of the delivery that
/// carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reoffer {
    /// In the node's slot, merged at this revision.
    Slot(Revision),
    /// Folded into the overflow not yet delivered.
    NextOverflow,
    /// In the open overflow.
    OpenOverflow,
}

/// How a report on the stream is classed (5.10, point 6): the two
/// invalid classes are disjoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    /// It answers an open delivery, and acted.
    Valid,
    /// Invalid, and ignored: never a fault.
    Repeat,
    /// Invalid, and a layer fault.
    Fault,
}

#[derive(Debug)]
pub(super) struct Stream {
    bound: usize,
    held: BTreeSet<NodeId>,
    slots: BTreeMap<NodeId, Slot>,
    /// Each node's delivery not yet answered, by its revision.
    unanswered: BTreeMap<NodeId, Revision>,
    /// The latest revision delivered for each node.
    delivered_for: BTreeMap<NodeId, Revision>,
    open: Option<OpenOverflow>,
    /// The revision of the overflow waiting to be delivered.
    next_overflow: Option<Revision>,
    /// The latest overflow revision delivered.
    overflow_delivered: Option<Revision>,
    reoffers: BTreeMap<NodeId, Reoffer>,
    /// The last revision issued to this binding.
    last: Revision,
}

impl Stream {
    pub(super) const fn new(bound: usize) -> Self {
        Self {
            bound,
            held: BTreeSet::new(),
            slots: BTreeMap::new(),
            unanswered: BTreeMap::new(),
            delivered_for: BTreeMap::new(),
            open: None,
            next_overflow: None,
            overflow_delivered: None,
            reoffers: BTreeMap::new(),
            last: Revision(0),
        }
    }

    pub(super) fn hold(&mut self, node: NodeId) {
        self.held.insert(node);
    }

    pub(super) fn holds(&self, node: NodeId) -> bool {
        self.held.contains(&node)
    }

    /// Whether a failed purge of `node` here has not yet been followed by
    /// the acknowledgement of its re-offer.
    pub(super) fn stale(&self, node: NodeId) -> bool {
        self.reoffers.contains_key(&node)
    }

    /// Merges an event into `node`'s slot, if this binding holds the node,
    /// and folds the slots into the next overflow if they then pass the
    /// bound. Returns the revision the merge took.
    pub(super) fn push(&mut self, node: NodeId, scope: Scope, effects: &Effects) -> Revision {
        if !self.holds(node) {
            return self.last;
        }
        let revision = self.issue();
        self.slots
            .entry(node)
            .or_insert_with(|| Slot::new(scope))
            .merge(scope, effects, revision);
        let bytes: usize = self.slots.values().map(Slot::bytes).sum();
        if bytes > self.bound {
            // Every held node is gated and purged under the overflow, so
            // the slots it replaces lose nothing; deliveries already made
            // stay unanswered until the binding answers them.
            self.slots.clear();
            self.next_overflow = Some(revision);
            for reoffer in self.reoffers.values_mut() {
                if let Reoffer::Slot(_) = reoffer {
                    *reoffer = Reoffer::NextOverflow;
                }
            }
        }
        revision
    }

    pub(super) fn next_event(&mut self) -> Option<Event> {
        if self.open.is_none()
            && let Some(revision) = self.next_overflow.take()
        {
            self.open = Some(OpenOverflow {
                revision,
                held: self.held.clone(),
                reported: BTreeSet::new(),
            });
            self.overflow_delivered = Some(revision);
            for reoffer in self.reoffers.values_mut() {
                if *reoffer == Reoffer::NextOverflow {
                    *reoffer = Reoffer::OpenOverflow;
                }
            }
            return Some(Event::Overflow(Overflow { revision }));
        }
        let (node, slot) = self
            .slots
            .iter()
            .find(|(node, _)| !self.unanswered.contains_key(node))?;
        let event = Invalidation {
            node: *node,
            scope: slot.scope,
            effects: slot.effects.clone(),
            revision: slot.revision,
        };
        self.unanswered.insert(event.node, event.revision);
        self.delivered_for.insert(event.node, event.revision);
        Some(Event::Node(event))
    }

    /// Applies `ack` if it answers an open delivery (5.10, points 3 and
    /// 6).
    pub(super) fn ack(&mut self, ack: Ack) -> Answer {
        match ack {
            Ack::Node { node, revision } => {
                let answer = self.node_answer(node, revision);
                if answer == Answer::Valid {
                    self.unanswered.remove(&node);
                    if let Some(slot) = self.slots.get_mut(&node)
                        && !slot.clear_through(revision)
                    {
                        self.slots.remove(&node);
                    }
                    if self.reoffers.get(&node).is_some_and(
                        |reoffer| matches!(reoffer, Reoffer::Slot(at) if *at <= revision),
                    ) {
                        self.reoffers.remove(&node);
                    }
                }
                answer
            }
            Ack::Overflow { revision } => {
                let answer = self.overflow_answer(revision);
                if answer == Answer::Valid {
                    self.open = None;
                    self.reoffers
                        .retain(|_, reoffer| *reoffer != Reoffer::OpenOverflow);
                }
                answer
            }
        }
    }

    /// Applies a failure report if it answers an open delivery (5.10,
    /// points 5 and 6): the delivery is answered and the node's purge is
    /// offered again, widened to every effect, as a node event or, when
    /// its slot does not fit the bound, within the next overflow.
    pub(super) fn fail(&mut self, report: PurgeFailed) -> Answer {
        let (node, answer) = match report {
            PurgeFailed::Node { node, revision } => {
                let answer = self.node_answer(node, revision);
                if answer == Answer::Valid {
                    self.unanswered.remove(&node);
                }
                (node, answer)
            }
            PurgeFailed::OverflowNode { node, revision } => {
                let answer = self.overflow_node_answer(node, revision);
                if answer == Answer::Valid
                    && let Some(open) = self.open.as_mut()
                {
                    open.reported.insert(node);
                }
                (node, answer)
            }
        };
        if answer == Answer::Valid {
            let revision = self.push(node, Scope::Object, &EVERYTHING);
            let reoffer = if self.slots.contains_key(&node) {
                Reoffer::Slot(revision)
            } else {
                Reoffer::NextOverflow
            };
            self.reoffers.insert(node, reoffer);
        }
        answer
    }

    pub(super) fn gated(&self, node: NodeId) -> bool {
        self.holds(node)
            && (self.open.is_some()
                || self.next_overflow.is_some()
                || self.slots.contains_key(&node)
                || self.unanswered.contains_key(&node))
    }

    /// Classes a node report: valid if it names the node's unanswered
    /// delivery; a fault if the binding does not hold the node, never had
    /// a delivery for it, or names a revision above the latest delivered
    /// for it; otherwise a repeat (5.10, point 6).
    fn node_answer(&self, node: NodeId, revision: Revision) -> Answer {
        if self.unanswered.get(&node) == Some(&revision) && self.holds(node) {
            return Answer::Valid;
        }
        match self.delivered_for.get(&node) {
            Some(latest) if self.holds(node) && revision <= *latest => Answer::Repeat,
            _ => Answer::Fault,
        }
    }

    /// Classes an overflow acknowledgement: valid for the open overflow's
    /// revision; a fault if no overflow was delivered or the revision is
    /// above the latest delivered; otherwise a repeat.
    fn overflow_answer(&self, revision: Revision) -> Answer {
        if self
            .open
            .as_ref()
            .is_some_and(|open| open.revision == revision)
        {
            return Answer::Valid;
        }
        match self.overflow_delivered {
            Some(latest) if revision <= latest => Answer::Repeat,
            _ => Answer::Fault,
        }
    }

    /// Classes an `OverflowNode` report: valid for the open overflow's
    /// revision, a node held at its delivery, reported once; a fault if
    /// the overflow answer would be one, if the binding does not hold the
    /// node, or if it names a node not held at the delivery of an overflow
    /// still open; otherwise a repeat.
    fn overflow_node_answer(&self, node: NodeId, revision: Revision) -> Answer {
        let open = self.open.as_ref().filter(|open| open.revision == revision);
        match open {
            Some(open) if !open.held.contains(&node) => Answer::Fault,
            Some(open) if !open.reported.contains(&node) => Answer::Valid,
            Some(_) => Answer::Repeat,
            None if !self.holds(node) => Answer::Fault,
            None => match self.overflow_answer(revision) {
                Answer::Valid | Answer::Repeat => Answer::Repeat,
                Answer::Fault => Answer::Fault,
            },
        }
    }

    fn issue(&mut self) -> Revision {
        self.last = Revision(self.last.0.saturating_add(1));
        self.last
    }
}
