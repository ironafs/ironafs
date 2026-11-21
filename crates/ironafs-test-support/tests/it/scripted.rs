// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The scripted core through its public API, as a layer's tests use it:
//! a capability acts only on its own binding (ADR 0004, C8, point 2), and
//! an injected purge failure stales one node in every binding and revokes
//! every context's handles on it, and nothing else (C15, point 6).

use ironafs_test_support::{Evidence, PurgeFailed, ScriptedCore};
use ironafs_vfs::{
    Ack, Attestation, BindingId, ContextId, Effects, Event, HandleCap, NodeId, Revision, Scope,
    VfsError,
};

const ALICE: Attestation = Attestation::FuseUid { uid: 1000 };
const BOB: Attestation = Attestation::FuseUid { uid: 1001 };
const FILE: NodeId = NodeId(10);
const OTHER: NodeId = NodeId(11);

const DATA: Effects = Effects {
    data: true,
    attributes: false,
    entries: None,
    rights: None,
};

/// Two bindings, two contexts, each holding both nodes.
fn core(bound: usize) -> (ScriptedCore, BindingId, BindingId) {
    let mut core = ScriptedCore::with_event_bound(3, bound);
    core.script_context(ALICE, ContextId(1));
    core.script_context(BOB, ContextId(2));
    let first = core.attach();
    let second = core.attach();
    for binding in [first, second] {
        for node in [FILE, OTHER] {
            core.hold(binding, node).unwrap();
        }
    }
    (core, first, second)
}

fn open(
    core: &mut ScriptedCore,
    binding: BindingId,
    who: Attestation,
    node: NodeId,
) -> Result<HandleCap, VfsError> {
    let caller = core.mint_caller(binding, who).unwrap();
    core.open_handle(binding, &caller, node)
}

#[test]
fn capability_minted_for_another_binding_is_refused() {
    let (mut core, first, second) = core(64 * 1024);
    let caller = core.mint_caller(first, ALICE).unwrap();
    assert_eq!(core.check_caller(second, &caller), Err(VfsError::Stale));
    assert_eq!(
        core.open_handle(second, &caller, FILE)
            .map(|cap| cap.handle()),
        Err(VfsError::Stale)
    );
    let handle = core.open_handle(first, &caller, FILE).unwrap();
    assert_eq!(core.check_handle(second, &handle), Err(VfsError::Stale));
    // Each capability presented on the wrong binding is a fault of the
    // binding that presented it.
    assert_eq!(core.layer_faults(second), 3);
    assert_eq!(core.layer_faults(first), 0);
    assert_eq!(core.check_handle(first, &handle), Ok(FILE));
}

#[test]
fn weak_break_and_purge_failure_stale_the_node_only() {
    let (mut core, first, second) = core(64 * 1024);
    let alice_file = open(&mut core, first, ALICE, FILE).unwrap();
    let bob_file = open(&mut core, second, BOB, FILE).unwrap();
    let alice_other = open(&mut core, first, ALICE, OTHER).unwrap();
    let bob_other = open(&mut core, second, BOB, OTHER).unwrap();
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);

    // The first binding's purge of FILE fails.
    let Some(Event::Node(event)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    assert_eq!(event.node, FILE);
    core.purge_failed(
        first,
        PurgeFailed::Node {
            node: FILE,
            revision: event.revision,
        },
    );
    assert_eq!(core.layer_faults(first), 0);

    // Both contexts' handles on FILE, in every binding, are revoked.
    assert!(core.node_stale(FILE));
    assert_eq!(core.check_handle(first, &alice_file), Err(VfsError::Stale));
    assert_eq!(core.check_handle(second, &bob_file), Err(VfsError::Stale));
    assert_eq!(
        open(&mut core, second, BOB, FILE).map(|cap| cap.handle()),
        Err(VfsError::Stale)
    );
    // Nothing else: the other node, its handles and its pending events.
    assert!(!core.node_stale(OTHER));
    assert_eq!(core.check_handle(first, &alice_other), Ok(OTHER));
    assert_eq!(core.check_handle(second, &bob_other), Ok(OTHER));
    assert!(core.gated(first, OTHER));
    assert!(core.gated(second, OTHER));
    assert!(open(&mut core, first, ALICE, OTHER).is_ok());
}

