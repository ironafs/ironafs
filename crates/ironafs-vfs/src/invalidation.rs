// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The invalidation stream from the core to each binding (ADR 0004, C17;
//! N47, point 3).
//!
//! Events are coalesced per node, not queued one by one (C17, point 1): a
//! node's slot per binding holds the union of its pending [`Effects`],
//! each with the [`Revision`] at which it was last merged. A layer applies
//! an event's purge and then acknowledges the revision it applied
//! ([`Ack`]), which clears only effects merged at or below that revision
//! (C17, points 2 and 6), or reports a purge that failed
//! ([`PurgeFailed`]). When a binding's pending events reach their byte
//! bound it gets one [`Overflow`] instead, which gates and purges every
//! node it holds.
//!
//! The slots, their revisions, the gating of replies and the byte bound
//! are the core's; this module holds the values that cross the contract
//! and the one rule that makes the union representable: how two effects
//! merge and widen ([`Effects::merge`]).

use crate::{ContextId, Name};

#[cfg(test)]
mod tests;

/// A node a binding holds, by the id the core gave the layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u64);

/// A binding's event counter (C17, point 6; the contract specification,
/// 5.10, point 1). Each binding has one sequence, from 1: every merge into
/// any of its slots, and every overflow, takes the next revision, so
/// revisions are compared only within one binding. A delivery carries the
/// revision of its slot's latest merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// How far an event reaches (N47, point 3), from narrowest to widest. A
/// layer that does not understand an event handles it conservatively at
/// its scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// One object.
    Object,
    /// A directory and its entries.
    Directory,
    /// Every node of a volume.
    Volume,
    /// Every node served by one server.
    Server,
}

/// Which entries of a directory changed (C17, point 6; N47, point 3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Entries {
    /// One entry, by name.
    Name(Name),
    /// A rename within the directory, its two names paired.
    Rename {
        /// The entry's old name.
        from: Name,
        /// The entry's new name.
        to: Name,
    },
    /// Every entry. A layer purges every entry name it has handed its
    /// operating system under the directory; a layer that cannot list
    /// those names treats the purge as one it cannot complete (C17,
    /// points 5 and 6).
    All,
}

impl Entries {
    /// Whether a purge of these entries purges the entry named `name`.
    fn purges(&self, name: &Name) -> bool {
        match self {
            Self::Name(held) => held == name,
            Self::Rename { from, to } => from == name || to == name,
            Self::All => true,
        }
    }
}

/// Whose rights changed (C17, point 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rights {
    /// One credential context's.
    Context(ContextId),
    /// Every context's.
    All,
}

/// The pending effects of a node's slot: the union of the events merged
/// into it (C17, point 6).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Effects {
    /// The file's data changed.
    pub data: bool,
    /// The node's attributes changed.
    pub attributes: bool,
    /// Entries of the directory changed, if any.
    pub entries: Option<Entries>,
    /// Rights on the node changed, if any.
    pub rights: Option<Rights>,
}

impl Effects {
    /// No effect.
    pub const NONE: Self = Self {
        data: false,
        attributes: false,
        entries: None,
        rights: None,
    };

    /// Merges `other` into these effects, so that they cover both (C17,
    /// point 6). Two different entry effects widen to [`Entries::All`],
    /// including a paired rename merged with any other names; two different
    /// contexts' rights widen to [`Rights::All`]. Nothing is dropped:
    /// afterwards `self` [covers](Self::covers) what it was and `other`.
    pub fn merge(&mut self, other: Self) {
        self.data |= other.data;
        self.attributes |= other.attributes;
        self.entries = match (self.entries.take(), other.entries) {
            (None, theirs) => theirs,
            (ours, None) => ours,
            (Some(ours), Some(theirs)) if ours == theirs => Some(ours),
            (Some(_), Some(_)) => Some(Entries::All),
        };
        self.rights = match (self.rights, other.rights) {
            (None, theirs) => theirs,
            (ours, None) => ours,
            (Some(ours), Some(theirs)) if ours == theirs => Some(ours),
            (Some(_), Some(_)) => Some(Rights::All),
        };
    }

