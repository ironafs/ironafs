// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replies to the kernel, built from typed values (fuse-codec.md 5.1,
//! points 3 and 5, and 5.5).
//!
//! The encoders write ADR 0004's cache and privilege rules themselves, so
//! no caller can pass a value that breaks them: every entry and attribute
//! timeout is zero (L5, point 1); OPEN replies ask for direct I/O and
//! nothing else, and OPENDIR replies for no caching (L5, points 2 and 3);
//! a node is a directory, a regular file or a symbolic link, never a
//! device, and its set-uid and set-gid bits are cleared (L3, point 2).

use crate::init::{MAJOR, Negotiated};
use crate::wire::{put_u16, put_u32, put_u64};
use crate::{Errno, MAX_DATA};

#[cfg(test)]
mod tests;

/// The size of the reply header.
const HEADER_SIZE: usize = 16;
/// The size of the INIT reply's body at every minor from 7.23.
const INIT_OUT_SIZE: usize = 64;
/// `FOPEN_DIRECT_IO`.
const FOPEN_DIRECT_IO: u32 = 1 << 0;
/// The permission bits a mode may show: the owner, group and other bits
/// and the sticky bit, never set-uid (`0o4000`) or set-gid (`0o2000`).
const SHOWN_PERMISSIONS: u16 = 0o1777;
/// The longest symbolic-link target, in bytes (vfs-contract 5.2).
const MAX_LINK_TARGET: usize = 4095;
/// Linux's `NAME_MAX`, reported as `namelen`; [`ironafs_vfs::MAX_NAME_BYTES`].
const NAME_MAX: u32 = 255;

const _: () = assert!(NAME_MAX as usize == ironafs_vfs::MAX_NAME_BYTES);

/// The type of a node. AFS has no other kinds, and L3, point 2, forbids
/// showing a device node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A directory.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link.
    Symlink,
}

impl Kind {
    /// The kind's file type bits in `mode`.
    const fn type_bits(self) -> u32 {
        match self {
            Self::Directory => 0o040_000,
            Self::File => 0o100_000,
            Self::Symlink => 0o120_000,
        }
    }
}

/// A time as the kernel's attributes carry it: seconds since the epoch,
/// at most `i64::MAX`, and nanoseconds below one second (5.5, point 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timestamp {
    seconds: u64,
    nanos: u32,
}

impl Timestamp {
    /// The time, or `None` if `seconds` is over `i64::MAX` or `nanos` is a
    /// second or more.
    #[must_use]
    pub const fn new(seconds: u64, nanos: u32) -> Option<Self> {
        if seconds > i64::MAX as u64 || nanos >= 1_000_000_000 {
            return None;
        }
        Some(Self { seconds, nanos })
    }
}

/// A node's attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attr {
    /// The inode number shown.
    pub ino: u64,
    /// The size in bytes.
    pub size: u64,
    /// The size in 512-byte blocks.
    pub blocks: u64,
    /// The last access.
    pub atime: Timestamp,
    /// The last change of data.
    pub mtime: Timestamp,
    /// The last change of status.
    pub ctime: Timestamp,
    /// The node's type.
    pub kind: Kind,
    /// The permission bits. Only the low nine and the sticky bit are
    /// shown.
    pub permissions: u16,
    /// The link count.
    pub nlink: u32,
    /// The owner's user id.
    pub uid: u32,
    /// The owner's group id.
    pub gid: u32,
    /// The preferred I/O size.
    pub blksize: u32,
}

/// A volume's usage, for STATFS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatFs {
    /// Total blocks.
    pub blocks: u64,
    /// Free blocks.
    pub bfree: u64,
    /// Blocks free to an unprivileged user.
    pub bavail: u64,
    /// Total file nodes.
    pub files: u64,
    /// Free file nodes.
    pub ffree: u64,
    /// The block size.
    pub bsize: u32,
    /// The fragment size.
    pub frsize: u32,
}

/// Why an encoder refused a value (fuse-codec.md 7, point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// Data over [`MAX_DATA`].
    DataTooLong,
    /// A link target empty, over 4,095 bytes, or holding a NUL.
    LinkTarget,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::DataTooLong => "reply data over the 1 MiB cap",
            Self::LinkTarget => "link target empty, over 4,095 bytes or holding a nul",
        })
    }
}

impl core::error::Error for EncodeError {}

/// A reply with this error and the payload `write` appends. `len` is
/// filled in after the payload. Every caller's payload is a fixed size or
/// at most [`MAX_DATA`], so the length fits a `u32`; were it not to, the
/// reply would carry `len` zero, which the kernel refuses rather than
/// misreads.
fn reply(
    unique: u64,
    error: i32,
    payload_size: usize,
    write: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE.saturating_add(payload_size));
    put_u32(&mut out, 0);
    put_u32(&mut out, error.cast_unsigned());
    put_u64(&mut out, unique);
    write(&mut out);
    let len = u32::try_from(out.len()).unwrap_or(0);
    if let Some(field) = out.first_chunk_mut::<4>() {
        *field = len.to_le_bytes();
    }
    out
}

/// An error reply: the header alone, with the negative error number.
#[must_use]
pub fn error(unique: u64, errno: Errno) -> Vec<u8> {
    // An `Errno` is positive and small, so its negation cannot overflow;
    // the fallback is never taken.
    let negative = errno.get().checked_neg().unwrap_or(-Errno::EIO.get());
    reply(unique, negative, 0, |_| {})
}

/// A success reply with no payload (FLUSH, RELEASE, RELEASEDIR, ACCESS,
/// DESTROY).
#[must_use]
pub fn empty(unique: u64) -> Vec<u8> {
    reply(unique, 0, 0, |_| {})
}

