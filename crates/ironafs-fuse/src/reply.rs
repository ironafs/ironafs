// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replies to the kernel, built from typed values (fuse-codec.md 5.1,
//! points 3 and 5, and 5.5).
//!
//! A reply is reachable only through the encoders that keep its rules
//! (5.5, point 11). [`request::decode`](crate::request::decode) hands the
//! layer one [`ReplyTo`] for each request that takes a reply, typed by the
//! reply shapes that request allows ([`shape`]); a request that takes none
//! comes with none. A `ReplyTo` cannot be built from a request id, copied
//! or used twice, and its encoders return a [`Reply`], whose bytes the
//! layer can read and write to the device but not change.
//!
//! The compiler keeps those rules. A decoded OPEN's reply can be encoded
//! once, as an OPEN reply or an error:
//!
//! ```
//! use ironafs_fuse::Errno;
//! use ironafs_fuse::request::{Phase, Request, decode};
//!
//! let mut bytes = Vec::new();
//! for field in [48u32, 14] {
//!     bytes.extend_from_slice(&field.to_le_bytes()); // len, opcode OPEN
//! }
//! bytes.extend_from_slice(&[0; 32]); // unique to padding
//! bytes.extend_from_slice(&[0; 8]); // open(2) flags, open_flags
//! let (_, request) = decode(&bytes, Phase::Negotiated { minor: 46 })?;
//! let Request::Open { reply, .. } = request else { panic!("not open") };
//! assert_eq!(reply.open(9).as_bytes().len(), 32);
//! # let (_, request) = decode(&bytes, Phase::Negotiated { minor: 46 })?;
//! # let Request::Open { reply, .. } = request else { panic!("not open") };
//! # let _ = reply.error(Errno::EACCES);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A reply token cannot be made from a request id:
//!
//! ```compile_fail,E0624
//! use ironafs_fuse::reply::{ReplyTo, shape};
//! let reply = ReplyTo::<shape::Open>::new(7, 0);
//! ```
//!
//! nor a reply from bytes:
//!
//! ```compile_fail,E0423
//! use ironafs_fuse::reply::Reply;
//! let reply = Reply(vec![16, 0, 0, 0]);
//! ```
//!
//! A READ's token has no OPEN encoder:
//!
//! ```compile_fail,E0599
//! # use ironafs_fuse::request::{Phase, Request, decode};
//! # let bytes = [0u8; 80];
//! # let (_, request) = decode(&bytes, Phase::Negotiated { minor: 46 }).unwrap();
//! let Request::Read { reply, .. } = request else { panic!("not read") };
//! let wrong = reply.open(9);
//! ```
//!
//! and a token is used once, and cannot be copied:
//!
//! ```compile_fail,E0382
//! # use ironafs_fuse::Errno;
//! # use ironafs_fuse::request::{Phase, Request, decode};
//! # let bytes = [0u8; 48];
//! # let (_, request) = decode(&bytes, Phase::Negotiated { minor: 46 }).unwrap();
//! let Request::Open { reply, .. } = request else { panic!("not open") };
//! let first = reply.open(9);
//! let second = reply.error(Errno::EIO);
//! ```
//!
//! ```compile_fail,E0599
//! # use ironafs_fuse::request::{Phase, Request, decode};
//! # let bytes = [0u8; 48];
//! # let (_, request) = decode(&bytes, Phase::Negotiated { minor: 46 }).unwrap();
//! let Request::Open { reply, .. } = request else { panic!("not open") };
//! let copy = reply.clone();
//! ```
//!
//! The encoders write ADR 0004's cache and privilege rules themselves, so
//! no caller can pass a value that breaks them: every entry and attribute
//! timeout is zero (L5, point 1); OPEN replies ask for direct I/O and
//! nothing else, and OPENDIR replies for no caching (L5, points 2 and 3);
//! a node is a directory, a regular file or a symbolic link, never a
//! device, and its set-uid and set-gid bits are cleared (L3, point 2).

use core::fmt;
use core::marker::PhantomData;

