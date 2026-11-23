// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Disk` interface (ADR 0004, C3, point 3; seams and scheduling
//! specification, 5.5) and the plans that pure code writes for it.
//!
//! This crate defines the interface and never calls it: operation code in
//! the core runs a [`Plan`] through `Disk` step by step and stops at the
//! first error. The production driver and the simulated disk implement it.

use alloc::vec::Vec;
use core::error::Error;
use core::fmt;

use crate::{StoreDir, StoreName, StorePath};

#[cfg(test)]
mod tests;

/// What [`Disk::read`] returns of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReadRange {
    /// The whole file, or [`DiskError::TooLarge`] and nothing if it is
    /// longer than `max` bytes.
    Whole {
        /// The most bytes the caller takes.
        max: u64,
    },
    /// The bytes from `offset` to the smaller of `offset + len` and the
    /// file's end: fewer than `len` only past the end, and none if `offset`
    /// is at or past it. An `offset + len` overflow is
    /// [`DiskError::TooLarge`].
    Part {
        /// The first byte.
        offset: u64,
        /// The most bytes returned.
        len: u64,
    },
}

/// Whether [`Disk::create`] syncs what it wrote, on the handle that wrote
/// it, before closing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Durability {
    /// No sync.
    Unsynced,
    /// Synced on the same handle before closing.
    Synced,
}

/// The kind of a directory entry, read without following a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A link, or any entry that is neither a file nor a directory: never
    /// followed and never removed.
    Other,
}

/// The space of the volume that holds the state directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Space {
    /// Bytes the daemon's account can still write.
    pub available: u64,
    /// The volume's size in bytes.
    pub total: u64,
}

/// The state directory as the core sees it (5.5, point 1). Every call may
/// block, so under the simulator every call is a yield point; no call takes
/// a deadline or a cancel token (5.5, point 5), and no core lock is held
/// across one.
///
/// After a failure the effect can be uncertain (5.5, point 4): an
/// operation that took effect can be reported as failed, and a failed sync
/// means the data is not durable, never to be trusted as durable later.
pub trait Disk: Send + Sync {
    /// Reads `range` of the file at `path`.
    ///
    /// # Errors
    ///
    /// `NotFound`, `TooLarge`, `IsLink`, `IsDir`, or `Io { op: Read }`.
    fn read(&self, path: &StorePath, range: ReadRange) -> Result<Vec<u8>, DiskError>;

    /// Creates exclusively, readable only by the daemon's account, and
    /// writes all of `bytes`. `Synced` syncs on the same handle before
    /// closing. Never replaces a file.
    ///
    /// # Errors
    ///
    /// `Exists`, `NoSpace`, `IsLink`, or `Io` with `Create`, `Write` or
    /// `Sync`. Where it failed decides what is left, and the error does not
    /// say where.
    fn create(&self, path: &StorePath, bytes: &[u8], d: Durability) -> Result<(), DiskError>;

    /// Creates a directory exclusively. Durable only after its parent's
    /// `sync_dir`.
    ///
    /// # Errors
    ///
    /// `Exists`, `NoSpace`, `IsLink`, or `Io { op: CreateDir }`.
    fn create_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;

    /// Replaces `to` with `from`, both in `dir`. Durable only after
    /// `sync_dir(dir)`.
    ///
    /// # Errors
    ///
    /// `NotFound` (the source), `IsLink`, `IsDir`, or `Io { op: Rename }`,
    /// after which whether it was applied is uncertain.
    fn rename(&self, dir: &StoreDir, from: &StoreName, to: &StoreName) -> Result<(), DiskError>;

    /// Syncs the directory through the handle held for it since it was
    /// opened.
    ///
    /// # Errors
    ///
    /// `NotFound`, `IsLink`, or `Io { op: SyncDir }`, after which every
    /// namespace change in the directory no earlier successful sync covered
    /// is of uncertain durability.
    fn sync_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;

    /// Removes a file. Durable only after `sync_dir`.
    ///
    /// # Errors
    ///
    /// `NotFound`, `IsLink`, `IsDir`, or `Io { op: Remove }`.
    fn remove(&self, path: &StorePath) -> Result<(), DiskError>;

