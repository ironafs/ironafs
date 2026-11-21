// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The depth and cycle rule of the namespace specification, 5.6.1, points
//! 1 and 2.

use super::{MAX_MOUNT_DEPTH, MountChain};
use crate::node::NodeError;
use crate::{CellIndex, FileId, Uniquifier, Vnode, VolumeId};

fn mount_point(vnode: u32) -> FileId {
    FileId {
        cell: CellIndex(1),
        volume: VolumeId(7),
        vnode: Vnode(vnode),
        uniquifier: Uniquifier(1),
    }
}

fn chain_of(depth: u32) -> MountChain {
    let mut chain = MountChain::empty();
    for vnode in 1..=depth {
        chain = chain.extended(mount_point(vnode)).unwrap();
    }
    chain
}

#[test]
fn extended_appends_the_mount_point_crossed() {
    let chain = chain_of(2);
    assert_eq!(chain.depth(), 2);
    assert_eq!(chain.ids(), &[mount_point(1), mount_point(2)]);
}

#[test]
fn extended_allows_the_32nd_crossing_and_refuses_the_33rd() {
    assert_eq!(MAX_MOUNT_DEPTH, 32);
    let full = chain_of(32);
    assert_eq!(full.depth(), 32);
    assert_eq!(
        full.extended(mount_point(33)),
        Err(NodeError::MountDepthExceeded)
    );
}

#[test]
fn extended_refuses_a_mount_point_already_in_the_chain() {
    let chain = chain_of(5);
    for vnode in 1..=5 {
        assert_eq!(
            chain.extended(mount_point(vnode)),
            Err(NodeError::MountCycle),
            "vnode {vnode}"
        );
    }
}

#[test]
fn extended_checks_depth_before_cycle() {
    assert_eq!(
        chain_of(32).extended(mount_point(1)),
        Err(NodeError::MountDepthExceeded)
    );
}

#[test]
fn extended_compares_whole_identities() {
    // The same vnode in another volume or cell is another mount point
    // (5.6.1, point 3).
    let chain = chain_of(1);
    let other_volume = FileId {
        volume: VolumeId(8),
        ..mount_point(1)
    };
    let other_cell = FileId {
        cell: CellIndex(2),
        ..mount_point(1)
    };
    assert_eq!(chain.extended(other_volume).unwrap().depth(), 2);
    assert_eq!(chain.extended(other_cell).unwrap().depth(), 2);
}
