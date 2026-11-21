// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the scripted core's invalidation stream: events coalesce per
//! node and binding, an acknowledgement clears only the revision it
//! applied, and a binding past its byte bound overflows (ADR 0004, C17).

use ironafs_vfs::{
    Ack, BindingId, ContextId, Effects, Entries, Event, Invalidation, Name, NodeId, Overflow,
    Revision, Rights, Scope, VfsError,
};

use super::SLOT_BYTES;
use crate::ScriptedCore;
use crate::scripted::Evidence;

const FILE: NodeId = NodeId(10);
const DIRECTORY: NodeId = NodeId(20);

const DATA: Effects = Effects {
    data: true,
    attributes: false,
    entries: None,
    rights: None,
};

const ATTRIBUTES: Effects = Effects {
    data: false,
    attributes: true,
    entries: None,
    rights: None,
};

fn entry(name: &[u8]) -> Effects {
    Effects {
        entries: Some(Entries::Name(Name::new(name).unwrap())),
        ..Effects::NONE
    }
}

fn rights(context: u64) -> Effects {
    Effects {
        rights: Some(Rights::Context(ContextId(context))),
        ..Effects::NONE
    }
}

fn holding(core: &mut ScriptedCore, nodes: &[NodeId]) -> BindingId {
    let binding = core.attach();
    for node in nodes {
        core.hold(binding, *node).unwrap();
    }
    binding
}

fn weak(core: &mut ScriptedCore, node: NodeId, effects: &Effects) {
    core.break_node(node, Evidence::Weak, Scope::Object, effects);
}

#[track_caller]
fn node_event(core: &mut ScriptedCore, binding: BindingId) -> Invalidation {
    match core.next_event(binding) {
        Some(Event::Node(invalidation)) => invalidation,
        other => panic!("expected a node event, got {other:?}"),
    }
}

#[test]
fn break_reaches_every_binding_that_holds_the_node() {
    let mut core = ScriptedCore::new(1);
    let first = holding(&mut core, &[FILE]);
    let second = holding(&mut core, &[FILE]);
    let elsewhere = holding(&mut core, &[DIRECTORY]);
    weak(&mut core, FILE, &DATA);
    assert_eq!(node_event(&mut core, first).effects, DATA);
    assert_eq!(node_event(&mut core, second).effects, DATA);
    assert_eq!(core.next_event(elsewhere), None);
}

#[test]
fn events_on_one_node_coalesce_into_one_slot() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    core.break_node(FILE, Evidence::Weak, Scope::Volume, &ATTRIBUTES);
    let event = node_event(&mut core, binding);
    assert_eq!(
        event,
        Invalidation {
            node: FILE,
            scope: Scope::Volume,
            effects: Effects {
                data: true,
                attributes: true,
                entries: None,
                rights: None,
            },
            revision: Revision(2),
        }
    );
    // Delivered and not yet acknowledged: not delivered again.
    assert_eq!(core.next_event(binding), None);
}

#[test]
fn rights_and_entry_events_on_one_node_merge() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[DIRECTORY]);
    weak(&mut core, DIRECTORY, &rights(1));
    weak(&mut core, DIRECTORY, &entry(b"a"));
    weak(&mut core, DIRECTORY, &rights(2));
    weak(&mut core, DIRECTORY, &entry(b"b"));
    let effects = node_event(&mut core, binding).effects;
    assert_eq!(effects.rights, Some(Rights::All));
    assert_eq!(effects.entries, Some(Entries::All));
}

#[test]
fn ack_of_delivered_revision_clears_the_slot() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    let event = node_event(&mut core, binding);
    assert!(core.gated(binding, FILE));
    core.ack(
        binding,
        Ack::Node {
            node: FILE,
            revision: event.revision,
        },
    );
    assert!(!core.gated(binding, FILE));
    assert_eq!(core.next_event(binding), None);
    assert_eq!(core.layer_faults(), 0);
}

