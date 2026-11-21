// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Caller and handle capabilities (ADR 0004, C8 and C13): the only way an
//! operation names who acts.
//!
//! A capability's fields are private to this crate, so a layer can neither
//! write one as a literal nor change one it was given; it gets them only
//! from the core. The core mints them through a [`Mint`] and checks each
//! one it is handed against its own table (binding, context, generation,
//! liveness and nonce), so a capability minted for another binding, or
//! made by a crate that built its own `Mint`, is refused at run time
//! (C8, point 2). Which crates may name `Mint` is a rule of the
//! composition, not of the type: see [`Mint`].
//!
//! ```compile_fail,E0451
//! // A layer cannot write a caller capability as a literal.
//! use ironafs_vfs::{BindingId, CallerCap, ContextId, CredentialGeneration};
//! let _ = CallerCap {
//!     binding: BindingId(1),
//!     context: ContextId(1),
//!     generation: CredentialGeneration(1),
//!     nonce: 0,
//! };
//! ```
//!
//! ```compile_fail,E0451
//! // Nor a handle capability.
//! use ironafs_vfs::{BindingId, HandleCap, HandleId};
//! let _ = HandleCap { binding: BindingId(1), handle: HandleId(1), nonce: 0 };
//! ```
//!
//! ```compile_fail,E0616
//! // It cannot move a capability it holds to another binding.
//! fn rebind(cap: &mut ironafs_vfs::CallerCap) {
//!     cap.binding = ironafs_vfs::BindingId(2);
//! }
//! ```
//!
//! ```compile_fail,E0277
//! // A handle capability has one holder: it cannot be copied, so the
//! // binding operation that releases it can take it by value.
//! fn keep(cap: &ironafs_vfs::HandleCap) -> ironafs_vfs::HandleCap {
//!     ironafs_vfs::HandleCap::clone(cap)
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // A handle capability is not a caller capability (C7, points 2 and 3).
//! fn caller_operation(_caller: &ironafs_vfs::CallerCap) {}
//! fn misuse(handle: &ironafs_vfs::HandleCap) {
//!     caller_operation(handle);
//! }
//! ```
//!
//! ```compile_fail,E0277
//! // There is no default capability to fall back on.
//! let _ = <ironafs_vfs::CallerCap as Default>::default();
//! ```

use core::fmt;

#[cfg(test)]
mod tests;

/// A binding: one attachment of a layer to the core (a FUSE mount, an
/// FSKit volume, an SMB share or session). Binding operations act on it
/// alone (C7, point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(pub u64);

/// A daemon-issued credential context id (C40, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextId(pub u64);

/// A credential context's generation (C40, point 7). A capability or handle
/// of an earlier generation no longer acts (C13, point 1; C14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialGeneration(pub u64);

/// An open handle's id in the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandleId(pub u64);

/// The operating system's own statement of who a caller is, which a layer
/// presents for the core to mint a caller capability from (C8, point 1).
///
/// The values are untrusted input: the core decides what each means, and a
/// layer naming a caller here is not the core accepting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attestation {
    /// The uid in a FUSE request header.
    FuseUid {
        /// The uid.
        uid: u32,
    },
    /// The uid in FSKit's caller context.
    FskitUid {
        /// The uid.
        uid: u32,
    },
    /// An authenticated SMB session of the front end (L17, L18), by the
    /// session id the core gave it.
    SmbSession {
        /// The session id.
        session: u64,
    },
}

/// The one constructor of capabilities.
///
/// The core holds the daemon's mint; the composition creates it once and
/// gives it to the core. A crate that builds its own `Mint` can make values
/// of the capability types, but the core refuses every capability it did
/// not record itself, by its nonce, binding, context and generation (C8,
/// point 2). That a layer crate never names `Mint` is not enforced by
/// the type: no build check holds it yet, and review does.
#[derive(Debug)]
pub struct Mint(());

impl Mint {
    /// A mint, for the composition to give the core.
    #[expect(
        clippy::new_without_default,
        reason = "a mint is made on purpose, never as a default"
    )]
    #[must_use]
    pub const fn new() -> Self {
        Self(())
    }

    /// Mints a caller capability for one binding and one credential context
    /// at one generation (C8, point 1). `nonce` is drawn by the core from
    /// its `Rng`, so a capability cannot be guessed from the others.
    #[must_use]
    pub const fn caller(
        &self,
        binding: BindingId,
        context: ContextId,
        generation: CredentialGeneration,
        nonce: u128,
    ) -> CallerCap {
        CallerCap {
            binding,
            context,
            generation,
            nonce,
        }
    }

    /// Mints the capability for a handle the core opened on `binding`
    /// (C13, point 1). The handle's own authority (context, generation,
    /// node, mode, rights at open, incarnation) stays in the core's table.
    #[must_use]
    pub const fn handle(&self, binding: BindingId, handle: HandleId, nonce: u128) -> HandleCap {
        HandleCap {
            binding,
            handle,
            nonce,
        }
    }
}

/// The authority of one caller on one binding (C8): what a caller
/// operation carries (C7, point 2).
#[derive(Clone)]
pub struct CallerCap {
    binding: BindingId,
    context: ContextId,
    generation: CredentialGeneration,
    nonce: u128,
}

impl CallerCap {
    /// The binding it was minted for.
    #[must_use]
    pub const fn binding(&self) -> BindingId {
        self.binding
    }

    /// The credential context it acts in.
    #[must_use]
    pub const fn context(&self) -> ContextId {
        self.context
    }

    /// The context's generation it was minted at.
    #[must_use]
    pub const fn generation(&self) -> CredentialGeneration {
        self.generation
    }

    /// The nonce the core checks it by.
    #[must_use]
    pub const fn nonce(&self) -> u128 {
        self.nonce
    }
}

impl fmt::Debug for CallerCap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallerCap")
            .field("binding", &self.binding)
            .field("context", &self.context)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// The authority of one open handle (C13): what a handle operation carries
/// (C7, point 3). It is not `Clone`, so the binding operation that
/// releases it takes it by value and the layer holds no copy afterwards.
pub struct HandleCap {
    binding: BindingId,
    handle: HandleId,
    nonce: u128,
}

impl HandleCap {
    /// The binding it was opened on.
    #[must_use]
    pub const fn binding(&self) -> BindingId {
        self.binding
    }

    /// The handle's id in the core.
    #[must_use]
    pub const fn handle(&self) -> HandleId {
        self.handle
    }

    /// The nonce the core checks it by.
    #[must_use]
    pub const fn nonce(&self) -> u128 {
        self.nonce
    }
}

impl fmt::Debug for HandleCap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandleCap")
            .field("binding", &self.binding)
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}
