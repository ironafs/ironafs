// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The node table's rules, one at a time (ADR 0004, C11; namespace
//! specification, 5.6.1 to 5.6.4).

use alloc::vec;

use ironafs_vfs::NodeId;

use super::{DYNAMIC_ROOT, Limits, MAX_CONTEXTS, MAX_NODES, NodeError, NodeKind, NodeTable};
use crate::identity::fixture::{CELL, context, file};
use crate::{CellIndex, FileId, PathKind, Uniquifier, Version, Vnode, VolumeId};

const ROOT_CELL: VolumeId = VolumeId(100);
const MOUNTED: VolumeId = VolumeId(200);
const BIG: u32 = 1_000;

fn root_of(volume: VolumeId) -> FileId {
    file(volume, 1)
}

fn table(nodes: u32) -> NodeTable {
    NodeTable::new(Limits { nodes, contexts: 8 }).unwrap()
}

/// The dynamic root, the cell's read-only entry node and its root node in
/// context 1.
fn entered(table: &mut NodeTable) -> (NodeId, NodeId) {
    let entry = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let root = table
        .cross(
            context(1),
            BIG,
            entry,
            root_of(ROOT_CELL),
            Version::ReadOnly,
        )
        .unwrap();
    (entry, root)
}

#[test]
fn new_holds_only_the_dynamic_root() {
    let table = table(4);
    assert_eq!(table.len(), 1);
    assert_eq!(table.kind(DYNAMIC_ROOT), Some(NodeKind::DynamicRoot));
    assert_eq!(table.parent(DYNAMIC_ROOT), Some(DYNAMIC_ROOT));
}

#[test]
fn new_refuses_limits_out_of_range() {
    for (nodes, contexts) in [(1, 1), (MAX_NODES + 1, 1), (2, 0), (2, MAX_CONTEXTS + 1)] {
        assert_eq!(
            NodeTable::new(Limits { nodes, contexts }).unwrap_err(),
            NodeError::InvalidLimits,
            "{nodes} {contexts}"
        );
    }
    assert!(
        NodeTable::new(Limits {
            nodes: MAX_NODES,
            contexts: MAX_CONTEXTS
        })
        .is_ok()
    );
}

#[test]
fn entry_is_one_node_per_cell_and_path() {
    let mut table = table(8);
    let read_only = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let read_write = table.entry(CELL, PathKind::ReadWrite).unwrap();
    assert_ne!(read_only, read_write);
    assert_eq!(table.entry(CELL, PathKind::ReadOnly).unwrap(), read_only);
    assert_eq!(table.parent(read_only), Some(DYNAMIC_ROOT));
    assert_eq!(
        table.kind(read_write),
        Some(NodeKind::Entry {
            cell: CELL,
            path: PathKind::ReadWrite
        })
    );
}

#[test]
fn cross_from_an_entry_makes_a_root_with_an_empty_chain() {
    let mut table = table(8);
    let (entry, root) = entered(&mut table);
    assert_eq!(
        table.kind(root),
        Some(NodeKind::Root {
            file: root_of(ROOT_CELL),
            anchor: entry
        })
    );
    assert_eq!(table.chain(root).unwrap().depth(), 0);
    assert_eq!(table.parent(root), None);
    assert_eq!(
        table.binding(context(1), entry),
        Some((root, Version::ReadOnly))
    );
}

#[test]
fn child_of_the_root_directory_has_the_anchor_as_parent() {
    let mut table = table(8);
    let (entry, root) = entered(&mut table);
    let child = table.child(entry, root, Vnode(5), Uniquifier(9)).unwrap();
    assert_eq!(table.parent(child), Some(entry));
    assert_eq!(table.containing_root(child), Some(root));
    let grandchild = table.child(child, root, Vnode(6), Uniquifier(1)).unwrap();
    assert_eq!(table.parent(grandchild), Some(child));
    assert_eq!(table.containing_root(grandchild), Some(root));
    assert_eq!(
        table.child(entry, root, Vnode(5), Uniquifier(9)).unwrap(),
        child
    );
}

#[test]
fn child_refuses_the_volume_root_as_an_entry() {
    let mut table = table(8);
    let (entry, root) = entered(&mut table);
    assert_eq!(
        table.child(entry, root, Vnode(1), Uniquifier(1)),
        Err(NodeError::KindConflict)
    );
}

#[test]
fn child_refuses_a_known_file_under_another_parent() {
    let mut table = table(8);
    let (entry, root) = entered(&mut table);
    let directory = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    table
        .child(directory, root, Vnode(6), Uniquifier(1))
        .unwrap();
    assert_eq!(
        table.child(entry, root, Vnode(6), Uniquifier(1)),
        Err(NodeError::ParentChanged)
    );
}

