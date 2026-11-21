// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What names a file and a credential context in the cache (ADR 0004,
//! C11, point 1; N38, point 2).
//!
//! Volume, vnode and uniquifier are distinct unsigned 32-bit types under
//! the `afs3` protocol identifier, never bare integers (N38, point 2). A
//! cell is named by its index in the cell table, which is never reused for
//! another cell within one daemon instance (namespace specification,
//! 5.6.1, point 4).

use ironafs_vfs::{ContextId, CredentialGeneration};

/// A cell, by its index in the cell table. The index is never reused for
/// another cell within one daemon instance, so a reload of the
/// configuration cannot make two cells compare equal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellIndex(pub u32);

/// A volume id (N38, point 2). The id 0 names no volume: the node table
/// refuses it in a key (namespace specification, 5.6.2, point 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VolumeId(pub u32);

/// A vnode number within a volume (N38, point 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vnode(pub u32);

/// A vnode's uniquifier (N38, point 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uniquifier(pub u32);

/// A file's identity in a cell: cell, volume, vnode and uniquifier. It is
/// also a mount point's identity (namespace specification, section 4).
///
/// The fields are ordered so that every file id of one volume sorts
/// together, and a volume is one range of the reverse index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId {
    /// The cell.
    pub cell: CellIndex,
    /// The volume.
    pub volume: VolumeId,
    /// The vnode.
    pub vnode: Vnode,
    /// The uniquifier.
    pub uniquifier: Uniquifier,
}

/// The path a dynamic-root name enters a cell's root volume on (namespace
/// specification, 5.6.3, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathKind {
    /// The cell's name: the read-only path.
    ReadOnly,
    /// The cell's name with a leading `.`: the read/write path.
    ReadWrite,
}

/// A volume's version (namespace specification, section 4), held only in a
/// context's binding, never on a node (5.6.2, point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Version {
    /// Read/write.
    ReadWrite,
    /// Read-only.
    ReadOnly,
    /// Backup.
    Backup,
}

/// A credential context at one generation (C18, point 1; C40): what pins
/// and bindings are charged to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextKey {
    /// The context.
    pub id: ContextId,
    /// Its credential generation.
    pub generation: CredentialGeneration,
}