use crate::Errno;
use crate::init::{MAJOR, Negotiated};
use crate::wire::{put_u16, put_u32, put_u64};

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
/// `STATX_BASIC_STATS`: the fields of `stat`, which a reply always fills.
const STATX_BASIC_STATS: u32 = 0x7ff;
/// `STATX_BTIME`: the birth time is filled.
const STATX_BTIME: u32 = 0x800;
/// The size of STATX's reply body: `fuse_statx_out`.
const STATX_OUT_SIZE: usize = 288;
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
    const fn type_bits(self) -> u16 {
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
    /// Zero seconds: what an absent time's field holds.
    const EPOCH: Self = Self {
        seconds: 0,
        nanos: 0,
    };

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
    /// Data over [`MAX_DATA`](crate::MAX_DATA).
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

/// The encoded bytes of one reply, ready to write to the device.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply(Vec<u8>);

impl Reply {
    /// The reply's bytes, header included.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// The reply shapes, one marker per kind of request (fuse-codec.md 5.5,
/// point 11). Each is uninhabited: it exists only as [`ReplyTo`]'s type
/// parameter.
pub mod shape {
    /// LOOKUP: an entry.
    #[derive(Debug)]
    pub enum Entry {}
    /// GETATTR: attributes.
    #[derive(Debug)]
    pub enum Attr {}
    /// STATX: extended attributes.
    #[derive(Debug)]
    pub enum Statx {}
    /// OPEN: a file handle with direct I/O.
    #[derive(Debug)]
    pub enum Open {}
    /// OPENDIR: a directory handle with no caching.
    #[derive(Debug)]
    pub enum Opendir {}
    /// READ: data, at most the size asked for.
    #[derive(Debug)]
    pub enum Data {}
    /// READDIR: only an error until the directory records are specified.
    #[derive(Debug)]
    pub enum Readdir {}
    /// READLINK: a link target.
    #[derive(Debug)]
    pub enum Readlink {}
    /// STATFS: a volume's usage.
    #[derive(Debug)]
    pub enum Statfs {}
    /// FLUSH, RELEASE, RELEASEDIR, ACCESS and DESTROY: success with no
    /// payload.
    #[derive(Debug)]
    pub enum Empty {}
    /// INIT: the negotiated protocol, or the major-only reply.
    #[derive(Debug)]
    pub enum Init {}
    /// A request the codec refused, or one the layer does not serve: an
    /// error only.
    #[derive(Debug)]
    pub enum ErrorOnly {}
}

/// The one reply a decoded request takes, in the shapes `S` allows. Only
/// the decoder makes one, it is neither `Clone` nor `Copy`, and every
/// encoder consumes it.
pub struct ReplyTo<S> {
    unique: u64,
    /// For [`shape::Data`], the most bytes the request asked for.
    limit: u32,
    shape: PhantomData<S>,
}

impl<S> ReplyTo<S> {
    pub(crate) const fn new(unique: u64, limit: u32) -> Self {
        Self {
            unique,
            limit,
            shape: PhantomData,
        }
    }

    /// The request's id, which the reply repeats.
    #[must_use]
    pub const fn unique(&self) -> u64 {
        self.unique
    }

    /// An error reply: the header alone, with the negative error number.
    #[must_use]
    pub fn error(self, errno: Errno) -> Reply {
        // An `Errno` is positive and small, so its negation cannot
        // overflow; the fallback is never taken.
        let negative = errno.get().checked_neg().unwrap_or(-Errno::EIO.get());
        encode(self.unique, negative, 0, |_| {})
    }
}

impl<S> fmt::Debug for ReplyTo<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplyTo")
            .field("shape", &core::any::type_name::<S>())
            .field("unique", &self.unique)
            .field("limit", &self.limit)
            .finish()
    }
}

impl<S> PartialEq for ReplyTo<S> {
    fn eq(&self, other: &Self) -> bool {
        self.unique == other.unique && self.limit == other.limit
    }
}

impl<S> Eq for ReplyTo<S> {}