    /// Whether a purge of these effects also purges everything `other`
    /// asks for. An entry purge purges the names it holds: one name, both
    /// names of a rename in either direction, or every entry.
    #[must_use]
    pub fn covers(&self, other: &Self) -> bool {
        let entries = match (&self.entries, &other.entries) {
            (_, None) | (Some(Entries::All), Some(_)) => true,
            (None | Some(Entries::Name(_) | Entries::Rename { .. }), Some(Entries::All))
            | (None, Some(_)) => false,
            (Some(ours), Some(Entries::Name(theirs))) => ours.purges(theirs),
            (Some(ours), Some(Entries::Rename { from, to })) => {
                ours.purges(from) && ours.purges(to)
            }
        };
        let rights = match (self.rights, other.rights) {
            (_, None) | (Some(Rights::All), Some(_)) => true,
            (None, Some(_)) => false,
            (Some(ours), Some(theirs)) => ours == theirs,
        };
        (self.data || !other.data) && (self.attributes || !other.attributes) && entries && rights
    }
}

/// A node's pending invalidation, as the core delivers it to a binding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Invalidation {
    /// The node.
    pub node: NodeId,
    /// How far the change reaches.
    pub scope: Scope,
    /// What to purge.
    pub effects: Effects,
    /// The slot's revision this delivery carries.
    pub revision: Revision,
}

/// A binding's pending events passed their byte bound (C17, point 6;
/// C39, point 3): every node the binding holds is gated and purged, or
/// handled as a purge that cannot be completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Overflow {
    /// The overflow's own revision.
    pub revision: Revision,
}

/// One event of a binding's invalidation stream.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// One node's pending effects.
    Node(Invalidation),
    /// The binding overflowed.
    Overflow(Overflow),
}

/// A binding's acknowledgement that it applied an event's purge, sent only
/// after the operating system's purge was applied (C17, point 2). Like a
/// [`PurgeFailed`] report it is untrusted and acts only if it answers an
/// open delivery; a repeat is ignored (the contract specification, 5.10,
/// point 6). It clears effects merged at or below the revision it names;
/// an event that arrived during the purge stays pending under its higher
/// revision (C17, point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ack {
    /// A node's purge, up to `revision`.
    Node {
        /// The node.
        node: NodeId,
        /// The revision applied.
        revision: Revision,
    },
    /// The overflow's purge.
    Overflow {
        /// The overflow's revision applied.
        revision: Revision,
    },
}

/// A binding's report that the operating system's purge of a delivery
/// failed: sent instead of an [`Ack`], never with one (the contract
/// specification, 5.10, point 5; C17, point 5).
///
/// Under an overflow, the binding reports each node that failed as an
/// `OverflowNode` and then acknowledges the overflow, which clears it for
/// every node not reported. A report is untrusted: the core acts on it only
/// if it answers an open delivery, one it made to this binding and that no
/// acknowledgement or report has answered yet. A second answer to a
/// delivery, or a revision at or below the latest the core delivered for
/// that node or overflow, is a repeat: ignored, never acted on, and never
/// a fault (the contract specification, 5.10, point 6). Only a
/// demonstrated purge failure revokes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PurgeFailed {
    /// The purge of a delivery of the node's slot failed.
    Node {
        /// The node.
        node: NodeId,
        /// The revision the delivery carried.
        revision: Revision,
    },
    /// The purge of one node the binding holds failed under an overflow.
    OverflowNode {
        /// The node.
        node: NodeId,
        /// The overflow's revision.
        revision: Revision,
    },
}

impl PurgeFailed {
    /// The node whose purge failed.
    #[must_use]
    pub const fn node(self) -> NodeId {
        match self {
            Self::Node { node, .. } | Self::OverflowNode { node, .. } => node,
        }
    }

    /// The revision of the delivery, or of the overflow, it reports.
    #[must_use]
    pub const fn revision(self) -> Revision {
        match self {
            Self::Node { revision, .. } | Self::OverflowNode { revision, .. } => revision,
        }
    }
}
