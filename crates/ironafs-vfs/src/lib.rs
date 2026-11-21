// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract between IronAFS's core and its operating-system layers
//! (ADR 0004, C7): the values that cross it.
//!
//! - who acts: caller and handle capabilities, which only the core mints
//!   ([`CallerCap`], [`HandleCap`], [`Mint`]; C8, C13), from the operating
//!   system's own statement of the caller ([`Attestation`]);
//! - when: the ingress record of each request, with its arrival and cancel
//!   token ([`Ingress`]; C39, point 3);
//! - what failed: one semantic error type ([`VfsError`]; C46);
//! - why a write is committed ([`CommitReason`]; C9);
//! - what changed: the invalidation stream and its acknowledgements
//!   ([`Event`], [`Ack`], [`Effects`]; C17);
//! - room to grow: bounded extension lists ([`Extensions`]; N41, point 7),
//!   and the reserved operations refused as not supported ([`Reserved`];
//!   N46, point 1);
//! - names: the raw bytes of an entry's name ([`Name`]) and the Linux name
//!   codec ([`linux`]; L29).
//!
//! It is a pure crate (C4, point 1): no I/O, no `unsafe`, no standard
//! library. Every value a layer hands the core through it is untrusted
//! input, so nothing here panics on one.

#![no_std]

extern crate alloc;

mod capability;
mod commit;
mod error;
mod extension;
mod ingress;
mod invalidation;
mod name;
mod reserved;

pub use capability::{
    Attestation, BindingId, CallerCap, ContextId, CredentialGeneration, HandleCap, HandleId, Mint,
};
pub use commit::CommitReason;
pub use error::{Surface, VfsError};
pub use extension::{Extension, ExtensionError, Extensions, MAX_EXTENSION_BYTES, MAX_EXTENSIONS};
pub use ingress::{Ingress, RequestId};
pub use invalidation::{
    Ack, Effects, Entries, Event, Invalidation, NodeId, Overflow, Revision, Rights, Scope,
};
pub use name::{MAX_NAME_BYTES, Name, NameError, linux};
pub use reserved::Reserved;