#[test]
fn child_refuses_a_parent_outside_the_volume() {
    let mut table = table(8);
    let (entry, root) = entered(&mut table);
    assert_eq!(
        table.child(DYNAMIC_ROOT, root, Vnode(5), Uniquifier(1)),
        Err(NodeError::NotInVolume)
    );
    assert_eq!(
        table.child(entry, entry, Vnode(5), Uniquifier(1)),
        Err(NodeError::NotInVolume)
    );
    assert_eq!(
        table.child(entry, NodeId(99), Vnode(5), Uniquifier(1)),
        Err(NodeError::UnknownNode)
    );
}

#[test]
fn cross_records_the_mount_point_in_the_chain() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let inner = table
        .cross(context(1), BIG, mount, root_of(MOUNTED), Version::ReadOnly)
        .unwrap();
    assert_eq!(table.chain(inner).unwrap().ids(), &[file(ROOT_CELL, 5)]);
    let inner_child = table.child(mount, inner, Vnode(3), Uniquifier(1)).unwrap();
    assert_eq!(table.parent(inner_child), Some(mount));
    assert_eq!(
        table.kind(inner_child),
        Some(NodeKind::Child {
            file: file(MOUNTED, 3),
            anchor: mount
        })
    );
}

#[test]
fn cross_refuses_a_repeated_mount_point() {
    // A mount point in volume 200 that names volume 200 again: the second
    // crossing of the same mount point is refused (5.6.1, point 6).
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let outer = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let first = table
        .cross(context(1), BIG, outer, root_of(MOUNTED), Version::ReadOnly)
        .unwrap();
    let again = table.child(outer, first, Vnode(7), Uniquifier(1)).unwrap();
    let second = table
        .cross(context(1), BIG, again, root_of(MOUNTED), Version::ReadOnly)
        .unwrap();
    let again_inside = table.child(again, second, Vnode(7), Uniquifier(1)).unwrap();
    assert_eq!(
        table.cross(
            context(1),
            BIG,
            again_inside,
            root_of(MOUNTED),
            Version::ReadOnly
        ),
        Err(NodeError::MountCycle)
    );
}

#[test]
fn cross_refuses_volume_zero_and_non_anchors() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    assert_eq!(
        table.cross(
            context(1),
            BIG,
            entry,
            root_of(VolumeId(0)),
            Version::ReadOnly
        ),
        Err(NodeError::VersionAbsent)
    );
    assert_eq!(
        table.cross(context(1), BIG, root, root_of(MOUNTED), Version::ReadOnly),
        Err(NodeError::NotAnAnchor)
    );
    assert_eq!(
        table.cross(
            context(1),
            BIG,
            DYNAMIC_ROOT,
            root_of(MOUNTED),
            Version::ReadOnly
        ),
        Err(NodeError::NotAnAnchor)
    );
    let other_cell = FileId {
        cell: CellIndex(2),
        ..root_of(MOUNTED)
    };
    assert_eq!(
        table.cross(context(1), BIG, entry, other_cell, Version::ReadOnly),
        Err(NodeError::CellMismatch)
    );
}

#[test]
fn two_contexts_resolving_one_mount_point_to_two_volumes_get_two_roots() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let first = table
        .cross(context(1), BIG, mount, root_of(MOUNTED), Version::ReadOnly)
        .unwrap();
    let second = table
        .cross(
            context(2),
            BIG,
            mount,
            root_of(VolumeId(201)),
            Version::ReadWrite,
        )
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        table.binding(context(1), mount),
        Some((first, Version::ReadOnly))
    );
    assert_eq!(
        table.binding(context(2), mount),
        Some((second, Version::ReadWrite))
    );
    let in_first = table.child(mount, first, Vnode(3), Uniquifier(1)).unwrap();
    let in_second = table.child(mount, second, Vnode(3), Uniquifier(1)).unwrap();
    assert_ne!(in_first, in_second);
}

