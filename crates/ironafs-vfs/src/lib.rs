// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The vocabulary of the contract between the core and the layers
//! (`docs/specs/vfs-contract.md`): the name of one directory entry and its
//! Linux codec (5.2; ADR 0004, L29), and the one semantic error type (5.12;
//! ADR 0004, C46).

#![no_std]

extern crate alloc;

mod error;
mod name;

pub use error::{Surface, VfsError};
pub use name::{MAX_NAME_BYTES, Name, NameError, linux};
