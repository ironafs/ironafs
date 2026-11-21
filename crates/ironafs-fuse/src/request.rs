// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding one request read from `/dev/fuse` (fuse-codec.md 5.4).
//!
//! The kernel's requests are untrusted input (ADR 0004, overview,
//! constraint 4): every length is checked against the bytes read and the
//! opcode's layout before a field is used, every read goes through a
//! cursor that cannot pass the end, and nothing is allocated from a
//! decoded length. A decoded request borrows the read buffer.

use ironafs_vfs::{Name, NameError, linux};

use crate::init::{InitRequest, decode_init};
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

/// The records of a `BATCH_FORGET`, read in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForgetBatch<'a> {
    /// Exactly `16 × count` bytes, checked by the decoder.
    records: &'a [u8],
}

impl ForgetBatch<'_> {
    /// How many records the batch holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len() / FORGET_RECORD
    }

    /// Whether the batch holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The records, in order.
    pub fn iter(&self) -> impl Iterator<Item = Forget> {
        let (records, _) = self.records.as_chunks::<FORGET_RECORD>();
        records.iter().map(|record| {
            let (node, nlookup) = record.split_at(8);
            Forget {
                node: u64::from_le_bytes(node.try_into().unwrap_or_default()),
                nlookup: u64::from_le_bytes(nlookup.try_into().unwrap_or_default()),
            }
        })
    }
}

/// The size of one `BATCH_FORGET` record.
const FORGET_RECORD: usize = 16;

/// A decoded request (fuse-codec.md 2, point 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request<'a> {
    /// INIT: negotiate with [`crate::init::negotiate`].
    Init(InitRequest),
    /// DESTROY.
    Destroy,
    /// LOOKUP of one name in the header's node.
    Lookup {
        /// The name, checked by the Linux name codec.
        name: Name,
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
    },
    /// READLINK.
    Readlink,
    /// OPEN.
    Open(Open),
    /// OPENDIR.
    Opendir(Open),
    /// READ.
    Read(Read),
    /// READDIR.
    Readdir(Read),
    /// RELEASE.
    Release(Release),
    /// RELEASEDIR.
    Releasedir(Release),
    /// FLUSH.
    Flush {
        /// The handle.
        fh: u64,
        /// The lock owner.
        lock_owner: u64,
    },
    /// STATFS.
    Statfs,
    /// ACCESS.
    Access {
        /// The `access(2)` mask, unjudged.
        mask: u32,
    },
    /// INTERRUPT of an earlier request. No reply.
    Interrupt {
        /// The interrupted request's `unique`.
        target: u64,
    },
    /// An opcode of the interface this codec does not decode yet. The
    /// layer answers [`Errno::EOPNOTSUPP`], never `ENOSYS` (5.4, point 9).
    NotDecoded(Opcode),
    /// An opcode the interface does not define. The layer answers
    /// [`Errno::EOPNOTSUPP`].
    Unknown {
        /// Its number.
        opcode: u32,
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
    /// An INIT body too short for its fields.
    InitBody,
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

/// A refused request, with its header so that the reply can name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestError {
    /// The request's header.
    pub header: Header,
    /// Why it was refused.
    pub malformed: Malformed,
}

impl RequestError {
    /// The error to answer with, or `None` for a request that takes no
    /// reply (fuse-codec.md 5.6, point 1).
    #[must_use]
    pub fn reply(&self) -> Option<Errno> {
        if let Some(opcode) = Opcode::from_u32(self.header.opcode)
            && !opcode.needs_reply()
        {
            return None;
        }
        Some(match self.malformed {
            Malformed::Name(error) => errno::for_name_error(error),
            Malformed::OutOfRange => Errno::EINVAL,
            Malformed::BeforeInit
            | Malformed::InitAgain
            | Malformed::Extensions
            | Malformed::BodyLength { .. }
            | Malformed::InitBody
            | Malformed::NameUnterminated
            | Malformed::UnknownFlags
            | Malformed::ForgetCount => Errno::EIO,
        })
    }
}

/// Why [`decode`] refused a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The header cannot be trusted; no reply.
    Frame(FrameError),
    /// The body was refused; answer with [`RequestError::reply`].
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
    let refuse = |malformed| DecodeError::Request(RequestError { header, malformed });
    let opcode = Opcode::from_u32(header.opcode);
    match (phase, opcode) {
        (Phase::AwaitingInit, Some(Opcode::Init)) => {}
        (Phase::AwaitingInit, _) => return Err(refuse(Malformed::BeforeInit)),
        (Phase::Negotiated { .. }, Some(Opcode::Init)) => {
            return Err(refuse(Malformed::InitAgain));
        }
        (Phase::Negotiated { minor }, _) => {
            if minor >= EXTENSIONS_MINOR && extlen != 0 {
                return Err(refuse(Malformed::Extensions));
            }
        }
    }
    let Some(opcode) = opcode else {
        return Ok((
            header,
            Request::Unknown {
                opcode: header.opcode,
            },
        ));
    };
    decode_body(opcode, body)
        .map(|request| (header, request))
        .map_err(refuse)
}

