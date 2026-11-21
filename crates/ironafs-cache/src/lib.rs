// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's cache state (ADR 0004, C4, point 1) and the `Disk` interface
//! over the state directory (C3, point 3; C4, point 7).
//!
//! It holds:
//! - the node table ([`NodeTable`]; C11): node ids, their keys and
//!   aliases, anchor chains, pins and each context's mount-point bindings;
//! - the fence ([`FenceTable`], [`FenceTuple`]; C16, point 2): each
//!   context's object and volume break counters and local volume epochs,
//!   the resume generation, the client-wide suspect epoch source, and the
//!   one check that compares a recorded tuple with the current one at
//!   install and at serve;
//! - the `Disk` interface: [`Disk`], its error and the paths and plans it
//!   takes (seams and scheduling specification, 5.5).
//!
//! It is a pure crate: it holds formats, decisions and plans, and never
//! calls `Disk` itself (`cargo xtask check-seams`).

#![no_std]

extern crate alloc;

mod disk;
mod fence;
mod identity;
mod node;
mod store_path;

pub use disk::{Disk, DiskError, DiskOp, Durability, EntryKind, Plan, PlanStep, ReadRange, Space};
pub use fence::{
    ClockWatch, EpochSource, FenceError, FenceLimits, FenceTable, FenceTuple, MAX_FENCE_BYTES,
    MIN_FENCE_BYTES, OBJECT_RECORD_BYTES, PauseGeneration, ServerValues, Stamp, Suspicion,
    VOLUME_RECORD_BYTES, VolumeEpoch,
};
pub use identity::{CellIndex, ContextKey, FileId, PathKind, Uniquifier, Version, Vnode, VolumeId};
pub use node::{
    DYNAMIC_ROOT, Limits, MAX_CONTEXTS, MAX_MOUNT_DEPTH, MAX_NODES, MountChain, NodeError,
    NodeKind, NodeTable,
};
pub use store_path::{MAX_COMPONENTS, MAX_NAME_BYTES, StoreDir, StoreName, StorePath};
