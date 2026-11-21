// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A scripted core behind the `ironafs-vfs` contract, for layer tests
//! (ADR 0004, C7), so a layer is tested without the client chain and no
//! test-only command enters the shipped daemon.
//!
//! A test scripts the state (which attestation is which credential
//! context, which handles are revoked, which generations are retired,
//! which callback breaks arrive and which purges fail) and the layer under
//! test talks to the core through the contract's own values:
//!
//! - the capability table: every caller and handle capability is minted
//!   through a [`Mint`](ironafs_vfs::Mint), and any the core did not
//!   record for the binding that presents it is refused with `Stale`
//!   (C8, point 2; contract specification, 5.3, point 3);
//! - the invalidation stream of each binding, coalesced per node, with
//!   its acknowledgements and overflow (C17);
//! - weak and authoritative breaks (C24), and an injected purge failure,
//!   which stales one node in every binding (C15, point 6).
//!
//! The contract's operations and its request and completion interfaces
//! are not in `ironafs-vfs` yet (the contract specification drafts
//! them), so the core has no read, lookup or reply path:
//! [`ScriptedCore::open_handle`] stands in for open until the contract
//! has one. The bytes it serves per context, the set-uid file and the
//! device entry, and a reply whose state changes before submission wait
//! on those interfaces.

use std::collections::{BTreeMap, BTreeSet};

use ironafs_vfs::{
    Ack, Attestation, BindingId, CallerCap, ContextId, Effects, Event, HandleCap, HandleId, NodeId,
    Revision, Scope, VfsError,
};

use ledger::Ledger;
use stream::{Acked, Stream};

mod ledger;
mod stream;

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

/// A binding's report that a purge it was delivered failed (contract
/// specification, 5.10, point 5).
// TODO(#67): use `ironafs_vfs::PurgeFailed` once the contract crate has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PurgeFailed {
    /// The purge of the delivery of `node`'s slot at `revision` failed.
    Node {
        /// The node.
        node: NodeId,
        /// The revision of the failed delivery.
        revision: Revision,
    },
    /// Under the overflow of `revision`, the purge of `node`, which the
    /// binding held when the overflow was delivered, failed.
    OverflowNode {
        /// The node.
        node: NodeId,
        /// The overflow's revision.
        revision: Revision,
    },
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
/// assert_eq!(core.check_caller(other, &caller), Err(VfsError::Stale));
/// assert_eq!(core.layer_faults(), 1);
/// ```
#[derive(Debug)]
pub struct ScriptedCore {
    ledger: Ledger,
    event_bound: usize,
    streams: BTreeMap<BindingId, Stream>,
    suspect: BTreeSet<NodeId>,
    /// The nodes whose purge failed, with each binding whose purge of
    /// them has not succeeded since, and the revision under which the
    /// purge was offered again.
    stale: BTreeMap<(NodeId, BindingId), Revision>,
    /// Layer faults the streams counted.
    faults: u64,
}

impl ScriptedCore {
    /// The default byte bound of a binding's pending events: the contract
    /// specification's proposed 1 MiB of slot state per binding (5.10,
    /// point 9).
    pub const DEFAULT_EVENT_BOUND: usize = 1024 * 1024;

    /// How many layer faults the core counted: a capability presented on
    /// a binding other than its own (contract specification, 5.3,
    /// point 3), a release of a capability it has no record of (5.6,
    /// point 2), and an acknowledgement or purge failure that is neither
    /// valid nor a repeat (5.10, point 6). The specification counts them
    /// per binding; this core keeps one count.
    #[must_use]
    pub fn layer_faults(&self) -> u64 {
        self.ledger.faults().saturating_add(self.faults)
    }

