// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A scripted core behind the `ironafs-vfs` contract, for layer tests
//! (ADR 0004, C7), so a layer is tested without the client chain and no
//! test-only command enters the shipped daemon.
//!
//! A test scripts the state (which attestation is which credential
//! context, which handles are revoked, which generations are retired) and
//! the layer under test talks to the core through the contract's own
//! values. It holds the core's capability table: it mints every caller
//! and handle capability through a [`Mint`](ironafs_vfs::Mint) and refuses
//! any it did not record for the binding that presents it (C8, point 2).
//!
//! The contract's operations and its request and completion interfaces
//! are not defined yet, so the core has no read, lookup or reply path:
//! [`ScriptedCore::open_handle`] stands in for open until the contract
//! has one.

use std::collections::{BTreeMap, BTreeSet};

use ironafs_vfs::{
    Ack, Attestation, BindingId, CallerCap, ContextId, Effects, Event, HandleCap, HandleId, NodeId,
    Scope, VfsError,
};

use ledger::Ledger;
use stream::Stream;

mod ledger;
mod stream;

/// The default byte bound of a binding's pending events.
const DEFAULT_EVENT_BOUND: usize = 64 * 1024;

/// How strong the evidence behind a callback break is (ADR 0004, C24).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Evidence {
    /// Anything weaker than authoritative: it may only lower trust (C24,
    /// points 2 and 3).
    Weak,
    /// Protected by a key no local user can determine, bound to the
    /// object and the event (C24, point 1).
    Authoritative,
}

/// A core whose state a test scripts, behind the contract's values.
///
/// ```
/// use ironafs_test_support::ScriptedCore;
/// use ironafs_vfs::{Attestation, ContextId, VfsError};
///
/// let mut core = ScriptedCore::new(7);
/// let mount = core.attach();
/// let other = core.attach();
/// core.script_context(Attestation::FuseUid { uid: 1000 }, ContextId(1));
/// let caller = core
///     .mint_caller(mount, Attestation::FuseUid { uid: 1000 })
///     .unwrap();
/// assert_eq!(core.check_caller(mount, &caller), Ok(ContextId(1)));
/// // A capability minted for one binding does not act on another.
/// assert_eq!(core.check_caller(other, &caller), Err(VfsError::Access));
/// ```
#[derive(Debug)]
pub struct ScriptedCore {
    ledger: Ledger,
    event_bound: usize,
    streams: BTreeMap<BindingId, Stream>,
    suspect: BTreeSet<NodeId>,
}

