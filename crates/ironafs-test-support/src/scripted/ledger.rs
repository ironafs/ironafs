// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The scripted core's capability table (ADR 0004, C8 and C13): what it
//! minted, for which binding, context and generation, and which handles
//! are still live.
//!
//! Every capability a test hands back is checked against this table, never
//! trusted for its own fields, so a capability minted for another binding,
//! made by another [`Mint`], or of a retired generation is refused (C8,
//! point 2; C14, point 3). Every refusal is `Stale`, which tells the layer
//! to attest the caller again and names no other binding; one recorded
//! for another binding is also counted as a layer fault (contract
//! specification, 5.3, point 3). Nonces come from a seeded
//! [`SplitMix64`], so a run is replayed from its seed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use ironafs_vfs::{
    Attestation, BindingId, CallerCap, ContextId, CredentialGeneration, HandleCap, HandleId, Mint,
    NodeId, VfsError,
};

use crate::SplitMix64;

#[cfg(test)]
mod tests;

/// What the core recorded for one caller capability, by its nonce.
#[derive(Debug)]
struct CallerRecord {
    binding: BindingId,
    context: ContextId,
    generation: CredentialGeneration,
}

/// What the core recorded for one open handle, by its id (C13, point 1).
#[derive(Debug)]
struct HandleRecord {
    binding: BindingId,
    nonce: u128,
    context: ContextId,
    generation: CredentialGeneration,
    node: NodeId,
    live: bool,
}

#[derive(Debug)]
pub(super) struct Ledger {
    mint: Mint,
    nonces: SplitMix64,
    /// Capabilities presented on a binding other than their own, and
    /// releases of a capability the core has no record of.
    /// Counted per binding (contract specification, 7, point 3).
    faults: Mutex<BTreeMap<BindingId, u64>>,
    next_binding: u64,
    next_handle: u64,
    /// How many contexts the core created for uids no test scripted.
    created_contexts: u64,
    bindings: BTreeSet<BindingId>,
    /// The scripted context of each attestation.
    attested: BTreeMap<AttestationKey, ContextId>,
    /// Each context's current generation; absent means the first.
    generations: BTreeMap<ContextId, CredentialGeneration>,
    callers: BTreeMap<u128, CallerRecord>,
    handles: BTreeMap<HandleId, HandleRecord>,
}

/// An attestation as an ordered key ([`Attestation`] is not `Ord`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AttestationKey {
    Fuse(u32),
    Fskit(u32),
    Smb(u64),
}

impl From<Attestation> for AttestationKey {
    fn from(attestation: Attestation) -> Self {
        match attestation {
            Attestation::FuseUid { uid } => Self::Fuse(uid),
            Attestation::FskitUid { uid } => Self::Fskit(uid),
            Attestation::SmbSession { session } => Self::Smb(session),
        }
    }
}

const FIRST_GENERATION: CredentialGeneration = CredentialGeneration(1);

/// The ids of contexts the core creates count up from here, clear of the
/// small ids tests script.
const FIRST_CREATED_CONTEXT: u64 = 1 << 63;

