// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's Linux layer (ADR 0004, L2): its own FUSE protocol codec,
//! written from the kernel's published interface
//! (`docs/specs/fuse-codec.md`).

pub mod errno;
pub mod init;
mod wire;

pub mod opcode;
pub mod reply;
pub mod request;

pub use errno::Errno;
pub use opcode::Opcode;

/// The most file data, or encoded directory entries, one request or reply
/// carries: 1 MiB (vfs-contract 5.2, point 3; fuse-codec.md 5.2).
pub const MAX_DATA: u32 = 1_048_576;