/// An encoder's refusal, with the reply still owed, so that the layer can
/// answer with an error instead (fuse-codec.md 7, point 4).
#[derive(Debug, PartialEq, Eq)]
pub struct Refused<S> {
    /// Why the value was refused.
    pub error: EncodeError,
    /// The reply, unused.
    pub reply: ReplyTo<S>,
}

/// A reply with this error and the payload `write` appends. `len` is
/// filled in after the payload. Every payload is a fixed size or at most
/// [`MAX_DATA`](crate::MAX_DATA), so the length fits a `u32`; were it not to, the reply
/// would carry `len` zero, which the kernel refuses rather than misreads.
fn encode(unique: u64, error: i32, payload_size: usize, write: impl FnOnce(&mut Vec<u8>)) -> Reply {
    let mut out = Vec::with_capacity(HEADER_SIZE.saturating_add(payload_size));
    put_u32(&mut out, 0);
    put_u32(&mut out, error.cast_unsigned());
    put_u64(&mut out, unique);
    write(&mut out);
    let len = u32::try_from(out.len()).unwrap_or(0);
    if let Some(field) = out.first_chunk_mut::<4>() {
        *field = len.to_le_bytes();
    }
    Reply(out)
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
    put_u32(out, u32::from(attr.mode()));
    put_u32(out, attr.nlink);
    put_u32(out, attr.uid);
    put_u32(out, attr.gid);
    put_u32(out, 0); // rdev: never a device
    put_u32(out, attr.blksize);
    put_u32(out, 0); // flags
}

impl Attr {
    /// The mode shown: the type bits and the shown permission bits. It
    /// fits 16 bits, as `struct fuse_statx` carries it.
    const fn mode(&self) -> u16 {
        self.kind.type_bits() | (self.permissions & SHOWN_PERMISSIONS)
    }
}

impl ReplyTo<shape::Entry> {
    /// LOOKUP's reply: the node, its generation and attributes, with both
    /// timeouts zero.
    #[must_use]
    pub fn entry(self, node: u64, generation: u64, attr: &Attr) -> Reply {
        encode(self.unique, 0, 128, |out| {
            put_u64(out, node);
            put_u64(out, generation);
            put_u64(out, 0); // entry_valid
            put_u64(out, 0); // attr_valid
            put_u32(out, 0); // entry_valid_nsec
            put_u32(out, 0); // attr_valid_nsec
            put_attr(out, attr);
        })
    }
}

impl ReplyTo<shape::Attr> {
    /// GETATTR's reply, with the timeout zero.
    #[must_use]
    pub fn attr(self, attr: &Attr) -> Reply {
        encode(self.unique, 0, 104, |out| {
            put_u64(out, 0); // attr_valid
            put_u32(out, 0); // attr_valid_nsec
            put_u32(out, 0); // dummy
            put_attr(out, attr);
        })
    }
}

impl ReplyTo<shape::Statx> {
    /// STATX's reply (fuse-codec.md 5.1, point 5): the basic fields, and
    /// the birth time when there is one (ADR 0004, N47, point 6), with the
    /// timeout zero.
    #[must_use]
    pub fn statx(self, attr: &Attr, birth: Option<Timestamp>) -> Reply {
        let mask = match birth {
            Some(_) => STATX_BASIC_STATS | STATX_BTIME,
            None => STATX_BASIC_STATS,
        };
        encode(self.unique, 0, STATX_OUT_SIZE, |out| {
            put_u64(out, 0); // attr_valid
            put_u32(out, 0); // attr_valid_nsec
            put_u32(out, 0); // flags
            out.extend_from_slice(&[0; 16]); // spare
            put_u32(out, mask);
            put_u32(out, attr.blksize);
            put_u64(out, 0); // attributes
            put_u32(out, attr.nlink);
            put_u32(out, attr.uid);
            put_u32(out, attr.gid);
            put_u16(out, attr.mode());
            put_u16(out, 0); // spare
            put_u64(out, attr.ino);
            put_u64(out, attr.size);
            put_u64(out, attr.blocks);
            put_u64(out, 0); // attributes_mask
            put_sx_time(out, attr.atime);
            put_sx_time(out, birth.unwrap_or(Timestamp::EPOCH));
            put_sx_time(out, attr.ctime);
            put_sx_time(out, attr.mtime);
            out.extend_from_slice(&[0; 16]); // rdev and dev: never a device
            out.extend_from_slice(&[0; 112]); // spare
        })
    }
}