#[test]
fn event_during_purge_stays_pending_under_higher_revision() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[DIRECTORY]);
    weak(&mut core, DIRECTORY, &rights(1));
    let applied = node_event(&mut core, binding);
    // The second event arrives while the layer applies the first purge.
    weak(&mut core, DIRECTORY, &entry(b"a"));
    core.ack(
        binding,
        Ack::Node {
            node: DIRECTORY,
            revision: applied.revision,
        },
    );
    assert!(core.gated(binding, DIRECTORY));
    let pending = node_event(&mut core, binding);
    assert!(pending.revision > applied.revision);
    assert_eq!(pending.effects, entry(b"a"));
    core.ack(
        binding,
        Ack::Node {
            node: DIRECTORY,
            revision: pending.revision,
        },
    );
    assert!(!core.gated(binding, DIRECTORY));
}

#[test]
fn ack_of_a_revision_never_issued_clears_nothing() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    let event = node_event(&mut core, binding);
    // Above the latest delivered, revision zero (never issued), and an
    // overflow never delivered: each changes nothing and is a layer fault.
    for ack in [
        Ack::Node {
            node: FILE,
            revision: Revision(event.revision.0 + 1),
        },
        Ack::Node {
            node: FILE,
            revision: Revision(0),
        },
        Ack::Overflow {
            revision: Revision(u64::MAX),
        },
        Ack::Overflow {
            revision: Revision(0),
        },
    ] {
        core.ack(binding, ack);
    }
    assert_eq!(core.layer_faults(), 4);
    assert!(core.gated(binding, FILE));
}

#[test]
fn ack_of_an_undelivered_revision_clears_nothing() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    // Issued but never delivered: the layer cannot have applied it.
    core.ack(
        binding,
        Ack::Node {
            node: FILE,
            revision: Revision(1),
        },
    );
    assert_eq!(core.layer_faults(), 1);
    assert!(core.gated(binding, FILE));
    assert_eq!(node_event(&mut core, binding).effects, DATA);
}

#[test]
fn repeated_ack_changes_nothing() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    let first = node_event(&mut core, binding);
    weak(&mut core, FILE, &ATTRIBUTES);
    let ack = Ack::Node {
        node: FILE,
        revision: first.revision,
    };
    core.ack(binding, ack);
    let second = node_event(&mut core, binding);
    // The first acknowledgement again: a repeat, ignored and never a
    // fault; the later effect stays pending.
    core.ack(binding, ack);
    assert!(core.gated(binding, FILE));
    assert_eq!(core.layer_faults(), 0);
    core.ack(
        binding,
        Ack::Node {
            node: FILE,
            revision: second.revision,
        },
    );
    assert!(!core.gated(binding, FILE));
}

#[test]
fn ack_on_another_binding_clears_nothing() {
    let mut core = ScriptedCore::new(1);
    let first = holding(&mut core, &[FILE]);
    let second = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    let event = node_event(&mut core, first);
    let _ = node_event(&mut core, second);
    core.ack(
        second,
        Ack::Node {
            node: FILE,
            revision: event.revision,
        },
    );
    assert!(core.gated(first, FILE));
    assert!(!core.gated(second, FILE));
}

#[test]
fn pending_bytes_over_bound_overflow() {
    let mut core = ScriptedCore::with_event_bound(1, SLOT_BYTES);
    let binding = holding(&mut core, &[FILE, DIRECTORY]);
    weak(&mut core, FILE, &DATA);
    weak(&mut core, DIRECTORY, &DATA);
    assert_eq!(
        core.next_event(binding),
        Some(Event::Overflow(Overflow {
            revision: Revision(2),
        }))
    );
    assert!(core.gated(binding, FILE));
    assert!(core.gated(binding, DIRECTORY));
    core.ack(
        binding,
        Ack::Overflow {
            revision: Revision(2),
        },
    );
    assert!(!core.gated(binding, FILE));
    assert!(!core.gated(binding, DIRECTORY));
    assert_eq!(core.next_event(binding), None);
}

