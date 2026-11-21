// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The node table (ADR 0004, C11).

mod chain;

pub use chain::{MAX_MOUNT_DEPTH, MountChain};

/// Why the node table refused a change. A refusal changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeError {
    /// Crossing would take a root past [`MAX_MOUNT_DEPTH`] mount points.
    MountDepthExceeded,
    /// Crossing would repeat a mount point already in the chain.
    MountCycle,
}
