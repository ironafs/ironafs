// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's Linux layer (ADR 0004, L2): its own codec for the FUSE
//! protocol, written from the kernel's published interface
//! (`docs/specs/fuse-codec.md`).
//!
//! The codec is pure: a [`session::Session`] decodes the bytes of one read
//! of `/dev/fuse` into a typed request carrying its one reply token, and
//! turns each finished [`reply::Reply`] into the bytes to write back;
//! [`init`] chooses the INIT flags from one reviewed allowlist, and
//! [`errno`] maps the contract's errors to Linux's. It reads no clock and
//! does no I/O. Every request is untrusted input, so nothing here panics
//! on one.
//!
//! ```
//! use ironafs_fuse::init::InitConfig;
//! use ironafs_fuse::reply::InitAnswer;
//! use ironafs_fuse::request::Request;
//! use ironafs_fuse::session::Session;
//!
//! // An INIT at protocol 7.23: a 40-byte header, then major, minor,
//! // max_readahead and flags.
//! let mut bytes = Vec::new();
//! for field in [56u32, 26] {
//!     bytes.extend_from_slice(&field.to_le_bytes()); // len, opcode
//! }
//! bytes.extend_from_slice(&1u64.to_le_bytes()); // unique
//! bytes.extend_from_slice(&[0; 24]); // nodeid, uid, gid, pid, padding
//! for field in [7u32, 23, 131_072, 0] {
//!     bytes.extend_from_slice(&field.to_le_bytes());
//! }
//!
//! let mut session = Session::new();
//! let (header, request) = session.decode(&bytes)?;
//! let Request::Init { reply } = request else { panic!("not an init") };
//! let config = InitConfig::new(131_072, 131_072, 12, 9, 1)?;
//! let (reply, answer) = reply.answer(&config);
//! let InitAnswer::Ready(negotiated) = answer else { panic!("not 7.x") };
//! assert_eq!(negotiated.minor(), 23);
//! let written = session.finish(reply)?;
//! assert_eq!(written.len(), 80);
//! assert_eq!((header.unique, session.minor()), (1, Some(23)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod errno;
pub mod init;
pub mod opcode;
pub mod reply;
pub mod request;
pub mod session;
mod wire;

pub use errno::Errno;
pub use opcode::Opcode;

/// The most file data, or encoded directory entries, one request or reply
/// carries: 1 MiB (vfs-contract 5.2, point 3; fuse-codec.md 5.2).
pub const MAX_DATA: u32 = 1_048_576;