/// Appends `struct fuse_sx_time` (16 bytes): seconds as an `i64`, which
/// a [`Timestamp`]'s seconds always fit, nanoseconds and a reserved word.
fn put_sx_time(out: &mut Vec<u8>, time: Timestamp) {
    put_u64(out, time.seconds);
    put_u32(out, time.nanos);
    put_u32(out, 0);
}

fn open_reply(unique: u64, fh: u64, open_flags: u32) -> Reply {
    encode(unique, 0, 16, |out| {
        put_u64(out, fh);
        put_u32(out, open_flags);
        put_u32(out, 0); // padding, or backing_id from 7.40
    })
}

impl ReplyTo<shape::Open> {
    /// OPEN's reply: the handle, with direct I/O and no other open flag.
    #[must_use]
    pub fn open(self, fh: u64) -> Reply {
        open_reply(self.unique, fh, FOPEN_DIRECT_IO)
    }
}

impl ReplyTo<shape::Opendir> {
    /// OPENDIR's reply: the handle, with no open flag, so the directory is
    /// not cached.
    #[must_use]
    pub fn opendir(self, fh: u64) -> Reply {
        open_reply(self.unique, fh, 0)
    }
}

impl ReplyTo<shape::Data> {
    /// READ's reply: the data.
    ///
    /// # Errors
    ///
    /// [`EncodeError::DataTooLong`] over the size the request asked for,
    /// which the decoder capped at [`MAX_DATA`](crate::MAX_DATA).
    pub fn data(self, bytes: &[u8]) -> Result<Reply, Refused<shape::Data>> {
        if u32::try_from(bytes.len()).map_or(true, |len| len > self.limit) {
            return Err(Refused {
                error: EncodeError::DataTooLong,
                reply: self,
            });
        }
        Ok(encode(self.unique, 0, bytes.len(), |out| {
            out.extend_from_slice(bytes);
        }))
    }
}

impl ReplyTo<shape::Readlink> {
    /// READLINK's reply: the target's bytes, with no NUL.
    ///
    /// # Errors
    ///
    /// [`EncodeError::LinkTarget`] for an empty target, one over 4,095
    /// bytes, or one holding a NUL.
    pub fn readlink(self, target: &[u8]) -> Result<Reply, Refused<shape::Readlink>> {
        if target.is_empty() || target.len() > MAX_LINK_TARGET || target.contains(&0) {
            return Err(Refused {
                error: EncodeError::LinkTarget,
                reply: self,
            });
        }
        Ok(encode(self.unique, 0, target.len(), |out| {
            out.extend_from_slice(target);
        }))
    }
}

impl ReplyTo<shape::Statfs> {
    /// STATFS's reply, with `namelen` 255.
    #[must_use]
    pub fn statfs(self, value: &StatFs) -> Reply {
        encode(self.unique, 0, 80, |out| {
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
}

impl ReplyTo<shape::Empty> {
    /// A success reply with no payload.
    #[must_use]
    pub fn ok(self) -> Reply {
        encode(self.unique, 0, 0, |_| {})
    }
}

impl ReplyTo<shape::Init> {
    /// INIT's reply for a settled protocol (5.5, point 5): its flags are
    /// those [`crate::init::select_flags`] chose, low half in `flags` and
    /// high half in `flags2`.
    #[must_use]
    pub fn init(self, negotiated: &Negotiated) -> Reply {
        encode(self.unique, 0, INIT_OUT_SIZE, |out| {
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

    /// INIT's reply to a kernel with a newer major: only major 7, after
    /// which the kernel sends a new INIT (fuse-codec.md 5.3, point 1).
    #[must_use]
    pub fn offer_major(self) -> Reply {
        encode(self.unique, 0, 4, |out| put_u32(out, MAJOR))
    }
}