#[test]
fn one_volume_mounted_twice_has_an_alias_under_each_anchor() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let first_mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let second_mount = table.child(entry, root, Vnode(6), Uniquifier(1)).unwrap();
    let first = table
        .cross(
            context(1),
            BIG,
            first_mount,
            root_of(MOUNTED),
            Version::ReadOnly,
        )
        .unwrap();
    let second = table
        .cross(
            context(1),
            BIG,
            second_mount,
            root_of(MOUNTED),
            Version::ReadOnly,
        )
        .unwrap();
    let a = table
        .child(first_mount, first, Vnode(3), Uniquifier(1))
        .unwrap();
    let b = table
        .child(second_mount, second, Vnode(3), Uniquifier(1))
        .unwrap();
    assert_ne!(a, b);
    assert_eq!(table.aliases(&file(MOUNTED, 3)), vec![a, b]);
    assert_eq!(table.aliases(&root_of(MOUNTED)), vec![first, second]);
    assert_eq!(table.volume_nodes(CELL, MOUNTED), vec![first, second, a, b]);
}

#[test]
fn pin_is_charged_once_per_node_held() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    // The binding holds the root, the entry node and the dynamic root.
    assert_eq!(table.charge(context(1)), 3);
    let child = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    table.pin(context(1), BIG, child).unwrap();
    assert_eq!(table.charge(context(1)), 4);
    table.pin(context(1), BIG, child).unwrap();
    assert_eq!(table.charge(context(1)), 4);
    // Another context pinning the same node is charged for its own hold:
    // the node, its parent, its root and the dynamic root.
    table.pin(context(2), BIG, child).unwrap();
    assert_eq!(table.charge(context(2)), 4);
}

#[test]
fn pin_over_the_budget_is_busy_and_changes_nothing() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let child = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    assert_eq!(table.pin(context(1), 3, child), Err(NodeError::Busy));
    assert_eq!(table.charge(context(1)), 3);
    assert_eq!(table.unpin(context(1), child), Err(NodeError::NotPinned));
    table.pin(context(1), 4, child).unwrap();
    assert_eq!(table.charge(context(1)), 4);
}

#[test]
fn unpin_releases_the_charge() {
    let mut table = table(16);
    let (entry, root) = entered(&mut table);
    let child = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    table.pin(context(2), BIG, child).unwrap();
    table.pin(context(2), BIG, child).unwrap();
    table.unpin(context(2), child).unwrap();
    assert_eq!(table.charge(context(2)), 4);
    table.unpin(context(2), child).unwrap();
    assert_eq!(table.charge(context(2)), 0);
    assert_eq!(table.unpin(context(2), child), Err(NodeError::NotPinned));
}

#[test]
fn pin_refuses_an_unknown_node() {
    let mut table = table(4);
    assert_eq!(
        table.pin(context(1), BIG, NodeId(77)),
        Err(NodeError::UnknownNode)
    );
}

#[test]
fn pin_refuses_a_context_over_the_context_limit() {
    let mut table = NodeTable::new(Limits {
        nodes: 4,
        contexts: 1,
    })
    .unwrap();
    table.pin(context(1), BIG, DYNAMIC_ROOT).unwrap();
    assert_eq!(
        table.pin(context(2), BIG, DYNAMIC_ROOT),
        Err(NodeError::Busy)
    );
    table.unpin(context(1), DYNAMIC_ROOT).unwrap();
    table.pin(context(2), BIG, DYNAMIC_ROOT).unwrap();
}

#[test]
fn a_full_table_evicts_an_unheld_node_and_never_reuses_its_id() {
    let mut table = table(3);
    let first = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let second = table.entry(CELL, PathKind::ReadWrite).unwrap();
    // Full: the oldest unheld node goes.
    let third = table.entry(CellIndex(2), PathKind::ReadOnly).unwrap();
    assert_eq!(table.len(), 3);
    assert_eq!(table.kind(first), None);
    assert!(third > second && second > first);
    let again = table.entry(CELL, PathKind::ReadOnly).unwrap();
    assert!(again > third);
}

#[test]
fn a_full_table_of_held_nodes_refuses_a_new_node_as_busy() {
    let mut table = table(3);
    let entry = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let other = table.entry(CELL, PathKind::ReadWrite).unwrap();
    table.pin(context(1), BIG, entry).unwrap();
    table.pin(context(1), BIG, other).unwrap();
    assert_eq!(
        table.entry(CellIndex(2), PathKind::ReadOnly),
        Err(NodeError::Busy)
    );
    assert_eq!(table.len(), 3);
}

#[test]
fn a_node_named_by_another_is_not_evicted() {
    let mut table = table(5);
    let (entry, root) = entered(&mut table);
    let child = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    table.release_context(context(1));
    table.pin(context(2), BIG, child).unwrap();
    // The binding is gone, but the pinned child names the root and entry.
    let other = table.entry(CellIndex(2), PathKind::ReadOnly).unwrap();
    let last = table.entry(CellIndex(3), PathKind::ReadOnly).unwrap();
    assert!(last > other);
    assert_eq!(table.kind(other), None);
    assert!(table.kind(root).is_some());
    assert!(table.kind(entry).is_some());
    assert_eq!(table.parent(child), Some(entry));
    assert_eq!(table.len(), 5);
}

