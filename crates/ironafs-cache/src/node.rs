// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The node table (ADR 0004, C11): the node ids the core hands the layers,
//! what each one names, and what keeps it in the table.
//!
//! - **Ids.** A [`NodeId`] is 64 bits, allocated in increasing order from
//!   1 and never reused within one table (C11, point 2). The daemon keeps
//!   one table for its whole life, so ids are unique within its instance. When the counter
//!   would pass `u64::MAX`, new nodes are refused, never wrapped.
//! - **Keys.** A node inside a volume is keyed by its [`FileId`] and its
//!   anchor: the mount-point or dynamic-root entry node the volume was
//!   entered through (C11, point 1). The dynamic root and its entry nodes
//!   are IronAFS's own (namespace specification, 5.6.3, point 5).
//! - **Root nodes** are the root directories of entered volumes. A layer is
//!   never given one: it holds the mount-point or entry node, and each
//!   context's binding leads from that node to the root node it resolved
//!   (5.6.2, points 6 and 9), so two contexts that resolve one mount point
//!   to different volumes get different root nodes (C11, point 9). A root
//!   node holds its anchor chain (5.6.1, point 4).
//! - **Aliases.** A reverse index from file id to every node with that file
//!   id (C11, point 3), ordered so a volume's nodes are one range.
//! - **Retention.** A record names only nodes older than itself: its
//!   parent and containing root, or a root's anchor. A node stays while a
//!   record names it, while a context holds it, or if it is the dynamic
//!   root (5.6.1, point 8). Only such a node's identity stays: its status
//!   and data are the caches' and may be evicted (C11, point 4).
//! - **Charges.** A pin or a binding is held by one context, and holds the
//!   node and every node it names, transitively. Each node a context holds
//!   is charged once to that context, against the budget the caller gives
//!   (C11, point 10; 5.6.1, point 8). A rebinding is charged its net
//!   change. A new pin or binding over the budget,
//!   or a new node in a full table where nothing can be evicted, is refused
//!   with [`NodeError::Busy`]. A held node is never evicted to make room.
//!
//! **Bounds.** At most [`Limits::nodes`] records (at most [`MAX_NODES`]),
//! one key and one reverse-index entry each; a root record carries one
//! inline [`MountChain`] of at most [`MAX_MOUNT_DEPTH`] identities. At most
//! [`Limits::contexts`] contexts (at most [`MAX_CONTEXTS`]) hold anything;
//! each holds, pins and binds at most as many nodes as its charge, which
//! is at most its budget and at most the table's size.

mod chain;

pub use chain::{MAX_MOUNT_DEPTH, MountChain};

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;
use core::ops::Bound;

use ironafs_vfs::NodeId;

use crate::{CellIndex, ContextKey, FileId, PathKind, Uniquifier, Version, Vnode, VolumeId};

#[cfg(test)]
mod tests;

/// The most nodes a table may be built for: 2^24 [OWN: far above any
/// pinned working set one machine's kernels hold, and small enough that
/// the table's memory stays bounded at a few gigabytes in the worst case].
pub const MAX_NODES: u32 = 1 << 24;

/// The most contexts that may hold nodes at once: 2^16 [OWN: far above the
/// credential contexts one machine has at once].
pub const MAX_CONTEXTS: u32 = 1 << 16;

/// The id of the dynamic root, the first id allocated.
pub const DYNAMIC_ROOT: NodeId = NodeId(1);

/// A table's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limits {
    /// The most records, the dynamic root included: 2 to [`MAX_NODES`].
    pub nodes: u32,
    /// The most contexts holding nodes at once: 1 to [`MAX_CONTEXTS`].
    pub contexts: u32,
}

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// `/afs`.
    DynamicRoot,
    /// A dynamic-root name: a cell on one path (5.6.3, point 5).
    Entry {
        /// The cell.
        cell: CellIndex,
        /// The path it enters.
        path: PathKind,
    },
    /// The root directory of a volume entered through `anchor`.
    Root {
        /// The root directory's file id.
        file: FileId,
        /// The mount-point or entry node it was entered through.
        anchor: NodeId,
    },
    /// A node found by lookup in a directory of a volume entered through
    /// `anchor`.
    Child {
        /// Its file id.
        file: FileId,
        /// The mount-point or entry node its volume was entered through.
        anchor: NodeId,
    },
}

