// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding one request read from `/dev/fuse` (fuse-codec.md 5.4).
//!
//! The kernel's requests are untrusted input (ADR 0004, overview,
//! constraint 4): every length is checked against the bytes read and the
//! opcode's layout before a field is used, every read goes through a
//! cursor that cannot pass the end, and nothing is allocated from a
//! decoded length but a LOOKUP's name, copied only after its 255-byte
//! cap is checked. A decoded request borrows the read buffer, and carries
//! the one [`ReplyTo`] it takes, if it takes one.

use ironafs_vfs::{Name, NameError, linux};

use crate::init::{InitDefect, InitRequest, decode_init};
use crate::reply::{ReplyTo, shape};
use crate::wire::Reader;
use crate::{Errno, MAX_DATA, Opcode, errno};

#[cfg(test)]
mod tests;

/// The size of the request header.
const HEADER_SIZE: usize = 40;

/// The first minor whose header carries `total_extlen`.
const EXTENSIONS_MINOR: u32 = 38;

/// `FUSE_GETATTR_FH`: GETATTR's `fh` is set.
const GETATTR_FH: u32 = 1 << 0;
/// `FUSE_READ_LOCKOWNER`: READ's `lock_owner` is valid.
const READ_LOCKOWNER: u32 = 1 << 1;
/// `FUSE_RELEASE_FLUSH`.
const RELEASE_FLUSH: u32 = 1 << 0;
/// `FUSE_RELEASE_FLOCK_UNLOCK`.
const RELEASE_FLOCK_UNLOCK: u32 = 1 << 1;
/// The first minor whose OPEN body carries `open_flags` in its second
/// word, which is reserved below it.
const OPEN_FLAGS_MINOR: u32 = 33;

/// The request header's fields the layer uses (fuse-codec.md 5.1,
/// point 2). They are handed on unjudged: the layer looks each id up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The opcode's number, known or not.
    pub opcode: u32,
    /// The request's id, which its reply repeats.
    pub unique: u64,
    /// The node the request acts on.
    pub nodeid: u64,
    /// The caller's user id, filled by the kernel.
    pub uid: u32,
    /// The caller's group id, filled by the kernel.
    pub gid: u32,
    /// The caller's process id, filled by the kernel.
    pub pid: u32,
}

/// Where the connection is: before INIT is answered, or after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Only INIT is accepted.
    AwaitingInit,
    /// INIT settled this minor.
    Negotiated {
        /// The negotiated minor.
        minor: u32,
    },
}

/// A READ or READDIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    /// The handle the layer gave at open.
    pub fh: u64,
    /// For READ, a byte offset at most `i64::MAX`; for READDIR, the
    /// layer's own position cookie.
    pub offset: u64,
    /// The most bytes wanted, at most [`MAX_DATA`].
    pub size: u32,
    /// The lock owner, when the kernel says it is valid.
    pub lock_owner: Option<u64>,
    /// The file's open flags, as `open(2)` took them.
    pub flags: u32,
}

/// An OPEN or OPENDIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Open {
    /// The flags `open(2)` was called with, unjudged.
    pub flags: u32,
}

/// A RELEASE or RELEASEDIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    /// The handle the layer gave at open.
    pub fh: u64,
    /// The file's open flags.
    pub flags: u32,
    /// `FUSE_RELEASE_FLUSH`: flush before releasing.
    pub flush: bool,
    /// `FUSE_RELEASE_FLOCK_UNLOCK`: drop the owner's `flock` locks.
    pub flock_unlock: bool,
    /// The lock owner.
    pub lock_owner: u64,
}

/// One record of a `BATCH_FORGET`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forget {
    /// The node whose lookups are dropped.
    pub node: u64,
    /// How many.
    pub nlookup: u64,
}

/// The size of one `BATCH_FORGET` record: `nodeid` and `nlookup`, a u64
/// each.
const FORGET_RECORD: usize = 16;

/// The records of a `BATCH_FORGET`, read in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForgetBatch<'a> {
    /// The whole records, exactly `count` of them, checked by the decoder.
    records: &'a [[u8; FORGET_RECORD]],
}