#[test]
fn failed_purge_is_delivered_again_and_its_ack_ends_the_staleness() {
    let (mut core, first, _) = core(64 * 1024);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(failed)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    core.purge_failed(
        first,
        PurgeFailed::Node {
            node: FILE,
            revision: failed.revision,
        },
    );
    // The purge failed, so the node is gated and offered again.
    assert!(core.gated(first, FILE));
    let Some(Event::Node(retry)) = core.next_event(first) else {
        panic!("expected the node's purge again");
    };
    assert_eq!(retry.node, FILE);
    assert!(retry.revision > failed.revision);
    core.ack(
        first,
        Ack::Node {
            node: FILE,
            revision: retry.revision,
        },
    );
    assert!(!core.node_stale(FILE));
    assert!(open(&mut core, first, ALICE, FILE).is_ok());
}

#[test]
fn purge_failure_under_overflow_stales_that_node_only() {
    // One slot fits; the second overflows.
    let (mut core, first, _) = core(64);
    let file = open(&mut core, first, ALICE, FILE).unwrap();
    let other = open(&mut core, first, BOB, OTHER).unwrap();
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Overflow(overflow)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: FILE,
            revision: overflow.revision,
        },
    );
    core.ack(
        first,
        Ack::Overflow {
            revision: overflow.revision,
        },
    );
    assert!(core.node_stale(FILE));
    assert!(!core.node_stale(OTHER));
    assert_eq!(core.check_handle(first, &file), Err(VfsError::Stale));
    assert_eq!(core.check_handle(first, &other), Ok(OTHER));
    assert!(core.gated(first, FILE));
    assert!(!core.gated(first, OTHER));
}

#[test]
fn purge_failure_not_matching_a_delivery_changes_nothing() {
    let mut core = ScriptedCore::new(3);
    core.script_context(ALICE, ContextId(1));
    let first = core.attach();
    let second = core.attach();
    core.hold(first, OTHER).unwrap();
    core.hold(second, FILE).unwrap();
    let handle = open(&mut core, second, ALICE, FILE).unwrap();
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(event)) = core.next_event(second) else {
        panic!("expected a node event");
    };
    let node = |revision| PurgeFailed::Node {
        node: FILE,
        revision,
    };
    // A binding that does not hold the node, a revision above the latest
    // delivered and an overflow never delivered are faults of the binding
    // that sent them; an unknown binding has no count; revision zero for a
    // node that had a delivery is a repeat. None changes anything
    // (contract specification, 5.10, point 6).
    core.purge_failed(first, node(event.revision));
    core.purge_failed(BindingId(99), node(event.revision));
    core.purge_failed(second, node(Revision(event.revision.0 + 1)));
    core.purge_failed(second, node(Revision(0)));
    core.purge_failed(
        second,
        PurgeFailed::OverflowNode {
            node: FILE,
            revision: event.revision,
        },
    );
    assert_eq!(core.layer_faults(first), 1);
    assert_eq!(core.layer_faults(second), 2);
    assert!(!core.node_stale(FILE));
    assert_eq!(core.check_handle(second, &handle), Ok(FILE));
}

#[test]
fn repeated_purge_failure_is_ignored() {
    let (mut core, first, _) = core(64 * 1024);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(event)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    let report = PurgeFailed::Node {
        node: FILE,
        revision: event.revision,
    };
    core.purge_failed(first, report);
    let Some(Event::Node(retry)) = core.next_event(first) else {
        panic!("expected the node's purge again");
    };
    // The same report again: a repeat, which offers nothing new.
    core.purge_failed(first, report);
    assert_eq!(core.next_event(first), None);
    assert_eq!(core.layer_faults(first), 0);
    core.ack(
        first,
        Ack::Node {
            node: FILE,
            revision: retry.revision,
        },
    );
    assert!(!core.node_stale(FILE));
}