/// Why the node table refused a change. A refusal changes no pin, binding
/// or charge, and makes or evicts no record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeError {
    /// Crossing would take a root past [`MAX_MOUNT_DEPTH`] mount points.
    MountDepthExceeded,
    /// Crossing would repeat a mount point already in the chain.
    MountCycle,
    /// Over the context's budget, a full table with nothing to evict, too
    /// many contexts, or a pin count at its maximum: retry later.
    Busy,
    /// No node id is left: the counter reached `u64::MAX`.
    IdsExhausted,
    /// The node is not in the table.
    UnknownNode,
    /// A volume id of 0, which names no volume.
    VersionAbsent,
    /// The node cannot anchor a volume: only a mount-point node or an entry
    /// node can.
    NotAnAnchor,
    /// An entry node's root must be in the entry's own cell.
    CellMismatch,
    /// The node given as a directory is not in the volume of the root
    /// given.
    NotInVolume,
    /// The key names a node of another kind: a directory entry naming its
    /// volume's root, or a root whose key a found node already has.
    KindConflict,
    /// The file is already in the table under another parent.
    ParentChanged,
    /// The context holds no pin on the node.
    NotPinned,
    /// Limits out of range.
    InvalidLimits,
}

impl fmt::Display for NodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MountDepthExceeded => "too many mount points crossed",
            Self::MountCycle => "mount point already crossed",
            Self::Busy => "busy, try again",
            Self::IdsExhausted => "node ids exhausted",
            Self::UnknownNode => "unknown node",
            Self::VersionAbsent => "volume version absent",
            Self::NotAnAnchor => "node cannot anchor a volume",
            Self::CellMismatch => "root in another cell",
            Self::NotInVolume => "directory not in the root's volume",
            Self::KindConflict => "node of another kind",
            Self::ParentChanged => "file under another parent",
            Self::NotPinned => "node not pinned",
            Self::InvalidLimits => "limits out of range",
        })
    }
}

impl core::error::Error for NodeError {}

/// The bounded node table.
#[derive(Debug)]
pub struct NodeTable {
    limits: Limits,
    /// The next id to allocate; `None` once `u64::MAX` is allocated.
    next_id: Option<u64>,
    records: BTreeMap<NodeId, Record>,
    keys: BTreeMap<Key, NodeId>,
    /// The reverse index: (file id, node) for every node with a file id.
    aliases: BTreeSet<(FileId, NodeId)>,
    /// Nodes no record names, no context holds, and not the dynamic root.
    evictable: BTreeSet<NodeId>,
    contexts: BTreeMap<ContextKey, Holder>,
}

/// What a node is keyed by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    DynamicRoot,
    Entry(CellIndex, PathKind),
    File(FileId, NodeId),
}

#[derive(Debug)]
struct Record {
    kind: NodeKind,
    /// The parent, for an entry or a child.
    parent: Option<NodeId>,
    /// The containing root, for a child.
    containing_root: Option<NodeId>,
    /// The anchor chain, for a root.
    chain: Option<Box<MountChain>>,
    /// How many records name this one.
    named_by: u32,
    /// How many contexts hold it.
    held_by: u32,
}

/// What one context holds.
#[derive(Debug, Default)]
struct Holder {
    /// For each node held, how many of the context's pins and bindings hold
    /// it. The charge is the number of entries.
    holds: BTreeMap<NodeId, u32>,
    /// Pin counts.
    pins: BTreeMap<NodeId, u32>,
    /// Bindings: from the mount-point or entry node to the root node and
    /// the version chosen.
    bindings: BTreeMap<NodeId, (NodeId, Version)>,
}

impl NodeTable {
    /// A table holding only the dynamic root, [`DYNAMIC_ROOT`].
    ///
    /// # Errors
    ///
    /// [`NodeError::InvalidLimits`] if a limit is out of range.
    pub fn new(limits: Limits) -> Result<Self, NodeError> {
        if !(2..=MAX_NODES).contains(&limits.nodes)
            || !(1..=MAX_CONTEXTS).contains(&limits.contexts)
        {
            return Err(NodeError::InvalidLimits);
        }
        let mut table = Self {
            limits,
            next_id: Some(DYNAMIC_ROOT.0),
            records: BTreeMap::new(),
            keys: BTreeMap::new(),
            aliases: BTreeSet::new(),
            evictable: BTreeSet::new(),
            contexts: BTreeMap::new(),
        };
        let root = table.make(Key::DynamicRoot, NodeKind::DynamicRoot, None, None, None)?;
        // The dynamic root is never evicted.
        table.evictable.remove(&root);
        Ok(table)
    }