    /// Removes an empty directory. Durable only after its parent's
    /// `sync_dir`.
    ///
    /// # Errors
    ///
    /// `NotFound`, `IsLink`, `NotEmpty`, or `Io { op: RemoveDir }`.
    fn remove_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;

    /// At most `max` entries after `after`, in byte order of their names,
    /// each with its kind, read without following a link.
    ///
    /// # Errors
    ///
    /// `NotFound`, `IsLink`, or `Io { op: List }`.
    fn list(
        &self,
        dir: &StoreDir,
        after: Option<&StoreName>,
        max: u32,
    ) -> Result<Vec<(StoreName, EntryKind)>, DiskError>;

    /// The space of the volume that holds the state directory.
    ///
    /// # Errors
    ///
    /// `Io { op: Space }`.
    fn space(&self) -> Result<Space, DiskError>;
}

/// Why a [`Disk`] call failed: the seam's only error (5.5, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiskError {
    /// A named entry does not exist.
    NotFound,
    /// The entry to create exists.
    Exists,
    /// No space, from `create` and `create_dir` only.
    NoSpace,
    /// A whole read over its maximum, or a range that overflows.
    TooLarge,
    /// A component of the path is a link or other redirection.
    IsLink,
    /// A named entry is a directory where a file was needed.
    IsDir,
    /// The directory to remove is not empty.
    NotEmpty,
    /// Any other failure, of the step named.
    Io {
        /// The step that failed.
        op: DiskOp,
    },
}

/// The step of a [`Disk`] call that failed with [`DiskError::Io`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiskOp {
    /// Reading a file.
    Read,
    /// Opening a file exclusively.
    Create,
    /// Writing a file's bytes.
    Write,
    /// Syncing a file on the handle that wrote it.
    Sync,
    /// Creating a directory.
    CreateDir,
    /// Renaming within a directory.
    Rename,
    /// Syncing a directory.
    SyncDir,
    /// Removing a file.
    Remove,
    /// Removing a directory.
    RemoveDir,
    /// Listing a directory.
    List,
    /// Reading the volume's space.
    Space,
}

impl DiskOp {
    /// The step's stable name, as traces record it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Create => "create",
            Self::Write => "write",
            Self::Sync => "sync",
            Self::CreateDir => "create-dir",
            Self::Rename => "rename",
            Self::SyncDir => "sync-dir",
            Self::Remove => "remove",
            Self::RemoveDir => "remove-dir",
            Self::List => "list",
            Self::Space => "space",
        }
    }
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("not found"),
            Self::Exists => formatter.write_str("already exists"),
            Self::NoSpace => formatter.write_str("no space left"),
            Self::TooLarge => formatter.write_str("too large"),
            Self::IsLink => formatter.write_str("a path component is a link"),
            Self::IsDir => formatter.write_str("is a directory"),
            Self::NotEmpty => formatter.write_str("directory not empty"),
            Self::Io { op } => write!(formatter, "{} failed", op.name()),
        }
    }
}

impl Error for DiskError {}

/// One step of a [`Plan`]: a `Disk` call that changes the state directory.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlanStep {
    /// [`Disk::create`].
    Create {
        /// The file to create.
        path: StorePath,
        /// What to write.
        bytes: Vec<u8>,
        /// Whether to sync it.
        durability: Durability,
    },
    /// [`Disk::create_dir`].
    CreateDir(StoreDir),
    /// [`Disk::rename`].
    Rename {
        /// The directory of both names.
        dir: StoreDir,
        /// The name renamed.
        from: StoreName,
        /// The name replaced.
        to: StoreName,
    },
    /// [`Disk::sync_dir`].
    SyncDir(StoreDir),
    /// [`Disk::remove`].
    Remove(StorePath),
    /// [`Disk::remove_dir`].
    RemoveDir(StoreDir),
}

/// A multi-step change as pure code expresses it (5.5, point 6): `Disk`
/// steps in order, which operation code runs one at a time and stops at
/// the first error. A plan is data, so its crash points can be enumerated.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Plan {
    /// The steps, in the order they run.
    pub steps: Vec<PlanStep>,
}