impl Ledger {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            mint: Mint::new(),
            nonces: SplitMix64::new(seed),
            faults: Mutex::new(BTreeMap::new()),
            next_binding: 0,
            next_handle: 0,
            created_contexts: 0,
            bindings: BTreeSet::new(),
            attested: BTreeMap::new(),
            generations: BTreeMap::new(),
            callers: BTreeMap::new(),
            handles: BTreeMap::new(),
        }
    }

    pub(super) fn attach(&mut self) -> BindingId {
        self.next_binding = self.next_binding.wrapping_add(1);
        let binding = BindingId(self.next_binding);
        self.bindings.insert(binding);
        binding
    }

    /// The binding dies: its callers and handles act no more (C14,
    /// point 3).
    pub(super) fn detach(&mut self, binding: BindingId) {
        self.bindings.remove(&binding);
        self.callers.retain(|_, record| record.binding != binding);
        for record in self.handles.values_mut() {
            if record.binding == binding {
                record.live = false;
            }
        }
    }

    pub(super) fn attached(&self, binding: BindingId) -> bool {
        self.bindings.contains(&binding)
    }

    pub(super) fn script_context(&mut self, attestation: Attestation, context: ContextId) {
        self.attested.insert(attestation.into(), context);
    }

    pub(super) fn mint_caller(
        &mut self,
        binding: BindingId,
        attestation: Attestation,
    ) -> Result<CallerCap, VfsError> {
        if !self.attached(binding) {
            return Err(VfsError::Stale);
        }
        let key = AttestationKey::from(attestation);
        let context = match (self.attested.get(&key).copied(), attestation) {
            (Some(context), _) => context,
            // A uid on a shared binding is always accepted: the core finds
            // or creates its context (contract specification, 5.3,
            // point 1). Every binding here is shared.
            (None, Attestation::FuseUid { .. } | Attestation::FskitUid { .. }) => {
                self.created_contexts = self.created_contexts.wrapping_add(1);
                let context = ContextId(FIRST_CREATED_CONTEXT.wrapping_add(self.created_contexts));
                self.attested.insert(key, context);
                context
            }
            // An SMB session the core did not authenticate mints nothing:
            // `Access`, and a layer fault.
            (None, Attestation::SmbSession { .. }) => {
                self.fault(binding);
                return Err(VfsError::Access);
            }
        };
        let generation = self.generation(context);
        let nonce = self.fresh_nonce();
        self.callers.insert(
            nonce,
            CallerRecord {
                binding,
                context,
                generation,
            },
        );
        Ok(self.mint.caller(binding, context, generation, nonce))
    }

    pub(super) fn check_caller(
        &self,
        binding: BindingId,
        cap: &CallerCap,
    ) -> Result<ContextId, VfsError> {
        if !self.attached(binding) {
            return Err(VfsError::Stale);
        }
        let record = self.callers.get(&cap.nonce()).ok_or(VfsError::Stale)?;
        if record.binding != binding || cap.binding() != binding {
            self.fault(binding);
            return Err(VfsError::Stale);
        }
        if record.context != cap.context() || record.generation != cap.generation() {
            return Err(VfsError::Stale);
        }
        if record.generation != self.generation(record.context) {
            return Err(VfsError::Stale);
        }
        Ok(record.context)
    }

    /// The context's credentials changed (C14, points 1 and 3): every
    /// capability and handle of its current generation stops acting.
    pub(super) fn retire_generation(&mut self, context: ContextId) {
        let next = CredentialGeneration(self.generation(context).0.wrapping_add(1));
        self.generations.insert(context, next);
    }

    pub(super) fn open_handle(
        &mut self,
        binding: BindingId,
        caller: &CallerCap,
        node: NodeId,
    ) -> Result<HandleCap, VfsError> {
        let context = self.check_caller(binding, caller)?;
        self.next_handle = self.next_handle.wrapping_add(1);
        let handle = HandleId(self.next_handle);
        let nonce = self.fresh_nonce();
        self.handles.insert(
            handle,
            HandleRecord {
                binding,
                nonce,
                context,
                generation: caller.generation(),
                node,
                live: true,
            },
        );
        Ok(self.mint.handle(binding, handle, nonce))
    }

    pub(super) fn check_handle(
        &self,
        binding: BindingId,
        cap: &HandleCap,
    ) -> Result<NodeId, VfsError> {
        let record = self
            .handles
            .get(&cap.handle())
            .filter(|record| record.nonce == cap.nonce())
            .ok_or(VfsError::Stale)?;
        if record.binding != binding || cap.binding() != binding {
            self.fault(binding);
            return Err(VfsError::Stale);
        }
        if !record.live
            || !self.attached(binding)
            || record.generation != self.generation(record.context)
        {
            return Err(VfsError::Stale);
        }
        Ok(record.node)
    }

    pub(super) fn revoke_handle(&mut self, handle: HandleId) {
        if let Some(record) = self.handles.get_mut(&handle) {
            record.live = false;
        }
    }

    /// Revokes every handle on `node`, in every binding and context.
    pub(super) fn revoke_node(&mut self, node: NodeId) {
        for record in self.handles.values_mut() {
            if record.node == node {
                record.live = false;
            }
        }
    }

    /// Ends the handle if `cap` is the one recorded for it on `binding`,
    /// live or not: releasing a revoked handle, or one of a retired
    /// generation or a detached binding, is ordinary. Release never fails
    /// (C9, point 4), so any other capability releases nothing and is
    /// counted as a layer fault (contract specification, 5.6, point 2).
    #[expect(
        clippy::needless_pass_by_value,
        reason = "release takes the capability, so the layer keeps no copy (C7, point 4)"
    )]
    pub(super) fn release(&mut self, binding: BindingId, cap: HandleCap) {
        let recorded = self.handles.get(&cap.handle()).is_some_and(|record| {
            record.binding == binding && cap.binding() == binding && record.nonce == cap.nonce()
        });
        if recorded {
            self.handles.remove(&cap.handle());
        } else {
            self.fault(binding);
        }
    }

    /// How many layer faults the table counted against `binding`.
    pub(super) fn faults(&self, binding: BindingId) -> u64 {
        self.counts().get(&binding).copied().unwrap_or(0)
    }

    fn fault(&self, binding: BindingId) {
        let mut counts = self.counts();
        let count = counts.entry(binding).or_insert(0);
        *count = count.saturating_add(1);
    }

    fn counts(&self) -> MutexGuard<'_, BTreeMap<BindingId, u64>> {
        self.faults.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn generation(&self, context: ContextId) -> CredentialGeneration {
        self.generations
            .get(&context)
            .copied()
            .unwrap_or(FIRST_GENERATION)
    }

    /// A nonce no capability in the table holds.
    fn fresh_nonce(&mut self) -> u128 {
        loop {
            let high = u128::from(self.nonces.next_u64());
            let nonce = (high << 64) | u128::from(self.nonces.next_u64());
            if !self.callers.contains_key(&nonce)
                && !self.handles.values().any(|record| record.nonce == nonce)
            {
                return nonce;
            }
        }
    }
}
