// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Disk` error's text is a fixed vocabulary, never a host's message
//! (seams and scheduling specification, 5.6, point 11 (c)).

use alloc::string::ToString;

use super::{DiskError, DiskOp};

#[test]
fn disk_error_messages_are_fixed() {
    assert_eq!(DiskError::NotFound.to_string(), "not found");
    assert_eq!(DiskError::NoSpace.to_string(), "no space left");
    assert_eq!(DiskError::IsLink.to_string(), "a path component is a link");
    assert_eq!(
        DiskError::Io {
            op: DiskOp::SyncDir
        }
        .to_string(),
        "sync-dir failed"
    );
}

#[test]
fn disk_op_names_are_distinct() {
    let ops = [
        DiskOp::Read,
        DiskOp::Create,
        DiskOp::Write,
        DiskOp::Sync,
        DiskOp::CreateDir,
        DiskOp::Rename,
        DiskOp::SyncDir,
        DiskOp::Remove,
        DiskOp::RemoveDir,
        DiskOp::List,
        DiskOp::Space,
    ];
    let mut names: alloc::vec::Vec<&str> = ops.iter().map(|op| op.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), ops.len());
}