#[test]
fn late_overflow_report_is_a_fault_while_open_and_a_repeat_after() {
    const LATE: NodeId = NodeId(13);
    let (mut core, first, _) = core(0);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Overflow(overflow)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    // Held only after the overflow was delivered.
    core.hold(first, LATE).unwrap();
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: LATE,
            revision: overflow.revision,
        },
    );
    // A node not held at the delivery of an overflow still open: a fault.
    assert_eq!(core.layer_faults(first), 1);
    assert!(!core.node_stale(LATE));
    core.ack(
        first,
        Ack::Overflow {
            revision: overflow.revision,
        },
    );
    // Once the overflow is acknowledged, the same report is a repeat.
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: LATE,
            revision: overflow.revision,
        },
    );
    assert_eq!(core.layer_faults(first), 1);
    assert!(!core.node_stale(LATE));
    assert_eq!(core.next_event(first), None);
}

#[test]
fn forged_and_repeated_reports() {
    // The contract specification's acceptance case 10.5, point 9.
    let (mut core, first, second) = core(64 * 1024);
    let held = open(&mut core, second, BOB, FILE).unwrap();
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(event)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    core.ack(
        first,
        Ack::Node {
            node: FILE,
            revision: Revision(event.revision.0 + 1),
        },
    );
    core.ack(
        first,
        Ack::Node {
            node: NodeId(99),
            revision: event.revision,
        },
    );
    core.purge_failed(
        first,
        PurgeFailed::Node {
            node: FILE,
            revision: Revision(event.revision.0 + 1),
        },
    );
    assert_eq!(core.layer_faults(first), 3);
    assert_eq!(core.check_handle(second, &held), Ok(FILE));
    let valid = Ack::Node {
        node: FILE,
        revision: event.revision,
    };
    core.ack(first, valid);
    core.ack(first, valid);
    assert_eq!(core.layer_faults(first), 3);
    assert!(!core.gated(first, FILE));
}

#[test]
fn reoffered_purge_folded_into_an_overflow_is_still_delivered() {
    const THIRD: NodeId = NodeId(12);
    // One slot fits; a second folds every slot into an overflow.
    let (mut core, first, _) = core(64);
    core.hold(first, THIRD).unwrap();
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Overflow(swept)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    // A new event takes the one slot that fits while the layer sweeps.
    core.break_node(THIRD, Evidence::Weak, Scope::Object, &DATA);
    // FILE's purge under the overflow fails: its re-offer needs a second
    // slot, so it folds into the overflow, which must be offered again
    // after the layer acknowledges the one it swept.
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: FILE,
            revision: swept.revision,
        },
    );
    core.ack(
        first,
        Ack::Overflow {
            revision: swept.revision,
        },
    );
    assert!(core.node_stale(FILE));
    let Some(Event::Overflow(again)) = core.next_event(first) else {
        panic!("expected the overflow again");
    };
    assert!(again.revision > swept.revision);
    core.ack(
        first,
        Ack::Overflow {
            revision: again.revision,
        },
    );
    // The overflow purged every held node, FILE included.
    assert!(!core.node_stale(FILE));
    assert!(!core.gated(first, FILE));
    assert!(open(&mut core, first, ALICE, FILE).is_ok());
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn reoffered_purge_into_a_pending_overflow_is_delivered() {
    // Every event overflows.
    let (mut core, first, _) = core(0);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Overflow(swept)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: FILE,
            revision: swept.revision,
        },
    );
    // The layer reports the node and then acknowledges the overflow; the
    // re-offer folded into it comes as a new overflow revision.
    core.ack(
        first,
        Ack::Overflow {
            revision: swept.revision,
        },
    );
    assert!(core.node_stale(FILE));
    let Some(Event::Overflow(again)) = core.next_event(first) else {
        panic!("expected the overflow again");
    };
    assert!(again.revision > swept.revision);
    core.ack(
        first,
        Ack::Overflow {
            revision: again.revision,
        },
    );
    assert!(!core.node_stale(FILE));
}