#[test]
fn eviction_spares_the_nodes_the_new_node_names() {
    let mut table = table(4);
    let (entry, root) = entered(&mut table);
    let other = table.entry(CellIndex(2), PathKind::ReadOnly).unwrap();
    table.release_context(context(1));
    // Full, nothing held: the root is the oldest evictable node, but the
    // new child names it, so the other entry goes.
    let child = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    assert_eq!(table.kind(other), None);
    assert_eq!(table.parent(child), Some(entry));
    assert_eq!(table.containing_root(child), Some(root));
    assert_eq!(table.len(), 4);
}

#[test]
fn ids_are_refused_once_exhausted() {
    let mut table = table(8);
    table.next_id = Some(u64::MAX);
    let last = table.entry(CELL, PathKind::ReadOnly).unwrap();
    assert_eq!(last, NodeId(u64::MAX));
    assert_eq!(
        table.entry(CELL, PathKind::ReadWrite),
        Err(NodeError::IdsExhausted)
    );
}

#[test]
fn pin_count_at_its_maximum_is_busy() {
    let mut table = table(4);
    table.pin(context(1), BIG, DYNAMIC_ROOT).unwrap();
    *table
        .contexts
        .get_mut(&context(1))
        .unwrap()
        .pins
        .get_mut(&DYNAMIC_ROOT)
        .unwrap() = u32::MAX;
    assert_eq!(
        table.pin(context(1), BIG, DYNAMIC_ROOT),
        Err(NodeError::Busy)
    );
}

#[test]
fn rebinding_moves_the_charge_to_the_new_root() {
    let mut table = table(16);
    let (entry, first) = entered(&mut table);
    let second = table
        .cross(
            context(1),
            BIG,
            entry,
            root_of(VolumeId(101)),
            Version::ReadOnly,
        )
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(table.charge(context(1)), 3);
    assert!(table.unbind(context(1), entry));
    assert_eq!(table.charge(context(1)), 0);
    assert!(!table.unbind(context(1), entry));
}

#[test]
fn cross_refused_over_the_budget_leaves_the_table_unchanged() {
    // A full table with one evictable node: the refused crossing makes no
    // root, evicts nothing and spends no id.
    let mut table = table(5);
    let (entry, root) = entered(&mut table);
    let mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let spare = table.entry(CellIndex(2), PathKind::ReadOnly).unwrap();
    assert_eq!(table.len(), 5);
    let before = table.next_id;
    assert_eq!(
        table.cross(context(2), 1, mount, root_of(MOUNTED), Version::ReadOnly),
        Err(NodeError::Busy)
    );
    assert_eq!(table.len(), 5);
    assert_eq!(table.next_id, before);
    assert!(table.kind(spare).is_some());
    assert_eq!(
        table.aliases(&root_of(MOUNTED)),
        [] as [ironafs_vfs::NodeId; 0]
    );
    assert_eq!(table.charge(context(2)), 0);
}

#[test]
fn cross_refused_over_the_context_limit_leaves_the_table_unchanged() {
    let mut table = NodeTable::new(Limits {
        nodes: 5,
        contexts: 1,
    })
    .unwrap();
    let (entry, root) = entered(&mut table);
    let mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let spare = table.entry(CellIndex(2), PathKind::ReadOnly).unwrap();
    let before = table.next_id;
    assert_eq!(
        table.cross(context(2), BIG, mount, root_of(MOUNTED), Version::ReadOnly),
        Err(NodeError::Busy)
    );
    assert_eq!(table.len(), 5);
    assert_eq!(table.next_id, before);
    assert!(table.kind(spare).is_some());
    assert_eq!(
        table.aliases(&root_of(MOUNTED)),
        [] as [ironafs_vfs::NodeId; 0]
    );
}

#[test]
fn rebinding_at_the_budget_is_charged_the_net() {
    // Context 1 holds exactly its budget of 3 (root, entry, dynamic root);
    // moving the binding to another volume's root still holds 3.
    let mut table = table(16);
    let (entry, first) = entered(&mut table);
    let second = table
        .cross(
            context(1),
            3,
            entry,
            root_of(VolumeId(101)),
            Version::ReadOnly,
        )
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(table.charge(context(1)), 3);
    assert_eq!(
        table.binding(context(1), entry),
        Some((second, Version::ReadOnly))
    );
}
