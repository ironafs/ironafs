// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the scripted core's capability table: each capability acts
//! only on the binding it was minted for, only while its generation and
//! handle are live, and only if the core recorded it (ADR 0004, C8,
//! point 2; C13; C14, point 3).

use ironafs_vfs::{
    Attestation, BindingId, CallerCap, ContextId, CredentialGeneration, HandleCap, Mint, NodeId,
    VfsError,
};

use crate::ScriptedCore;

const ALICE: Attestation = Attestation::FuseUid { uid: 1000 };
const ALICE_CONTEXT: ContextId = ContextId(1);
const FILE: NodeId = NodeId(10);

/// A core with two bindings and one scripted caller.
fn core() -> (ScriptedCore, BindingId, BindingId) {
    let mut core = ScriptedCore::new(7);
    let first = core.attach();
    let second = core.attach();
    core.script_context(ALICE, ALICE_CONTEXT);
    (core, first, second)
}

fn caller(core: &mut ScriptedCore, binding: BindingId) -> CallerCap {
    core.mint_caller(binding, ALICE).unwrap()
}

fn handle(core: &mut ScriptedCore, binding: BindingId) -> HandleCap {
    let cap = caller(core, binding);
    core.open_handle(binding, &cap, FILE).unwrap()
}

#[test]
fn caller_acts_on_its_own_binding() {
    let (mut core, first, _) = core();
    let cap = caller(&mut core, first);
    assert_eq!(cap.binding(), first);
    assert_eq!(core.check_caller(first, &cap), Ok(ALICE_CONTEXT));
}

