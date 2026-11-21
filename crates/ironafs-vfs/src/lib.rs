// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract between IronAFS's core and its operating-system layers
//! (ADR 0004, C7).

#![no_std]

extern crate alloc;

mod capability;
mod error;

pub use capability::{
    Attestation, BindingId, CallerCap, ContextId, CredentialGeneration, HandleCap, HandleId, Mint,
};
pub use error::{Surface, VfsError};
