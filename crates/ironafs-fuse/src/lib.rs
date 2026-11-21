// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's Linux layer (ADR 0004, L2): its own FUSE protocol codec,
//! written from the kernel's published interface
//! (`docs/specs/fuse-codec.md`).

pub mod errno;
mod wire;

pub mod opcode;

pub use errno::Errno;
pub use opcode::Opcode;