    /// A core with no bindings, whose capability nonces come from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::with_event_bound(seed, Self::DEFAULT_EVENT_BOUND)
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
            stale: BTreeMap::new(),
            faults: 0,
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
    /// (C14, point 3), its pending events are dropped, and a node stale
    /// only from its failed purge is stale no more.
    pub fn detach(&mut self, binding: BindingId) {
        self.ledger.detach(binding);
        self.streams.remove(&binding);
        // Its operating system's cache went with it.
        self.stale.retain(|(_, failed), _| *failed != binding);
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
    /// [`VfsError::Access`] if no context is scripted for `attestation`
    /// (the contract specification does not yet say what the core answers
    /// an attestation it has no context for).
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
    /// [`VfsError::Stale`] if the core did not mint `cap` for `binding`
    /// (another mint's, or altered), if it minted it for another binding
    /// (also counted as a layer fault), if `binding` is detached, or if
    /// the generation is retired (contract specification, 5.3, point 3).
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
    /// contract's open: it checks the caller, and refuses a node a failed
    /// purge left stale (C15, point 6).
    ///
    /// # Errors
    ///
    /// As [`ScriptedCore::check_caller`] for `caller`;
    /// [`VfsError::Stale`] if `node` is stale.
    pub fn open_handle(
        &mut self,
        binding: BindingId,
        caller: &CallerCap,
        node: NodeId,
    ) -> Result<HandleCap, VfsError> {
        if self.node_stale(node) {
            self.ledger.check_caller(binding, caller)?;
            return Err(VfsError::Stale);
        }
        self.ledger.open_handle(binding, caller, node)
    }

    /// The node `cap`'s handle is open on, if the core minted it for
    /// `binding` and it is live (C13, point 2).
    ///
    /// # Errors
    ///
    /// [`VfsError::Stale`] if the core has no record of `cap` (another
    /// mint's, altered, or released), if it minted it for another binding
    /// (also counted as a layer fault), or if the handle is revoked, its
    /// generation retired or `binding` detached (contract specification,
    /// 5.3, point 3).
    pub fn check_handle(&self, binding: BindingId, cap: &HandleCap) -> Result<NodeId, VfsError> {
        self.ledger.check_handle(binding, cap)
    }

    /// Revokes the handle (C14, point 3): every later check of it answers
    /// [`VfsError::Stale`].
    pub fn revoke_handle(&mut self, handle: HandleId) {
        self.ledger.revoke_handle(handle);
    }

    /// Releases the handle `cap` names, if the core minted `cap` for
    /// `binding`, live or revoked. Release never fails (C9, point 4): any
    /// other capability releases nothing and is counted as a layer fault
    /// (contract specification, 5.6, point 2).
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
    /// point 2; contract specification, 5.10, point 3). Only the delivered
    /// revision it names is cleared; effects merged after that delivery
    /// stay pending under their higher revision (C17, point 6).
    ///
    /// An acknowledgement has no reply, and is valid only if it names a
    /// delivery to `binding` not yet acknowledged or reported (5.10,
    /// point 6). An invalid one changes nothing: if its revision is
    /// non-zero and at or below the latest delivered for that node (or for
    /// the overflow) it is a repeat, ignored; otherwise (a revision never
    /// delivered, a node `binding` does not hold, an overflow never
    /// delivered, a binding not attached) it is counted as a layer fault.
    pub fn ack(&mut self, binding: BindingId, ack: Ack) {
        let Some(stream) = self.streams.get_mut(&binding) else {
            self.fault();
            return;
        };
        match stream.ack(ack) {
            Acked::Node(node, revision) => {
                if self
                    .stale
                    .get(&(node, binding))
                    .is_some_and(|offered| *offered <= revision)
                {
                    self.stale.remove(&(node, binding));
                }
            }
            Acked::Overflow(revision) => {
                // The overflow purged every node the binding holds.
                self.stale
                    .retain(|(_, failed), offered| *failed != binding || *offered > revision);
            }
            Acked::Repeat => {}
            Acked::Fault => self.fault(),
        }
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

    /// `binding` reports that a purge it was delivered failed: a node
    /// event's ([`PurgeFailed::Node`]) or, under an overflow, one node's
    /// ([`PurgeFailed::OverflowNode`], reported before the overflow is
    /// acknowledged) (C15, point 6; C17, point 5; contract specification,
    /// 5.10, point 5). The operating
    /// system may still serve the node's old state, so the node answers
    /// [`VfsError::Stale`] in every binding: every context's handles on it,
    /// on every binding, are revoked, and new opens of it are refused until
    /// `binding` acknowledges a purge that covers it. The purge is offered
    /// to `binding` again, as a node event or, when its slot does not fit
    /// the bound, within the overflow. Nothing else changes: no other node,
    /// handle or pending event.
    ///
    /// The report has no reply (5.6, point 1) and follows the validity
    /// rule of [`ScriptedCore::ack`]: an `OverflowNode` report is valid
    /// only for a node `binding` held when the overflow was delivered,
    /// once. An invalid report revokes nothing.
    pub fn purge_failed(&mut self, binding: BindingId, report: PurgeFailed) {
        let Some(stream) = self.streams.get_mut(&binding) else {
            self.fault();
            return;
        };
        let (PurgeFailed::Node { node, .. } | PurgeFailed::OverflowNode { node, .. }) = report;
        match stream.fail(report) {
            Ok(offered) => {
                self.ledger.revoke_node(node);
                self.stale.insert((node, binding), offered);
            }
            Err(Acked::Fault) => self.fault(),
            Err(_) => {}
        }
    }

    /// Whether a failed purge left `node` stale (C15, point 6).
    #[must_use]
    pub fn node_stale(&self, node: NodeId) -> bool {
        self.stale
            .range((node, BindingId(0))..=(node, BindingId(u64::MAX)))
            .next()
            .is_some()
    }

    fn fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }
}