impl ForgetBatch<'_> {
    /// How many records the batch holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the batch holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The records, in order. Each field is taken from its own eight bytes
    /// of the record's array, so no width can be misread or defaulted.
    pub fn iter(&self) -> impl Iterator<Item = Forget> {
        self.records.iter().map(|record| {
            let [
                n0,
                n1,
                n2,
                n3,
                n4,
                n5,
                n6,
                n7,
                c0,
                c1,
                c2,
                c3,
                c4,
                c5,
                c6,
                c7,
            ] = *record;
            Forget {
                node: u64::from_le_bytes([n0, n1, n2, n3, n4, n5, n6, n7]),
                nlookup: u64::from_le_bytes([c0, c1, c2, c3, c4, c5, c6, c7]),
            }
        })
    }
}

/// A decoded request (fuse-codec.md 2, point 1), with the reply it takes.
/// The three requests that take no reply carry none (5.6, point 1).
#[derive(Debug, PartialEq, Eq)]
pub enum Request<'a> {
    /// INIT: negotiate with [`crate::init::negotiate`].
    Init {
        /// The kernel's offer, checked.
        init: InitRequest,
        /// The reply.
        reply: ReplyTo<shape::Init>,
    },
    /// DESTROY.
    Destroy {
        /// The reply.
        reply: ReplyTo<shape::Empty>,
    },
    /// LOOKUP of one name in the header's node.
    Lookup {
        /// The name, checked by the Linux name codec.
        name: Name,
        /// The reply.
        reply: ReplyTo<shape::Entry>,
    },
    /// FORGET of the header's node. No reply.
    Forget {
        /// How many lookups are dropped.
        nlookup: u64,
    },
    /// `BATCH_FORGET`. No reply.
    BatchForget(ForgetBatch<'a>),
    /// GETATTR.
    Getattr {
        /// The handle, when the kernel names one.
        fh: Option<u64>,
        /// The reply.
        reply: ReplyTo<shape::Attr>,
    },
    /// STATX.
    Statx {
        /// The handle, when the kernel names one.
        fh: Option<u64>,
        /// The `statx(2)` synchronisation flags, unjudged.
        flags: u32,
        /// The fields asked for, unjudged: the reply says which it fills.
        mask: u32,
        /// The reply.
        reply: ReplyTo<shape::Statx>,
    },
    /// READLINK.
    Readlink {
        /// The reply.
        reply: ReplyTo<shape::Readlink>,
    },
    /// OPEN.
    Open {
        /// The request.
        open: Open,
        /// The reply.
        reply: ReplyTo<shape::Open>,
    },
    /// OPENDIR.
    Opendir {
        /// The request.
        open: Open,
        /// The reply.
        reply: ReplyTo<shape::Opendir>,
    },
    /// READ.
    Read {
        /// The request.
        read: Read,
        /// The reply, of at most `read.size` bytes.
        reply: ReplyTo<shape::Data>,
    },
    /// READDIR.
    Readdir {
        /// The request.
        read: Read,
        /// The reply.
        reply: ReplyTo<shape::Readdir>,
    },
    /// RELEASE.
    Release {
        /// The request.
        release: Release,
        /// The reply.
        reply: ReplyTo<shape::Empty>,
    },
    /// RELEASEDIR.
    Releasedir {
        /// The request.
        release: Release,
        /// The reply.
        reply: ReplyTo<shape::Empty>,
    },
    /// FLUSH.
    Flush {
        /// The handle.
        fh: u64,
        /// The lock owner.
        lock_owner: u64,
        /// The reply.
        reply: ReplyTo<shape::Empty>,
    },
    /// STATFS.
    Statfs {
        /// The reply.
        reply: ReplyTo<shape::Statfs>,
    },
    /// ACCESS.
    Access {
        /// The `access(2)` mask, unjudged.
        mask: u32,
        /// The reply.
        reply: ReplyTo<shape::Empty>,
    },
    /// INTERRUPT of an earlier request. No reply.
    Interrupt {
        /// The interrupted request's `unique`.
        target: u64,
    },
    /// An opcode of the interface this codec does not decode yet. The
    /// layer answers [`Errno::EOPNOTSUPP`], never `ENOSYS` (5.4, point 9).
    NotDecoded {
        /// The opcode.
        opcode: Opcode,
        /// The reply.
        reply: ReplyTo<shape::ErrorOnly>,
    },
    /// An opcode the interface does not define. The layer answers
    /// [`Errno::EOPNOTSUPP`].
    Unknown {
        /// Its number.
        opcode: u32,
        /// The reply.
        reply: ReplyTo<shape::ErrorOnly>,
    },
}