impl ScriptedCore {
    /// A core with no bindings, whose capability nonces come from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::with_event_bound(seed, DEFAULT_EVENT_BOUND)
    }

    /// As [`ScriptedCore::new`], with each binding's pending events
    /// bounded at `bytes` (C17, point 6).
    #[must_use]
    pub fn with_event_bound(seed: u64, bytes: usize) -> Self {
        Self {
            ledger: Ledger::new(seed),
            event_bound: bytes,
            streams: BTreeMap::new(),
            suspect: BTreeSet::new(),
        }
    }

    /// Attaches a new binding (a mount, a volume, a share) and returns its
    /// id.
    pub fn attach(&mut self) -> BindingId {
        let binding = self.ledger.attach();
        self.streams.insert(binding, Stream::new(self.event_bound));
        binding
    }

    /// The binding dies: its caller capabilities and handles act no more
    /// (C14, point 3).
    pub fn detach(&mut self, binding: BindingId) {
        self.ledger.detach(binding);
        self.streams.remove(&binding);
    }

    /// Scripts the credential context the core gives a caller that the
    /// operating system attests as `attestation`. A context starts at its
    /// first generation.
    pub fn script_context(&mut self, attestation: Attestation, context: ContextId) {
        self.ledger.script_context(attestation, context);
    }

    /// Mints a caller capability for `binding` from the operating system's
    /// statement of the caller (C8, point 1), at its context's current
    /// generation.
    ///
    /// # Errors
    ///
    /// [`VfsError::Stale`] if `binding` is not attached;
    /// [`VfsError::Access`] if no context is scripted for `attestation`.
    pub fn mint_caller(
        &mut self,
        binding: BindingId,
        attestation: Attestation,
    ) -> Result<CallerCap, VfsError> {
        self.ledger.mint_caller(binding, attestation)
    }

    /// The credential context `cap` acts in, if the core minted it for
    /// `binding` and its generation is current.
    ///
    /// # Errors
    ///
    /// [`VfsError::Access`] if the core did not mint `cap` for `binding`
    /// (another binding's, another mint's, or altered);
    /// [`VfsError::Stale`] if `binding` is detached or the generation is
    /// retired.
    pub fn check_caller(&self, binding: BindingId, cap: &CallerCap) -> Result<ContextId, VfsError> {
        self.ledger.check_caller(binding, cap)
    }

    /// Retires the context's current generation, as a token replaced by
    /// a different identity does (C14, points 1 and 3): its caller
    /// capabilities and handles answer [`VfsError::Stale`], and callers
    /// minted afterwards get the next generation.
    pub fn retire_generation(&mut self, context: ContextId) {
        self.ledger.retire_generation(context);
    }

    /// Opens a handle on `node` for `caller`, on `binding`, and mints its
    /// capability (C13, point 1). This is the script's stand-in for the
    /// contract's open: it checks the caller and nothing about the node.
    ///
    /// # Errors
    ///
    /// As [`ScriptedCore::check_caller`] for `caller`.
    pub fn open_handle(
        &mut self,
        binding: BindingId,
        caller: &CallerCap,
        node: NodeId,
    ) -> Result<HandleCap, VfsError> {
        self.ledger.open_handle(binding, caller, node)
    }

    /// The node `cap`'s handle is open on, if the core minted it for
    /// `binding` and it is live (C13, point 2).
    ///
    /// # Errors
    ///
    /// [`VfsError::Access`] if the core did not mint `cap` for `binding`,
    /// or the handle was released; [`VfsError::Stale`] if the handle is
    /// revoked, its generation retired or `binding` detached.
    pub fn check_handle(&self, binding: BindingId, cap: &HandleCap) -> Result<NodeId, VfsError> {
        self.ledger.check_handle(binding, cap)
    }

    /// Revokes the handle (C14, point 3): every later check of it answers
    /// [`VfsError::Stale`].
    pub fn revoke_handle(&mut self, handle: HandleId) {
        self.ledger.revoke_handle(handle);
    }

    /// Releases the handle `cap` names, if the core minted `cap` for
    /// `binding`. Release never fails (C9, point 4): any other capability
    /// releases nothing.
    pub fn release(&mut self, binding: BindingId, cap: HandleCap) {
        self.ledger.release(binding, cap);
    }

    /// Scripts that `binding` has handed its operating system `node`, so
    /// the node's coherency events reach it (C17, point 3).
    ///
    /// # Errors
    ///
    /// [`VfsError::Stale`] if `binding` is not attached.
    pub fn hold(&mut self, binding: BindingId, node: NodeId) -> Result<(), VfsError> {
        self.streams
            .get_mut(&binding)
            .ok_or(VfsError::Stale)?
            .hold(node);
        Ok(())
    }

    /// A callback break on `node`: its effects merge into the node's slot
    /// in every binding that holds it (C17, points 1 and 3). Weak evidence
    /// also marks the node suspect (C24, point 2). Neither revokes a
    /// handle: weak evidence never does (C24, point 3).
    pub fn break_node(
        &mut self,
        node: NodeId,
        evidence: Evidence,
        scope: Scope,
        effects: &Effects,
    ) {
        if evidence == Evidence::Weak {
            self.suspect.insert(node);
        }
        for stream in self.streams.values_mut() {
            stream.push(node, scope, effects);
        }
    }

    /// Whether a weak break marked `node` suspect.
    #[must_use]
    pub fn suspect(&self, node: NodeId) -> bool {
        self.suspect.contains(&node)
    }

    /// The next event `binding` has not been delivered: an overflow
    /// first, then node slots in node order. A delivered event is not
    /// delivered again until it is acknowledged.
    pub fn next_event(&mut self, binding: BindingId) -> Option<Event> {
        self.streams.get_mut(&binding)?.next_event()
    }

    /// `binding` acknowledges that it applied an event's purge (C17,
    /// point 2). Only the delivered revision it names is cleared; effects
    /// merged after that delivery stay pending under their higher
    /// revision (C17, point 6). An acknowledgement of a revision no longer
    /// delivered clears nothing.
    ///
    /// # Errors
    ///
    /// [`VfsError::Stale`] if `binding` is not attached;
    /// [`VfsError::OutOfRange`] if the revision was never issued to
    /// `binding`. Nothing is cleared on an error.
    pub fn ack(&mut self, binding: BindingId, ack: Ack) -> Result<(), VfsError> {
        self.streams
            .get_mut(&binding)
            .ok_or(VfsError::Stale)?
            .ack(ack)
    }

    /// Whether `binding` must gate replies on `node` (C16, point 5): the
    /// node has pending effects there, or the binding has an
    /// unacknowledged overflow and holds the node.
    #[must_use]
    pub fn gated(&self, binding: BindingId, node: NodeId) -> bool {
        self.streams
            .get(&binding)
            .is_some_and(|stream| stream.gated(node))
    }
}
