// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's cache state (ADR 0004, C4, point 1) and the `Disk` interface
//! over the state directory (C3, point 3; C4, point 7).
//!
//! For now it holds the interface only: [`Disk`], its error and the paths
//! and plans it takes (seams and scheduling specification, 5.5). It is a
//! pure crate: it holds formats, decisions and plans, and never calls
//! `Disk` itself (`cargo xtask check-seams`).

#![no_std]

extern crate alloc;

mod disk;
mod store_path;

pub use disk::{Disk, DiskError, DiskOp, Durability, EntryKind, Plan, PlanStep, ReadRange, Space};
pub use store_path::{MAX_COMPONENTS, MAX_NAME_BYTES, StoreDir, StoreName, StorePath};