/// A request whose header cannot be trusted. It gets no reply; the layer
/// ends the connection (fuse-codec.md 5.4, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// Fewer bytes than a header.
    Short {
        /// The bytes read.
        read: usize,
    },
    /// The header's `len` is not the number of bytes read.
    LengthMismatch {
        /// The header's `len`.
        declared: u32,
        /// The bytes read.
        read: usize,
    },
}

/// Why a request with a sound header was refused (fuse-codec.md 5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// A request other than INIT before INIT.
    BeforeInit,
    /// A second INIT.
    InitAgain,
    /// Request extensions, which no selected flag asks for.
    Extensions,
    /// A body of another size than the opcode's.
    BodyLength {
        /// The opcode's size.
        expected: usize,
        /// The body's size.
        found: usize,
    },
    /// An INIT body of another size than its version and flags give.
    InitBody,
    /// An INIT offering a flag its minor does not define.
    InitFlags,
    /// A name with no NUL, or bytes after it.
    NameUnterminated,
    /// Bytes that are not a name.
    Name(NameError),
    /// A bit the negotiated protocol does not define in a flags field.
    UnknownFlags,
    /// A size or offset over its cap.
    OutOfRange,
    /// A `BATCH_FORGET` count that does not match its body.
    ForgetCount,
}

impl Malformed {
    /// The error a refused request is answered with.
    const fn errno(self) -> Errno {
        match self {
            Self::Name(error) => errno::for_name_error(error),
            Self::OutOfRange => Errno::EINVAL,
            Self::BeforeInit
            | Self::InitAgain
            | Self::Extensions
            | Self::BodyLength { .. }
            | Self::InitBody
            | Self::InitFlags
            | Self::NameUnterminated
            | Self::UnknownFlags
            | Self::ForgetCount => Errno::EIO,
        }
    }
}

/// A refused request, with its header and, unless it takes no reply, the
/// reply it is owed.
#[derive(Debug, PartialEq, Eq)]
pub struct RequestError {
    /// The request's header.
    pub header: Header,
    /// Why it was refused.
    pub malformed: Malformed,
    /// The reply owed; `None` for a request that takes none (5.6,
    /// point 1).
    reply: Option<ReplyTo<shape::ErrorOnly>>,
}

impl RequestError {
    /// The error the reply carries, or `None` for a request that takes no
    /// reply.
    #[must_use]
    pub fn errno(&self) -> Option<Errno> {
        self.reply.as_ref().map(|_| self.malformed.errno())
    }

    /// The error reply, or `None` for a request that takes no reply.
    #[must_use]
    pub fn into_reply(self) -> Option<crate::reply::Reply> {
        let errno = self.malformed.errno();
        self.reply.map(|reply| reply.error(errno))
    }
}

/// Why [`decode`] refused a request.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The header cannot be trusted; no reply.
    Frame(FrameError),
    /// The body was refused; answer with [`RequestError::into_reply`].
    Request(RequestError),
}