fn put_time(out: &mut Vec<u8>, time: Timestamp) {
    put_u64(out, time.seconds);
}

/// Appends `fuse_attr` (88 bytes).
fn put_attr(out: &mut Vec<u8>, attr: &Attr) {
    put_u64(out, attr.ino);
    put_u64(out, attr.size);
    put_u64(out, attr.blocks);
    put_time(out, attr.atime);
    put_time(out, attr.mtime);
    put_time(out, attr.ctime);
    put_u32(out, attr.atime.nanos);
    put_u32(out, attr.mtime.nanos);
    put_u32(out, attr.ctime.nanos);
    put_u32(
        out,
        attr.kind.type_bits() | u32::from(attr.permissions & SHOWN_PERMISSIONS),
    );
    put_u32(out, attr.nlink);
    put_u32(out, attr.uid);
    put_u32(out, attr.gid);
    put_u32(out, 0); // rdev: never a device
    put_u32(out, attr.blksize);
    put_u32(out, 0); // flags
}

/// LOOKUP's reply: the node, its generation and attributes, with both
/// timeouts zero.
#[must_use]
pub fn entry(unique: u64, node: u64, generation: u64, attr: &Attr) -> Vec<u8> {
    reply(unique, 0, 128, |out| {
        put_u64(out, node);
        put_u64(out, generation);
        put_u64(out, 0); // entry_valid
        put_u64(out, 0); // attr_valid
        put_u32(out, 0); // entry_valid_nsec
        put_u32(out, 0); // attr_valid_nsec
        put_attr(out, attr);
    })
}

/// GETATTR's reply, with the timeout zero.
#[must_use]
pub fn attr(unique: u64, attr: &Attr) -> Vec<u8> {
    reply(unique, 0, 104, |out| {
        put_u64(out, 0); // attr_valid
        put_u32(out, 0); // attr_valid_nsec
        put_u32(out, 0); // dummy
        put_attr(out, attr);
    })
}

fn open_reply(unique: u64, fh: u64, open_flags: u32) -> Vec<u8> {
    reply(unique, 0, 16, |out| {
        put_u64(out, fh);
        put_u32(out, open_flags);
        put_u32(out, 0); // padding, or backing_id from 7.40
    })
}

/// OPEN's reply: the handle, with direct I/O and no other open flag.
#[must_use]
pub fn open(unique: u64, fh: u64) -> Vec<u8> {
    open_reply(unique, fh, FOPEN_DIRECT_IO)
}

/// OPENDIR's reply: the handle, with no open flag, so the directory is not
/// cached.
#[must_use]
pub fn opendir(unique: u64, fh: u64) -> Vec<u8> {
    open_reply(unique, fh, 0)
}

/// READ's reply: the data.
///
/// # Errors
///
/// [`EncodeError::DataTooLong`] over [`MAX_DATA`].
pub fn data(unique: u64, bytes: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if u32::try_from(bytes.len()).map_or(true, |len| len > MAX_DATA) {
        return Err(EncodeError::DataTooLong);
    }
    Ok(reply(unique, 0, bytes.len(), |out| {
        out.extend_from_slice(bytes);
    }))
}

/// READLINK's reply: the target's bytes, with no NUL.
///
/// # Errors
///
/// [`EncodeError::LinkTarget`] for an empty target, one over 4,095 bytes,
/// or one holding a NUL.
pub fn readlink(unique: u64, target: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if target.is_empty() || target.len() > MAX_LINK_TARGET || target.contains(&0) {
        return Err(EncodeError::LinkTarget);
    }
    Ok(reply(unique, 0, target.len(), |out| {
        out.extend_from_slice(target);
    }))
}

/// STATFS's reply, with `namelen` 255.
#[must_use]
pub fn statfs(unique: u64, value: &StatFs) -> Vec<u8> {
    reply(unique, 0, 80, |out| {
        put_u64(out, value.blocks);
        put_u64(out, value.bfree);
        put_u64(out, value.bavail);
        put_u64(out, value.files);
        put_u64(out, value.ffree);
        put_u32(out, value.bsize);
        put_u32(out, NAME_MAX);
        put_u32(out, value.frsize);
        put_u32(out, 0); // padding
        out.extend_from_slice(&[0; 24]); // spare
    })
}

/// INIT's reply for a settled protocol (5.5, point 5): its flags are
/// those [`crate::init::select_flags`] chose, low half in `flags` and high
/// half in `flags2`.
#[must_use]
pub fn init(unique: u64, negotiated: &Negotiated) -> Vec<u8> {
    reply(unique, 0, INIT_OUT_SIZE, |out| {
        let start = out.len();
        put_u32(out, MAJOR);
        put_u32(out, negotiated.minor());
        put_u32(out, negotiated.max_readahead);
        put_u32(out, negotiated.flags().low());
        put_u16(out, negotiated.max_background);
        put_u16(out, negotiated.congestion_threshold);
        put_u32(out, negotiated.max_write);
        put_u32(out, negotiated.time_gran);
        put_u16(out, 0); // max_pages: FUSE_MAX_PAGES is not selected
        put_u16(out, 0); // map_alignment
        put_u32(out, negotiated.flags().high());
        // max_stack_depth, request_timeout and the unused words.
        out.resize(start.saturating_add(INIT_OUT_SIZE), 0);
    })
}

/// INIT's reply to a kernel with a newer major: only major 7, after which
/// the kernel sends a new INIT (fuse-codec.md 5.3, point 1).
#[must_use]
pub fn init_major_only(unique: u64) -> Vec<u8> {
    reply(unique, 0, 4, |out| put_u32(out, MAJOR))
}