#[test]
fn caller_minted_for_one_binding_is_refused_on_another() {
    let (mut core, first, second) = core();
    let cap = caller(&mut core, first);
    assert_eq!(core.check_caller(second, &cap), Err(VfsError::Stale));
    // A capability recorded for another binding is a fault of the binding
    // that presented it, and of no other.
    assert_eq!(core.layer_faults(second), 1);
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn caller_from_a_foreign_mint_is_refused() {
    let (mut core, first, _) = core();
    let real = caller(&mut core, first);
    // A copy of every visible field, nonce included, from a mint the core
    // never recorded: refused only if the core checks its own table.
    let forged = Mint::new().caller(first, ALICE_CONTEXT, real.generation(), real.nonce() ^ 1);
    assert_eq!(core.check_caller(first, &forged), Err(VfsError::Stale));
    let other_context = Mint::new().caller(first, ContextId(2), real.generation(), real.nonce());
    assert_eq!(
        core.check_caller(first, &other_context),
        Err(VfsError::Stale)
    );
}

#[test]
fn caller_of_a_retired_generation_is_stale() {
    let (mut core, first, _) = core();
    let old = caller(&mut core, first);
    core.retire_generation(ALICE_CONTEXT);
    assert_eq!(core.check_caller(first, &old), Err(VfsError::Stale));
    let new = caller(&mut core, first);
    assert_eq!(new.generation(), CredentialGeneration(2));
    assert_eq!(core.check_caller(first, &new), Ok(ALICE_CONTEXT));
}

#[test]
fn unscripted_attestation_is_refused() {
    let (mut core, first, _) = core();
    assert_eq!(
        core.mint_caller(first, Attestation::FuseUid { uid: 0 })
            .map(|cap| cap.context()),
        Err(VfsError::Access)
    );
    // The operating system may send a uid the core refuses: not a fault.
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn caller_on_an_unknown_binding_is_refused() {
    let (mut core, _, _) = core();
    assert_eq!(
        core.mint_caller(BindingId(99), ALICE)
            .map(|cap| cap.context()),
        Err(VfsError::Stale)
    );
}

#[test]
fn nonces_differ_between_capabilities() {
    let (mut core, first, _) = core();
    let one = caller(&mut core, first);
    let two = caller(&mut core, first);
    assert_ne!(one.nonce(), two.nonce());
    assert_eq!(core.check_caller(first, &one), Ok(ALICE_CONTEXT));
    assert_eq!(core.check_caller(first, &two), Ok(ALICE_CONTEXT));
}

#[test]
fn handle_acts_on_its_own_binding() {
    let (mut core, first, _) = core();
    let cap = handle(&mut core, first);
    assert_eq!(core.check_handle(first, &cap), Ok(FILE));
}

#[test]
fn handle_minted_for_one_binding_is_refused_on_another() {
    let (mut core, first, second) = core();
    let cap = handle(&mut core, first);
    assert_eq!(core.check_handle(second, &cap), Err(VfsError::Stale));
    assert_eq!(core.layer_faults(second), 1);
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn handle_from_a_foreign_mint_is_refused() {
    let (mut core, first, _) = core();
    let real = handle(&mut core, first);
    let forged = Mint::new().handle(first, real.handle(), real.nonce() ^ 1);
    assert_eq!(core.check_handle(first, &forged), Err(VfsError::Stale));
}

#[test]
fn open_with_a_caller_of_another_binding_is_refused() {
    let (mut core, first, second) = core();
    let cap = caller(&mut core, first);
    assert_eq!(
        core.open_handle(second, &cap, FILE).map(|cap| cap.handle()),
        Err(VfsError::Stale)
    );
}

#[test]
fn revoked_handle_is_stale() {
    let (mut core, first, _) = core();
    let cap = handle(&mut core, first);
    let other = handle(&mut core, first);
    core.revoke_handle(cap.handle());
    assert_eq!(core.check_handle(first, &cap), Err(VfsError::Stale));
    assert_eq!(core.check_handle(first, &other), Ok(FILE));
}

#[test]
fn handle_of_a_retired_generation_is_stale() {
    let (mut core, first, _) = core();
    let cap = handle(&mut core, first);
    core.retire_generation(ALICE_CONTEXT);
    assert_eq!(core.check_handle(first, &cap), Err(VfsError::Stale));
}

#[test]
fn detach_makes_its_handles_and_callers_stale() {
    let (mut core, first, second) = core();
    let gone_caller = caller(&mut core, first);
    let gone_handle = handle(&mut core, first);
    let kept = handle(&mut core, second);
    core.detach(first);
    assert_eq!(core.check_caller(first, &gone_caller), Err(VfsError::Stale));
    assert_eq!(core.check_handle(first, &gone_handle), Err(VfsError::Stale));
    assert_eq!(core.check_handle(second, &kept), Ok(FILE));
}

#[test]
fn release_ends_the_handle() {
    let (mut core, first, _) = core();
    let cap = handle(&mut core, first);
    let kept = handle(&mut core, first);
    // The same id and nonce, so the probe passes while the handle lives.
    let probe = Mint::new().handle(first, cap.handle(), cap.nonce());
    assert_eq!(core.check_handle(first, &probe), Ok(FILE));
    core.release(first, cap);
    assert_eq!(core.check_handle(first, &probe), Err(VfsError::Stale));
    assert_eq!(core.check_handle(first, &kept), Ok(FILE));
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn release_on_another_binding_leaves_the_handle_live() {
    let (mut core, first, second) = core();
    let cap = handle(&mut core, first);
    // The layer of the second binding hands back a capability with the
    // same handle id and nonce but not its own: nothing is released.
    let copy = Mint::new().handle(first, cap.handle(), cap.nonce());
    core.release(second, copy);
    assert_eq!(core.check_handle(first, &cap), Ok(FILE));
    // Release still answers success; a capability with no record for the
    // presenting binding is that binding's fault (5.6, point 2).
    assert_eq!(core.layer_faults(second), 1);
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn release_of_a_dead_handle_is_not_a_fault() {
    let (mut core, first, _) = core();
    let revoked = handle(&mut core, first);
    let retired = handle(&mut core, first);
    core.revoke_handle(revoked.handle());
    core.release(first, revoked);
    core.retire_generation(ALICE_CONTEXT);
    core.release(first, retired);
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn release_of_an_already_released_handle_is_a_fault() {
    let (mut core, first, _) = core();
    let cap = handle(&mut core, first);
    let again = Mint::new().handle(first, cap.handle(), cap.nonce());
    core.release(first, cap);
    core.release(first, again);
    assert_eq!(core.layer_faults(first), 1);
}

#[test]
fn unknown_smb_session_is_refused_and_counted() {
    let (mut core, first, _) = core();
    assert_eq!(
        core.mint_caller(first, Attestation::SmbSession { session: 9 })
            .map(|cap| cap.context()),
        Err(VfsError::Access)
    );
    assert_eq!(core.layer_faults(first), 1);
}