/// The header, its `total_extlen` field and the body.
fn decode_header(bytes: &[u8]) -> Result<(Header, u16, &[u8]), DecodeError> {
    let frame = |error| DecodeError::Frame(error);
    let short = frame(FrameError::Short { read: bytes.len() });
    let (head, body) = bytes.split_at_checked(HEADER_SIZE).ok_or(short)?;
    let mut reader = Reader::new(head);
    let len = reader.u32().ok_or(short)?;
    if usize::try_from(len).ok() != Some(bytes.len()) {
        return Err(frame(FrameError::LengthMismatch {
            declared: len,
            read: bytes.len(),
        }));
    }
    let header = Header {
        opcode: reader.u32().ok_or(short)?,
        unique: reader.u64().ok_or(short)?,
        nodeid: reader.u64().ok_or(short)?,
        uid: reader.u32().ok_or(short)?,
        gid: reader.u32().ok_or(short)?,
        pid: reader.u32().ok_or(short)?,
    };
    let extlen = reader.u16().ok_or(short)?;
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

fn decode_body(opcode: Opcode, body: &[u8]) -> Result<Request<'_>, Malformed> {
    Ok(match opcode {
        Opcode::Init => Request::Init(decode_init(body).ok_or(Malformed::InitBody)?),
        Opcode::Destroy => {
            exact(body, 0)?;
            Request::Destroy
        }
        Opcode::Readlink => {
            exact(body, 0)?;
            Request::Readlink
        }
        Opcode::Statfs => {
            exact(body, 0)?;
            Request::Statfs
        }
        Opcode::Lookup => Request::Lookup {
            name: decode_name(body)?,
        },
        Opcode::Forget => {
            let mut reader = exact(body, 8)?;
            Request::Forget {
                nlookup: field(reader.u64(), body, 8)?,
            }
        }
        Opcode::BatchForget => Request::BatchForget(decode_batch_forget(body)?),
        Opcode::Getattr => decode_getattr(body)?,
        Opcode::Open => Request::Open(decode_open(body)?),
        Opcode::Opendir => Request::Opendir(decode_open(body)?),
        Opcode::Read => {
            let read = decode_read(body)?;
            let end = read
                .offset
                .checked_add(u64::from(read.size))
                .ok_or(Malformed::OutOfRange)?;
            if i64::try_from(end).is_err() {
                return Err(Malformed::OutOfRange);
            }
            Request::Read(read)
        }
        Opcode::Readdir => Request::Readdir(decode_read(body)?),
        Opcode::Release => Request::Release(decode_release(body)?),
        Opcode::Releasedir => Request::Releasedir(decode_release(body)?),
        Opcode::Flush => decode_flush(body)?,
        Opcode::Access => {
            let mut reader = exact(body, 8)?;
            Request::Access {
                mask: field(reader.u32(), body, 8)?,
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
        | Opcode::Statx
        | Opcode::CopyFileRange64 => Request::NotDecoded(opcode),
    })
}

fn decode_getattr(body: &[u8]) -> Result<Request<'_>, Malformed> {
    let mut reader = exact(body, 16)?;
    let flags = field(reader.u32(), body, 16)?;
    field(reader.skip(4), body, 16)?;
    let fh = field(reader.u64(), body, 16)?;
    if flags & !GETATTR_FH != 0 {
        return Err(Malformed::UnknownFlags);
    }
    Ok(Request::Getattr {
        fh: (flags & GETATTR_FH != 0).then_some(fh),
    })
}

fn decode_flush(body: &[u8]) -> Result<Request<'_>, Malformed> {
    let mut reader = exact(body, 24)?;
    let fh = field(reader.u64(), body, 24)?;
    field(reader.skip(8), body, 24)?;
    Ok(Request::Flush {
        fh,
        lock_owner: field(reader.u64(), body, 24)?,
    })
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
    let (head, records) = body.split_at_checked(8).ok_or(Malformed::BodyLength {
        expected: 8,
        found: body.len(),
    })?;
    let count = field(Reader::new(head).u32(), body, 8)?;
    let expected = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(FORGET_RECORD))
        .ok_or(Malformed::ForgetCount)?;
    if records.len() != expected {
        return Err(Malformed::ForgetCount);
    }
    Ok(ForgetBatch { records })
}

fn decode_open(body: &[u8]) -> Result<Open, Malformed> {
    let mut reader = exact(body, 8)?;
    // `open_flags`, the second word, is ignored (5.4, point 6).
    Ok(Open {
        flags: field(reader.u32(), body, 8)?,
    })
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
