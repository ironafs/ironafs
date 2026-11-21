// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contexts and file ids the integration tests share.

use ironafs_cache::{CellIndex, ContextKey, FileId, Uniquifier, Vnode, VolumeId};
use ironafs_vfs::{ContextId, CredentialGeneration};

/// The cell every fixture file is in.
pub(crate) const CELL: CellIndex = CellIndex(1);

/// Context `id` at credential generation 1.
pub(crate) fn context(id: u64) -> ContextKey {
    ContextKey {
        id: ContextId(id),
        generation: CredentialGeneration(1),
    }
}

/// The file `vnode` of volume `volume` in [`CELL`], uniquifier 1.
pub(crate) fn file(volume: u32, vnode: u32) -> FileId {
    FileId {
        cell: CELL,
        volume: VolumeId(volume),
        vnode: Vnode(vnode),
        uniquifier: Uniquifier(1),
    }
}