/// Decodes one request: all the bytes one read of `/dev/fuse` returned
/// (fuse-codec.md 5.4, in its order).
///
/// # Errors
///
/// A [`DecodeError::Frame`] if the header cannot be trusted, and a
/// [`DecodeError::Request`] if the body breaks its layout, its phase or a
/// cap.
pub fn decode(bytes: &[u8], phase: Phase) -> Result<(Header, Request<'_>), DecodeError> {
    let (header, extlen, body) = decode_header(bytes)?;
    let opcode = Opcode::from_u32(header.opcode);
    let refuse = |malformed| {
        let replies = opcode.is_none_or(Opcode::needs_reply);
        DecodeError::Request(RequestError {
            header,
            malformed,
            reply: replies.then(|| ReplyTo::new(header.unique, 0)),
        })
    };
    let minor = match (phase, opcode) {
        (Phase::AwaitingInit, Some(Opcode::Init)) => 0,
        (Phase::AwaitingInit, _) => return Err(refuse(Malformed::BeforeInit)),
        (Phase::Negotiated { .. }, Some(Opcode::Init)) => {
            return Err(refuse(Malformed::InitAgain));
        }
        (Phase::Negotiated { minor }, _) => {
            if minor >= EXTENSIONS_MINOR && extlen != 0 {
                return Err(refuse(Malformed::Extensions));
            }
            minor
        }
    };
    let Some(opcode) = opcode else {
        return Ok((
            header,
            Request::Unknown {
                opcode: header.opcode,
                reply: ReplyTo::new(header.unique, 0),
            },
        ));
    };
    decode_body(opcode, body, header.unique, minor)
        .map(|request| (header, request))
        .map_err(refuse)
}

/// The header, its `total_extlen` field and the body.
fn decode_header(bytes: &[u8]) -> Result<(Header, u16, &[u8]), DecodeError> {
    let frame = |error| DecodeError::Frame(error);
    let short = || frame(FrameError::Short { read: bytes.len() });
    let (head, body) = bytes.split_at_checked(HEADER_SIZE).ok_or_else(short)?;
    let mut reader = Reader::new(head);
    let len = reader.u32().ok_or_else(short)?;
    if usize::try_from(len).ok() != Some(bytes.len()) {
        return Err(frame(FrameError::LengthMismatch {
            declared: len,
            read: bytes.len(),
        }));
    }
    let header = Header {
        opcode: reader.u32().ok_or_else(short)?,
        unique: reader.u64().ok_or_else(short)?,
        nodeid: reader.u64().ok_or_else(short)?,
        uid: reader.u32().ok_or_else(short)?,
        gid: reader.u32().ok_or_else(short)?,
        pid: reader.u32().ok_or_else(short)?,
    };
    let extlen = reader.u16().ok_or_else(short)?;
    Ok((header, extlen, body))
}

/// Checks that `body` has exactly `size` bytes and returns a reader over
/// it.
fn exact(body: &[u8], size: usize) -> Result<Reader<'_>, Malformed> {
    if body.len() == size {
        Ok(Reader::new(body))
    } else {
        Err(Malformed::BodyLength {
            expected: size,
            found: body.len(),
        })
    }
}

