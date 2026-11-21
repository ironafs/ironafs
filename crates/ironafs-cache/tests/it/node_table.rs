// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance cases of issue #91 (ADR 0004, C11, points 2, 3, 9 and
//! 10), each named by the case it shows.

use ironafs_cache::{
    CellIndex, ContextKey, DYNAMIC_ROOT, FileId, Limits, NodeError, NodeTable, PathKind,
    Uniquifier, Version, Vnode, VolumeId,
};
use ironafs_vfs::{ContextId, CredentialGeneration, NodeId};

const CELL: CellIndex = CellIndex(1);
const BIG: u32 = 1_000;

fn context(id: u64) -> ContextKey {
    ContextKey {
        id: ContextId(id),
        generation: CredentialGeneration(1),
    }
}

fn file(volume: u32, vnode: u32) -> FileId {
    FileId {
        cell: CELL,
        volume: VolumeId(volume),
        vnode: Vnode(vnode),
        uniquifier: Uniquifier(1),
    }
}

/// A cell root (volume 100) entered in context 1, with two mount points,
/// vnodes 5 and 6, in its root directory.
fn two_mount_points(table: &mut NodeTable) -> (NodeId, NodeId, NodeId) {
    let entry = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let root = table
        .cross(context(1), BIG, entry, file(100, 1), Version::ReadOnly)
        .unwrap();
    let first = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    let second = table.child(entry, root, Vnode(6), Uniquifier(1)).unwrap();
    (root, first, second)
}

fn table(nodes: u32) -> NodeTable {
    NodeTable::new(Limits {
        nodes,
        contexts: 16,
    })
    .unwrap()
}

#[test]
fn a_break_reaches_every_alias_of_a_file_id() {
    // Volume 200 mounted at two mount points: one file, two nodes, and a
    // break naming the file id reaches both, in each of two contexts.
    let mut table = table(64);
    let (_, first_mount, second_mount) = two_mount_points(&mut table);
    let mut expected = Vec::new();
    for ctx in [context(1), context(2)] {
        for mount in [first_mount, second_mount] {
            let root = table
                .cross(ctx, BIG, mount, file(200, 1), Version::ReadOnly)
                .unwrap();
            let directory = table.child(mount, root, Vnode(3), Uniquifier(1)).unwrap();
            let leaf = table
                .child(directory, root, Vnode(4), Uniquifier(1))
                .unwrap();
            if !expected.contains(&leaf) {
                expected.push(leaf);
            }
        }
    }
    expected.sort();
    assert_eq!(expected.len(), 2);
    assert_eq!(table.aliases(&file(200, 4)), expected);
    let volume = table.volume_nodes(CELL, VolumeId(200));
    for node in &expected {
        assert!(volume.contains(node));
    }
}

#[test]
fn two_contexts_resolving_one_mount_point_to_different_volumes_get_different_nodes() {
    let mut table = table(64);
    let (_, mount, _) = two_mount_points(&mut table);
    let read_only = table
        .cross(context(1), BIG, mount, file(201, 1), Version::ReadOnly)
        .unwrap();
    let read_write = table
        .cross(context(2), BIG, mount, file(200, 1), Version::ReadWrite)
        .unwrap();
    assert_ne!(read_only, read_write);
    let below_first = table
        .child(mount, read_only, Vnode(3), Uniquifier(1))
        .unwrap();
    let below_second = table
        .child(mount, read_write, Vnode(3), Uniquifier(1))
        .unwrap();
    assert_ne!(below_first, below_second);
    // The layer holds one node for the path: the mount point, whose parent
    // is the same for both, and `..` of either child is that node.
    assert_eq!(table.parent(below_first), Some(mount));
    assert_eq!(table.parent(below_second), Some(mount));
    assert_eq!(
        table.binding(context(1), mount),
        Some((read_only, Version::ReadOnly))
    );
    assert_eq!(
        table.binding(context(2), mount),
        Some((read_write, Version::ReadWrite))
    );
}

#[test]
fn a_pin_over_the_contexts_budget_is_refused_as_busy() {
    let mut table = table(64);
    let (root, mount, _) = two_mount_points(&mut table);
    // Context 1's binding holds the root, the entry and the dynamic root.
    assert_eq!(table.charge(context(1)), 3);
    assert_eq!(table.pin(context(1), 3, mount), Err(NodeError::Busy));
    assert_eq!(table.charge(context(1)), 3);
    table.pin(context(1), 4, mount).unwrap();
    assert_eq!(table.charge(context(1)), 4);
    let _ = root;
}

#[test]
fn a_pin_into_a_full_table_is_refused_as_busy() {
    // A table of four: the dynamic root, the entry, the root and one
    // mount point, all held. A lookup that would hand the layer a new node
    // finds no room and is refused as busy; nothing pinned is evicted.
    let mut table = table(4);
    let entry = table.entry(CELL, PathKind::ReadOnly).unwrap();
    let root = table
        .cross(context(1), BIG, entry, file(100, 1), Version::ReadOnly)
        .unwrap();
    let mount = table.child(entry, root, Vnode(5), Uniquifier(1)).unwrap();
    table.pin(context(1), BIG, mount).unwrap();
    assert_eq!(
        table.child(entry, root, Vnode(6), Uniquifier(1)),
        Err(NodeError::Busy)
    );
    for node in [DYNAMIC_ROOT, entry, root, mount] {
        assert!(table.kind(node).is_some(), "{node:?}");
    }
}

#[test]
fn a_node_id_is_not_reused_after_eviction() {
    let mut table = table(3);
    let mut seen = Vec::new();
    for cell in 0..100 {
        let node = table.entry(CellIndex(cell), PathKind::ReadOnly).unwrap();
        assert!(!seen.contains(&node), "{node:?} reused");
        seen.push(node);
    }
    assert_eq!(table.len(), 3);
}