#[test]
fn detach_ends_the_staleness_its_failed_purge_caused() {
    let (mut core, first, second) = core(64 * 1024);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(event)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    core.purge_failed(
        first,
        PurgeFailed::Node {
            node: FILE,
            revision: event.revision,
        },
    );
    assert!(core.node_stale(FILE));
    // The binding whose operating system may hold the old state is gone.
    core.detach(first);
    assert!(!core.node_stale(FILE));
    assert!(open(&mut core, second, BOB, FILE).is_ok());
}

#[test]
fn failure_of_a_delivery_folded_into_an_overflow_is_handled() {
    // One slot fits; a second folds every slot into an overflow.
    let (mut core, first, second) = core(64);
    let alice = open(&mut core, first, ALICE, FILE).unwrap();
    let bob = open(&mut core, second, BOB, FILE).unwrap();
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(delivered)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    // A second event during the purge folds FILE's slot into an overflow.
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    // The purge of the delivery still unanswered fails.
    core.purge_failed(
        first,
        PurgeFailed::Node {
            node: FILE,
            revision: delivered.revision,
        },
    );
    assert_eq!(core.layer_faults(first), 0);
    assert!(core.node_stale(FILE));
    assert_eq!(core.check_handle(first, &alice), Err(VfsError::Stale));
    assert_eq!(core.check_handle(second, &bob), Err(VfsError::Stale));
    // The overflow comes first; the re-offer then comes on FILE's slot,
    // and only its acknowledgement ends the staleness.
    let Some(Event::Overflow(overflow)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    core.ack(
        first,
        Ack::Overflow {
            revision: overflow.revision,
        },
    );
    assert!(core.node_stale(FILE));
    let Some(Event::Node(reoffer)) = core.next_event(first) else {
        panic!("expected the re-offer");
    };
    assert_eq!(reoffer.node, FILE);
    assert!(reoffer.revision > overflow.revision);
    core.ack(
        first,
        Ack::Node {
            node: FILE,
            revision: reoffer.revision,
        },
    );
    assert!(!core.node_stale(FILE));
    assert_eq!(core.layer_faults(first), 0);
}

#[test]
fn staleness_ends_only_with_the_delivery_that_carries_the_reoffer() {
    // The re-offer folds into an overflow; a later node event for the same
    // node does not carry it.
    let (mut core, first, _) = core(64);
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Overflow(swept)) = core.next_event(first) else {
        panic!("expected an overflow");
    };
    core.break_node(OTHER, Evidence::Weak, Scope::Object, &DATA);
    core.purge_failed(
        first,
        PurgeFailed::OverflowNode {
            node: FILE,
            revision: swept.revision,
        },
    );
    core.ack(
        first,
        Ack::Overflow {
            revision: swept.revision,
        },
    );
    // A later event for FILE, delivered on its own slot.
    let Some(Event::Overflow(reoffer)) = core.next_event(first) else {
        panic!("expected the overflow carrying the re-offer");
    };
    core.break_node(FILE, Evidence::Weak, Scope::Object, &DATA);
    let Some(Event::Node(later)) = core.next_event(first) else {
        panic!("expected a node event");
    };
    core.ack(
        first,
        Ack::Node {
            node: FILE,
            revision: later.revision,
        },
    );
    assert!(core.node_stale(FILE));
    core.ack(
        first,
        Ack::Overflow {
            revision: reoffer.revision,
        },
    );
    assert!(!core.node_stale(FILE));
    assert_eq!(core.layer_faults(first), 0);
}