/// A field of a body whose length [`exact`] already checked. The `None`
/// branch cannot be taken; it is mapped to a length error, never a panic.
fn field<T>(value: Option<T>, body: &[u8], size: usize) -> Result<T, Malformed> {
    value.ok_or(Malformed::BodyLength {
        expected: size,
        found: body.len(),
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per opcode of the table, so one place shows which are decoded"
)]
fn decode_body(
    opcode: Opcode,
    body: &[u8],
    unique: u64,
    minor: u32,
) -> Result<Request<'_>, Malformed> {
    Ok(match opcode {
        Opcode::Init => Request::Init {
            init: decode_init(body).map_err(|defect| match defect {
                InitDefect::Body => Malformed::InitBody,
                InitDefect::Flags => Malformed::InitFlags,
            })?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Destroy => {
            exact(body, 0)?;
            Request::Destroy {
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Readlink => {
            exact(body, 0)?;
            Request::Readlink {
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Statfs => {
            exact(body, 0)?;
            Request::Statfs {
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Lookup => Request::Lookup {
            name: decode_name(body)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Forget => {
            let mut reader = exact(body, 8)?;
            Request::Forget {
                nlookup: field(reader.u64(), body, 8)?,
            }
        }
        Opcode::BatchForget => Request::BatchForget(decode_batch_forget(body)?),
        Opcode::Getattr => {
            let (fh, _) = decode_getattr(body, 16)?;
            Request::Getattr {
                fh,
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Open => Request::Open {
            open: decode_open(body, minor)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Opendir => Request::Opendir {
            open: decode_open(body, minor)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Read => {
            let read = decode_read(body)?;
            let end = read
                .offset
                .checked_add(u64::from(read.size))
                .ok_or(Malformed::OutOfRange)?;
            if i64::try_from(end).is_err() {
                return Err(Malformed::OutOfRange);
            }
            Request::Read {
                read,
                reply: ReplyTo::new(unique, read.size),
            }
        }
        Opcode::Readdir => Request::Readdir {
            read: decode_read(body)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Release => Request::Release {
            release: decode_release(body)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Releasedir => Request::Releasedir {
            release: decode_release(body)?,
            reply: ReplyTo::new(unique, 0),
        },
        Opcode::Flush => {
            let mut reader = exact(body, 24)?;
            let fh = field(reader.u64(), body, 24)?;
            field(reader.skip(8), body, 24)?;
            Request::Flush {
                fh,
                lock_owner: field(reader.u64(), body, 24)?,
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Access => {
            let mut reader = exact(body, 8)?;
            Request::Access {
                mask: field(reader.u32(), body, 8)?,
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Statx => {
            let (fh, mut reader) = decode_getattr(body, 24)?;
            Request::Statx {
                fh,
                flags: field(reader.u32(), body, 24)?,
                mask: field(reader.u32(), body, 24)?,
                reply: ReplyTo::new(unique, 0),
            }
        }
        Opcode::Interrupt => {
            let mut reader = exact(body, 8)?;
            Request::Interrupt {
                target: field(reader.u64(), body, 8)?,
            }
        }
        Opcode::Setattr
        | Opcode::Symlink
        | Opcode::Mknod
        | Opcode::Mkdir
        | Opcode::Unlink
        | Opcode::Rmdir
        | Opcode::Rename
        | Opcode::Link
        | Opcode::Write
        | Opcode::Fsync
        | Opcode::Setxattr
        | Opcode::Getxattr
        | Opcode::Listxattr
        | Opcode::Removexattr
        | Opcode::Fsyncdir
        | Opcode::Getlk
        | Opcode::Setlk
        | Opcode::Setlkw
        | Opcode::Create
        | Opcode::Bmap
        | Opcode::Ioctl
        | Opcode::Poll
        | Opcode::NotifyReply
        | Opcode::Fallocate
        | Opcode::Readdirplus
        | Opcode::Rename2
        | Opcode::Lseek
        | Opcode::CopyFileRange
        | Opcode::Setupmapping
        | Opcode::Removemapping
        | Opcode::Syncfs
        | Opcode::Tmpfile
        | Opcode::CopyFileRange64 => Request::NotDecoded {
            opcode,
            reply: ReplyTo::new(unique, 0),
        },
    })
}

/// GETATTR's body, or the first 16 bytes of STATX's: `getattr_flags`, a
/// reserved word and `fh`; `size` is the whole body's size.
fn decode_getattr(body: &[u8], size: usize) -> Result<(Option<u64>, Reader<'_>), Malformed> {
    let mut reader = exact(body, size)?;
    let flags = field(reader.u32(), body, size)?;
    field(reader.skip(4), body, size)?;
    let fh = field(reader.u64(), body, size)?;
    if flags & !GETATTR_FH != 0 {
        return Err(Malformed::UnknownFlags);
    }
    Ok(((flags & GETATTR_FH != 0).then_some(fh), reader))
}

/// A name and its terminating NUL, with no other NUL (5.4, point 5).
fn decode_name(body: &[u8]) -> Result<Name, Malformed> {
    let (last, name) = body.split_last().ok_or(Malformed::NameUnterminated)?;
    if *last != 0 || name.contains(&0) {
        return Err(Malformed::NameUnterminated);
    }
    linux::decode(name).map_err(Malformed::Name)
}

fn decode_batch_forget(body: &[u8]) -> Result<ForgetBatch<'_>, Malformed> {
    let (head, rest) = body.split_at_checked(8).ok_or(Malformed::BodyLength {
        expected: 8,
        found: body.len(),
    })?;
    let count = field(Reader::new(head).u32(), body, 8)?;
    let (records, trailing) = rest.as_chunks::<FORGET_RECORD>();
    if !trailing.is_empty() || usize::try_from(count).ok() != Some(records.len()) {
        return Err(Malformed::ForgetCount);
    }
    Ok(ForgetBatch { records })
}

/// OPEN's and OPENDIR's body (5.4, point 6). The second word is reserved
/// below 7.33 and ignored there; from 7.33 it is `open_flags`, whose one
/// defined bit, `FUSE_OPEN_KILL_SUIDGID`, comes only with
/// `FUSE_HANDLE_KILLPRIV_V2`, which is never selected, so any bit is
/// refused.
fn decode_open(body: &[u8], minor: u32) -> Result<Open, Malformed> {
    let mut reader = exact(body, 8)?;
    let flags = field(reader.u32(), body, 8)?;
    let open_flags = field(reader.u32(), body, 8)?;
    if minor >= OPEN_FLAGS_MINOR && open_flags != 0 {
        return Err(Malformed::UnknownFlags);
    }
    Ok(Open { flags })
}

/// READ's and READDIR's shared body, with its size cap (5.2).
fn decode_read(body: &[u8]) -> Result<Read, Malformed> {
    let mut reader = exact(body, 40)?;
    let fh = field(reader.u64(), body, 40)?;
    let offset = field(reader.u64(), body, 40)?;
    let size = field(reader.u32(), body, 40)?;
    let read_flags = field(reader.u32(), body, 40)?;
    let lock_owner = field(reader.u64(), body, 40)?;
    let flags = field(reader.u32(), body, 40)?;
    if read_flags & !READ_LOCKOWNER != 0 {
        return Err(Malformed::UnknownFlags);
    }
    if size > MAX_DATA {
        return Err(Malformed::OutOfRange);
    }
    Ok(Read {
        fh,
        offset,
        size,
        lock_owner: (read_flags & READ_LOCKOWNER != 0).then_some(lock_owner),
        flags,
    })
}

fn decode_release(body: &[u8]) -> Result<Release, Malformed> {
    let mut reader = exact(body, 24)?;
    let fh = field(reader.u64(), body, 24)?;
    let flags = field(reader.u32(), body, 24)?;
    let release_flags = field(reader.u32(), body, 24)?;
    let lock_owner = field(reader.u64(), body, 24)?;
    if release_flags & !(RELEASE_FLUSH | RELEASE_FLOCK_UNLOCK) != 0 {
        return Err(Malformed::UnknownFlags);
    }
    Ok(Release {
        fh,
        flags,
        flush: release_flags & RELEASE_FLUSH != 0,
        flock_unlock: release_flags & RELEASE_FLOCK_UNLOCK != 0,
        lock_owner,
    })
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Short { read } => write!(formatter, "{read} bytes, fewer than a request header"),
            Self::LengthMismatch { declared, read } => {
                write!(
                    formatter,
                    "the header says {declared} bytes, {read} were read"
                )
            }
        }
    }
}

impl core::fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BeforeInit => formatter.write_str("a request before init"),
            Self::InitAgain => formatter.write_str("a second init"),
            Self::Extensions => {
                formatter.write_str("request extensions, which were not negotiated")
            }
            Self::BodyLength { expected, found } => {
                write!(formatter, "a body of {found} bytes where {expected} belong")
            }
            Self::InitBody => {
                formatter.write_str("an init body of another size than its version and flags give")
            }
            Self::InitFlags => {
                formatter.write_str("an init offering a flag its minor does not define")
            }
            Self::NameUnterminated => formatter.write_str("a name not ended by its one nul"),
            Self::Name(error) => write!(formatter, "not a name: {error}"),
            Self::UnknownFlags => formatter.write_str("an undefined bit in a flags field"),
            Self::OutOfRange => formatter.write_str("a size or offset over its cap"),
            Self::ForgetCount => {
                formatter.write_str("a batch forget count that does not match its body")
            }
        }
    }
}

impl core::fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "fuse request {} (opcode {}): {}",
            self.header.unique, self.header.opcode, self.malformed
        )
    }
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "untrusted fuse frame: {error}"),
            Self::Request(error) => error.fmt(formatter),
        }
    }
}

impl core::error::Error for FrameError {}
impl core::error::Error for Malformed {}
impl core::error::Error for RequestError {}
impl core::error::Error for DecodeError {}