    /// The number of records, the dynamic root included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Never true: the dynamic root is always present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// What `node` is, or `None` if it is not in the table.
    #[must_use]
    pub fn kind(&self, node: NodeId) -> Option<NodeKind> {
        self.records.get(&node).map(|record| record.kind)
    }

    /// The answer to `..` (namespace specification, 5.6.4, point 2): the
    /// dynamic root for the dynamic root and an entry node, the recorded
    /// parent for a node found by lookup, and `None` for a root node, which
    /// no layer holds, or a node not in the table.
    #[must_use]
    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        let record = self.records.get(&node)?;
        match record.kind {
            NodeKind::DynamicRoot => Some(DYNAMIC_ROOT),
            NodeKind::Root { .. } => None,
            NodeKind::Entry { .. } | NodeKind::Child { .. } => record.parent,
        }
    }

    /// The root node of the volume `node` was found in.
    #[must_use]
    pub fn containing_root(&self, node: NodeId) -> Option<NodeId> {
        self.records.get(&node)?.containing_root
    }

    /// A root node's anchor chain.
    #[must_use]
    pub fn chain(&self, root: NodeId) -> Option<&MountChain> {
        self.records.get(&root)?.chain.as_deref()
    }

    /// The entry node for `cell` on `path`, made if absent.
    ///
    /// # Errors
    ///
    /// [`NodeError::Busy`] or [`NodeError::IdsExhausted`] if a node is made
    /// and none can be.
    pub fn entry(&mut self, cell: CellIndex, path: PathKind) -> Result<NodeId, NodeError> {
        let key = Key::Entry(cell, path);
        if let Some(&node) = self.keys.get(&key) {
            return Ok(node);
        }
        self.make(
            key,
            NodeKind::Entry { cell, path },
            Some(DYNAMIC_ROOT),
            None,
            None,
        )
    }

    /// The node of the file `vnode`, `uniquifier` found by lookup in
    /// directory `parent` of the volume whose root node is `root`, made if
    /// absent. `parent` is `root`'s anchor for a name in the root
    /// directory, else a node found in that volume.
    ///
    /// # Errors
    ///
    /// [`NodeError::UnknownNode`]; [`NodeError::NotInVolume`] if `root` is
    /// not a root node or `parent` is not in its volume;
    /// [`NodeError::KindConflict`] if the file is the root directory;
    /// [`NodeError::ParentChanged`] if the node exists under another
    /// parent; [`NodeError::Busy`] or [`NodeError::IdsExhausted`] if a node
    /// is made and none can be.
    pub fn child(
        &mut self,
        parent: NodeId,
        root: NodeId,
        vnode: Vnode,
        uniquifier: Uniquifier,
    ) -> Result<NodeId, NodeError> {
        let root_record = self.records.get(&root).ok_or(NodeError::UnknownNode)?;
        let parent_record = self.records.get(&parent).ok_or(NodeError::UnknownNode)?;
        let NodeKind::Root {
            file: root_file,
            anchor,
        } = root_record.kind
        else {
            return Err(NodeError::NotInVolume);
        };
        if parent != anchor && parent_record.containing_root != Some(root) {
            return Err(NodeError::NotInVolume);
        }
        let file = FileId {
            vnode,
            uniquifier,
            ..root_file
        };
        let key = Key::File(file, anchor);
        if let Some(&node) = self.keys.get(&key) {
            let record = self.records.get(&node).ok_or(NodeError::UnknownNode)?;
            return match record.kind {
                NodeKind::Child { .. } if record.parent == Some(parent) => Ok(node),
                NodeKind::Child { .. } => Err(NodeError::ParentChanged),
                _ => Err(NodeError::KindConflict),
            };
        }
        self.make(
            key,
            NodeKind::Child { file, anchor },
            Some(parent),
            Some(root),
            None,
        )
    }

    /// Crosses `from`, a mount-point node or an entry node, into the volume
    /// root `root` that `context` resolved it to with `version`, and binds
    /// `from` to that root node in `context` (5.6.2, points 6 and 9). The
    /// binding replaces any earlier one of `context` for `from`. The root
    /// node is charged to `context` against `budget`.
    ///
    /// # Errors
    ///
    /// [`NodeError::UnknownNode`]; [`NodeError::NotAnAnchor`];
    /// [`NodeError::VersionAbsent`] for volume 0;
    /// [`NodeError::CellMismatch`] for an entry node and a root in another
    /// cell; [`NodeError::MountDepthExceeded`] or
    /// [`NodeError::MountCycle`] (5.6.1, point 1);
    /// [`NodeError::KindConflict`]; [`NodeError::Busy`];
    /// [`NodeError::IdsExhausted`].
    pub fn cross(
        &mut self,
        context: ContextKey,
        budget: u32,
        from: NodeId,
        root: FileId,
        version: Version,
    ) -> Result<NodeId, NodeError> {
        let from_record = self.records.get(&from).ok_or(NodeError::UnknownNode)?;
        let chain = match from_record.kind {
            NodeKind::Entry { cell, .. } => {
                if root.volume.0 == 0 {
                    return Err(NodeError::VersionAbsent);
                }
                if root.cell != cell {
                    return Err(NodeError::CellMismatch);
                }
                MountChain::empty()
            }
            NodeKind::Child { file, .. } => {
                if root.volume.0 == 0 {
                    return Err(NodeError::VersionAbsent);
                }
                let containing = from_record
                    .containing_root
                    .and_then(|node| self.chain(node))
                    .ok_or(NodeError::UnknownNode)?;
                containing.extended(file)?
            }
            NodeKind::DynamicRoot | NodeKind::Root { .. } => return Err(NodeError::NotAnAnchor),
        };
        let key = Key::File(root, from);
        let existing = match self.keys.get(&key) {
            Some(&node) => {
                let record = self.records.get(&node).ok_or(NodeError::UnknownNode)?;
                if record.chain.as_deref() != Some(&chain) {
                    return Err(NodeError::KindConflict);
                }
                Some(node)
            }
            None => None,
        };
        let old = self
            .contexts
            .get(&context)
            .and_then(|holder| holder.bindings.get(&from).map(|&(root, _)| root));
        if let Some(node) = existing
            && old == Some(node)
        {
            self.set_binding(context, from, node, version);
            return Ok(node);
        }
        // Every check runs before the root is made, so a refusal evicts
        // nothing and spends no id. A root names only its anchor, so a new
        // root holds itself and what `from` holds.
        let (prospective, unmade) = match existing {
            Some(node) => (self.closure(node), 0),
            None => (self.closure(from), 1),
        };
        self.check_hold(context, budget, &prospective, unmade, old)?;
        let node = match existing {
            Some(node) => node,
            None => self.make(
                key,
                NodeKind::Root {
                    file: root,
                    anchor: from,
                },
                None,
                None,
                Some(Box::new(chain)),
            )?,
        };
        let closure = self.closure(node);
        self.apply_hold(context, closure);
        if let Some(old_root) = old {
            self.release(context, old_root);
        }
        self.set_binding(context, from, node, version);
        Ok(node)
    }

    /// `context`'s binding of `from`: the root node and the version chosen.
    #[must_use]
    pub fn binding(&self, context: ContextKey, from: NodeId) -> Option<(NodeId, Version)> {
        self.contexts.get(&context)?.bindings.get(&from).copied()
    }

    /// Ends `context`'s binding of `from`, if any; returns whether there
    /// was one.
    pub fn unbind(&mut self, context: ContextKey, from: NodeId) -> bool {
        let Some((root, _)) = self
            .contexts
            .get_mut(&context)
            .and_then(|holder| holder.bindings.remove(&from))
        else {
            return false;
        };
        self.release(context, root);
        true
    }

    /// Pins `node` for `context`, charged against `budget`.
    ///
    /// # Errors
    ///
    /// [`NodeError::UnknownNode`]; [`NodeError::Busy`] over the budget,
    /// with too many contexts, or at the pin count's maximum.
    pub fn pin(&mut self, context: ContextKey, budget: u32, node: NodeId) -> Result<(), NodeError> {
        if !self.records.contains_key(&node) {
            return Err(NodeError::UnknownNode);
        }
        let count = self
            .contexts
            .get(&context)
            .and_then(|holder| holder.pins.get(&node).copied())
            .unwrap_or(0);
        let next = count.checked_add(1).ok_or(NodeError::Busy)?;
        if count == 0 {
            self.hold(context, budget, node)?;
        }
        if let Some(holder) = self.contexts.get_mut(&context) {
            holder.pins.insert(node, next);
        }
        Ok(())
    }

    /// Removes one of `context`'s pins on `node`.
    ///
    /// # Errors
    ///
    /// [`NodeError::NotPinned`] if `context` holds none.
    pub fn unpin(&mut self, context: ContextKey, node: NodeId) -> Result<(), NodeError> {
        let holder = self
            .contexts
            .get_mut(&context)
            .ok_or(NodeError::NotPinned)?;
        let count = holder.pins.get_mut(&node).ok_or(NodeError::NotPinned)?;
        *count = count.saturating_sub(1);
        if *count == 0 {
            holder.pins.remove(&node);
            self.release(context, node);
        }
        Ok(())
    }

    /// Ends every pin and binding of `context`.
    pub fn release_context(&mut self, context: ContextKey) {
        let Some(holder) = self.contexts.remove(&context) else {
            return;
        };
        for node in holder.holds.keys() {
            self.unhold_one(*node);
        }
    }

    /// How many nodes are charged to `context`.
    #[must_use]
    pub fn charge(&self, context: ContextKey) -> u32 {
        self.contexts.get(&context).map_or(0, |holder| {
            u32::try_from(holder.holds.len()).unwrap_or(u32::MAX)
        })
    }

    /// Every node with file id `file`: the aliases a callback break or a
    /// local change reaches (C11, point 3), in increasing id order.
    #[must_use]
    pub fn aliases(&self, file: &FileId) -> Vec<NodeId> {
        self.aliases
            .range((*file, NodeId(0))..=(*file, NodeId(u64::MAX)))
            .map(|&(_, node)| node)
            .collect()
    }

    /// Every node of `volume` in `cell`, in file id order.
    #[must_use]
    pub fn volume_nodes(&self, cell: CellIndex, volume: VolumeId) -> Vec<NodeId> {
        let first = FileId {
            cell,
            volume,
            vnode: Vnode(0),
            uniquifier: Uniquifier(0),
        };
        let last = FileId {
            cell,
            volume,
            vnode: Vnode(u32::MAX),
            uniquifier: Uniquifier(u32::MAX),
        };
        self.aliases
            .range((
                Bound::Included((first, NodeId(0))),
                Bound::Included((last, NodeId(u64::MAX))),
            ))
            .map(|&(_, node)| node)
            .collect()
    }

    /// Makes a record, evicting one unheld node no record names if the
    /// table is full, never one the new record names.
    fn make(
        &mut self,
        key: Key,
        kind: NodeKind,
        parent: Option<NodeId>,
        containing_root: Option<NodeId>,
        chain: Option<Box<MountChain>>,
    ) -> Result<NodeId, NodeError> {
        let anchor = match kind {
            NodeKind::Root { anchor, .. } => Some(anchor),
            _ => None,
        };
        let named = [parent, containing_root, anchor];
        let id = self.next_id.ok_or(NodeError::IdsExhausted)?;
        let full = u32::try_from(self.records.len()).map_or(true, |len| len >= self.limits.nodes);
        if full {
            let victim = self
                .evictable
                .iter()
                .copied()
                .find(|node| !named.contains(&Some(*node)))
                .ok_or(NodeError::Busy)?;
            self.evict(victim);
        }
        let node = NodeId(id);
        self.next_id = id.checked_add(1);
        for target in named.into_iter().flatten() {
            if let Some(record) = self.records.get_mut(&target) {
                record.named_by = record.named_by.saturating_add(1);
                self.evictable.remove(&target);
            }
        }
        if let NodeKind::Root { file, .. } | NodeKind::Child { file, .. } = kind {
            self.aliases.insert((file, node));
        }
        self.records.insert(
            node,
            Record {
                kind,
                parent,
                containing_root,
                chain,
                named_by: 0,
                held_by: 0,
            },
        );
        self.keys.insert(key, node);
        self.evictable.insert(node);
        Ok(node)
    }

    /// Removes an evictable record and stops it naming others.
    fn evict(&mut self, node: NodeId) {
        self.evictable.remove(&node);
        let Some(record) = self.records.remove(&node) else {
            return;
        };
        let key = match record.kind {
            NodeKind::DynamicRoot => Key::DynamicRoot,
            NodeKind::Entry { cell, path } => Key::Entry(cell, path),
            NodeKind::Root { file, anchor } | NodeKind::Child { file, anchor } => {
                self.aliases.remove(&(file, node));
                Key::File(file, anchor)
            }
        };
        self.keys.remove(&key);
        for target in Self::named(&record) {
            if let Some(named) = self.records.get_mut(&target) {
                named.named_by = named.named_by.saturating_sub(1);
                if named.named_by == 0 && named.held_by == 0 && target != DYNAMIC_ROOT {
                    self.evictable.insert(target);
                }
            }
        }
    }

    /// The nodes a record names.
    fn named(record: &Record) -> impl Iterator<Item = NodeId> {
        let anchor = match record.kind {
            NodeKind::Root { anchor, .. } => Some(anchor),
            _ => None,
        };
        [record.parent, record.containing_root, anchor]
            .into_iter()
            .flatten()
    }

    /// `node` and every node it names, transitively. Every record names
    /// only older nodes, so the walk ends; it visits each node once.
    fn closure(&self, node: NodeId) -> BTreeSet<NodeId> {
        let mut seen = BTreeSet::new();
        let mut stack = Vec::from([node]);
        while let Some(next) = stack.pop() {
            if !seen.insert(next) {
                continue;
            }
            if let Some(record) = self.records.get(&next) {
                stack.extend(Self::named(record));
            }
        }
        seen
    }

    /// Adds one hold of `context` on `node` and everything it names,
    /// charging each newly held node against `budget`.
    fn hold(&mut self, context: ContextKey, budget: u32, node: NodeId) -> Result<(), NodeError> {
        let closure = self.closure(node);
        self.check_hold(context, budget, &closure, 0, None)?;
        self.apply_hold(context, closure);
        Ok(())
    }

    /// Whether `context` may hold `closure` and `unmade` nodes not yet in
    /// the table within `budget`, once its hold on `releasing` (a root it
    /// is rebinding away from) is dropped: the net charge is compared, so a
    /// rebinding is never charged for both roots at once.
    fn check_hold(
        &self,
        context: ContextKey,
        budget: u32,
        closure: &BTreeSet<NodeId>,
        unmade: usize,
        releasing: Option<NodeId>,
    ) -> Result<(), NodeError> {
        let Some(holder) = self.contexts.get(&context) else {
            let contexts = u32::try_from(self.contexts.len()).unwrap_or(u32::MAX);
            if contexts >= self.limits.contexts {
                return Err(NodeError::Busy);
            }
            let total = closure.len().checked_add(unmade).ok_or(NodeError::Busy)?;
            return Self::within(total, budget);
        };
        let dropped: BTreeSet<NodeId> = releasing
            .map_or_default(|root| self.closure(root))
            .into_iter()
            .filter(|node| holder.holds.get(node) == Some(&1))
            .collect();
        let fresh = closure
            .iter()
            .filter(|node| !holder.holds.contains_key(node) || dropped.contains(node))
            .count();
        let total = holder
            .holds
            .len()
            .checked_sub(dropped.len())
            .and_then(|kept| kept.checked_add(fresh))
            .and_then(|total| total.checked_add(unmade))
            .ok_or(NodeError::Busy)?;
        Self::within(total, budget)
    }

    fn within(total: usize, budget: u32) -> Result<(), NodeError> {
        if u32::try_from(total).map_or(true, |total| total > budget) {
            Err(NodeError::Busy)
        } else {
            Ok(())
        }
    }

    /// Adds one hold of `context` on every node of `closure`.
    fn apply_hold(&mut self, context: ContextKey, closure: BTreeSet<NodeId>) {
        let holder = self.contexts.entry(context).or_default();
        for target in closure {
            let count = holder.holds.entry(target).or_insert(0);
            *count = count.saturating_add(1);
            if *count == 1
                && let Some(record) = self.records.get_mut(&target)
            {
                record.held_by = record.held_by.saturating_add(1);
                self.evictable.remove(&target);
            }
        }
    }

    /// Removes one hold of `context` on `node` and everything it names.
    fn release(&mut self, context: ContextKey, node: NodeId) {
        let closure = self.closure(node);
        let Some(holder) = self.contexts.get_mut(&context) else {
            return;
        };
        let mut dropped = Vec::new();
        for target in closure {
            if let Some(count) = holder.holds.get_mut(&target) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    holder.holds.remove(&target);
                    dropped.push(target);
                }
            }
        }
        if holder.holds.is_empty() {
            self.contexts.remove(&context);
        }
        for target in dropped {
            self.unhold_one(target);
        }
    }

    /// One context stopped holding `node`.
    fn unhold_one(&mut self, node: NodeId) {
        if let Some(record) = self.records.get_mut(&node) {
            record.held_by = record.held_by.saturating_sub(1);
            if record.held_by == 0 && record.named_by == 0 && node != DYNAMIC_ROOT {
                self.evictable.insert(node);
            }
        }
    }

    fn set_binding(&mut self, context: ContextKey, from: NodeId, root: NodeId, version: Version) {
        if let Some(holder) = self.contexts.get_mut(&context) {
            holder.bindings.insert(from, (root, version));
        }
    }
}
