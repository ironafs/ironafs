// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Why a handle's dirty data is committed (ADR 0004, C9).
//!
//! A write reaches the server through one operation, commit, and each
//! layer maps its own events to one of these reasons (C9, point 2). A
//! commit reports success only when the server has accepted the store
//! (C9, point 5); release never fails (C9, point 4).

/// The event a layer maps to a commit (C9, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommitReason {
    /// The application flushed the file (FUSE flush, an SMB flush).
    Flush,
    /// The application asked for the file's data to be durable (fsync).
    Fsync,
    /// The last close of the open.
    Close,
    /// The core's own interval for dirty data.
    Interval,
    /// The open was made write-through, so every write commits.
    WriteThrough,
    /// A whole-volume or whole-system sync.
    Sync,
}
