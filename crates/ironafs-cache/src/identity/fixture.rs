// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contexts and file ids the crate's unit tests share.

use ironafs_vfs::{ContextId, CredentialGeneration};

use crate::{CellIndex, ContextKey, FileId, Uniquifier, Vnode, VolumeId};

/// The cell every fixture file is in.
pub(crate) const CELL: CellIndex = CellIndex(1);

/// Context `id` at credential generation 1.
pub(crate) fn context(id: u64) -> ContextKey {
    ContextKey {
        id: ContextId(id),
        generation: CredentialGeneration(1),
    }
}

/// The file `vnode` of `volume` in [`CELL`], uniquifier 1.
pub(crate) fn file(volume: VolumeId, vnode: u32) -> FileId {
    FileId {
        cell: CELL,
        volume,
        vnode: Vnode(vnode),
        uniquifier: Uniquifier(1),
    }
}