#[test]
fn names_count_toward_the_bound() {
    let mut core = ScriptedCore::with_event_bound(1, SLOT_BYTES);
    let binding = holding(&mut core, &[DIRECTORY]);
    weak(&mut core, DIRECTORY, &entry(b"a"));
    assert!(matches!(core.next_event(binding), Some(Event::Overflow(_))));
}

#[test]
fn event_after_overflow_sweep_stays_pending_after_overflow_ack() {
    let mut core = ScriptedCore::with_event_bound(1, SLOT_BYTES);
    let binding = holding(&mut core, &[FILE, DIRECTORY]);
    weak(&mut core, FILE, &DATA);
    weak(&mut core, DIRECTORY, &DATA);
    let Some(Event::Overflow(overflow)) = core.next_event(binding) else {
        panic!("expected an overflow");
    };
    // The layer has swept FILE; a new event arrives before the overflow's
    // acknowledgement.
    weak(&mut core, FILE, &ATTRIBUTES);
    core.ack(
        binding,
        Ack::Overflow {
            revision: overflow.revision,
        },
    );
    assert!(core.gated(binding, FILE));
    assert!(!core.gated(binding, DIRECTORY));
    let pending = node_event(&mut core, binding);
    assert!(pending.revision > overflow.revision);
    assert_eq!(pending.effects, ATTRIBUTES);
    core.ack(
        binding,
        Ack::Node {
            node: FILE,
            revision: pending.revision,
        },
    );
    assert!(!core.gated(binding, FILE));
}

#[test]
fn weak_break_marks_node_suspect_authoritative_does_not() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE, DIRECTORY]);
    core.break_node(DIRECTORY, Evidence::Authoritative, Scope::Object, &DATA);
    assert!(!core.suspect(DIRECTORY));
    weak(&mut core, FILE, &DATA);
    assert!(core.suspect(FILE));
    // Both reach the binding all the same.
    assert_eq!(node_event(&mut core, binding).node, FILE);
    assert_eq!(node_event(&mut core, binding).node, DIRECTORY);
}

#[test]
fn ack_of_a_delivery_folded_into_an_overflow_is_a_repeat() {
    let mut core = ScriptedCore::with_event_bound(1, SLOT_BYTES);
    let binding = holding(&mut core, &[FILE, DIRECTORY]);
    weak(&mut core, FILE, &DATA);
    let delivered = node_event(&mut core, binding);
    // The next event folds FILE's delivered slot into an overflow.
    weak(&mut core, DIRECTORY, &DATA);
    core.ack(
        binding,
        Ack::Node {
            node: FILE,
            revision: delivered.revision,
        },
    );
    assert_eq!(core.layer_faults(), 0);
    assert!(core.gated(binding, FILE));
}

#[test]
fn ack_for_a_node_not_held_is_a_fault() {
    let mut core = ScriptedCore::new(1);
    let binding = holding(&mut core, &[FILE]);
    weak(&mut core, FILE, &DATA);
    let event = node_event(&mut core, binding);
    core.ack(
        binding,
        Ack::Node {
            node: DIRECTORY,
            revision: event.revision,
        },
    );
    assert_eq!(core.layer_faults(), 1);
    assert!(core.gated(binding, FILE));
}

#[test]
fn hold_on_an_unknown_binding_is_stale() {
    let mut core = ScriptedCore::new(1);
    assert_eq!(core.hold(BindingId(99), FILE), Err(VfsError::Stale));
    core.ack(
        BindingId(99),
        Ack::Overflow {
            revision: Revision(1),
        },
    );
    assert_eq!(core.layer_faults(), 1);
}

#[test]
fn default_event_bound_is_the_spec_figure() {
    assert_eq!(ScriptedCore::DEFAULT_EVENT_BOUND, 1024 * 1024);
}
